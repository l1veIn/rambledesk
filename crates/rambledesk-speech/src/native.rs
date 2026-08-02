use super::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
const AUDIO_QUEUE_CAPACITY: usize = 512;
const SHERPA_SAMPLE_RATE: i32 = 16_000;
const SHERPA_FRAME_SAMPLES: usize = 800;
const SHERPA_TAIL_PADDING_SAMPLES: usize = 12_800;
const SHERPA_FINALIZE_ROUNDS: u32 = 256;

pub fn list_input_devices() -> Result<Vec<String>, SpeechError> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|error| SpeechError::InputConfiguration(error.to_string()))?;
    let mut names = devices
        .filter_map(|device| device.name().ok())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    Ok(names)
}

struct SherpaOnline {
    recognizer: OnlineRecognizer,
    stream: OnlineStream,
    last_text: String,
    segment_index: u64,
    pending: Vec<f32>,
}

impl SherpaOnline {
    fn create(model_dir: &std::path::Path) -> Result<Self, SpeechError> {
        let required = [
            "encoder.int8.onnx",
            "decoder.onnx",
            "joiner.int8.onnx",
            "tokens.txt",
        ];
        let missing = required
            .iter()
            .filter(|name| !model_dir.join(name).is_file())
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(SpeechError::ModelIncomplete(format!(
                "{} 缺少 {}",
                model_dir.display(),
                missing.join("、")
            )));
        }

        let path = |name: &str| model_dir.join(name).to_string_lossy().into_owned();
        let mut config = OnlineRecognizerConfig::default();
        config.model_config.transducer.encoder = Some(path("encoder.int8.onnx"));
        config.model_config.transducer.decoder = Some(path("decoder.onnx"));
        config.model_config.transducer.joiner = Some(path("joiner.int8.onnx"));
        config.model_config.tokens = Some(path("tokens.txt"));
        config.model_config.num_threads = 2;
        config.model_config.provider = Some("cpu".to_owned());
        let bpe_vocab = model_dir.join("bpe.vocab");
        if is_valid_bpe_vocab(&bpe_vocab) {
            config.model_config.modeling_unit = Some("bpe".to_owned());
            config.model_config.bpe_vocab = Some(bpe_vocab.to_string_lossy().into_owned());
        }
        config.decoding_method = Some("modified_beam_search".to_owned());
        config.max_active_paths = 4;
        config.enable_endpoint = true;
        config.rule1_min_trailing_silence = 2.4;
        config.rule2_min_trailing_silence = 0.8;
        config.rule3_min_utterance_length = 10.0;

        let recognizer = OnlineRecognizer::create(&config).ok_or_else(|| {
            SpeechError::ModelLoad(format!(
                "Sherpa X-ASR recognizer 创建失败：{}",
                model_dir.display()
            ))
        })?;
        let stream = recognizer.create_stream();
        Ok(Self {
            recognizer,
            stream,
            last_text: String::new(),
            segment_index: 0,
            pending: Vec::with_capacity(SHERPA_FRAME_SAMPLES * 2),
        })
    }

    fn accept(&mut self, samples: &[f32], identity: &EventIdentity, sink: &SpeechEventSink) {
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= SHERPA_FRAME_SAMPLES {
            let frame: Vec<f32> = self.pending.drain(..SHERPA_FRAME_SAMPLES).collect();
            self.stream.accept_waveform(SHERPA_SAMPLE_RATE, &frame);
            while self.recognizer.is_ready(&self.stream) {
                self.recognizer.decode(&self.stream);
            }
            self.emit_partial(identity, sink);
            if self.recognizer.is_endpoint(&self.stream) {
                self.commit_current(identity, sink);
                self.recognizer.reset(&self.stream);
                self.last_text.clear();
                sink(SpeechEvent::Partial {
                    request_id: identity.request_id.clone(),
                    voice_session_id: identity.voice_session_id.clone(),
                    text: String::new(),
                });
            }
        }
    }

    fn emit_partial(&mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        let text = self.current_text();
        if !text.is_empty() && text != self.last_text {
            self.last_text = text.clone();
            sink(SpeechEvent::Partial {
                request_id: identity.request_id.clone(),
                voice_session_id: identity.voice_session_id.clone(),
                text,
            });
        }
    }

    fn commit_current(&mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        let text = self.current_text();
        if text.is_empty() {
            return;
        }
        sink(SpeechEvent::Stable {
            request_id: identity.request_id.clone(),
            voice_session_id: identity.voice_session_id.clone(),
            chunk_index: self.segment_index,
            text,
        });
        self.segment_index += 1;
    }

    fn finish(mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        if !self.pending.is_empty() {
            self.stream
                .accept_waveform(SHERPA_SAMPLE_RATE, &self.pending);
        }
        let silence = vec![0.0; SHERPA_TAIL_PADDING_SAMPLES];
        self.stream.accept_waveform(SHERPA_SAMPLE_RATE, &silence);
        self.stream.input_finished();
        let mut rounds = 0;
        while self.recognizer.is_ready(&self.stream) && rounds < SHERPA_FINALIZE_ROUNDS {
            self.recognizer.decode(&self.stream);
            rounds += 1;
        }
        self.commit_current(identity, sink);
        sink(SpeechEvent::Partial {
            request_id: identity.request_id.clone(),
            voice_session_id: identity.voice_session_id.clone(),
            text: String::new(),
        });
    }

    fn current_text(&self) -> String {
        self.recognizer
            .get_result(&self.stream)
            .map(|result| result.text.trim().to_owned())
            .unwrap_or_default()
    }
}

fn is_valid_bpe_vocab(path: &std::path::Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    !bytes.is_empty()
        && !bytes.contains(&0)
        && String::from_utf8(bytes)
            .is_ok_and(|text| text.lines().take(32).any(|line| line.contains('\t')))
}

struct NativeSpeechSession {
    identity: EventIdentity,
    running: Arc<AtomicBool>,
    stream: Option<cpal::Stream>,
    worker: Option<JoinHandle<()>>,
    sink: SpeechEventSink,
}

impl NativeSpeechSession {
    fn start(config: SpeechSessionConfig, sink: SpeechEventSink) -> Result<Self, SpeechError> {
        let provider = SpeechProvider::SherpaOnline;
        let sherpa = SherpaOnline::create(&config.model_path)?;

        let host = cpal::default_host();
        let device = if let Some(selected) = config.input_device.as_deref() {
            host.input_devices()
                .map_err(|error| SpeechError::InputConfiguration(error.to_string()))?
                .find(|device| device.name().is_ok_and(|name| name == selected))
                .ok_or_else(|| {
                    SpeechError::InputConfiguration(format!("找不到麦克风：{selected}"))
                })?
        } else {
            host.default_input_device()
                .ok_or(SpeechError::NoInputDevice)?
        };
        let device_name = device
            .name()
            .unwrap_or_else(|_| "系统默认麦克风".to_owned());
        let supported_config = device
            .default_input_config()
            .map_err(|error| SpeechError::InputConfiguration(error.to_string()))?;
        let sample_format = supported_config.sample_format();
        if !matches!(
            sample_format,
            cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
        ) {
            return Err(SpeechError::UnsupportedSampleFormat(format!(
                "{sample_format:?}"
            )));
        }
        let stream_config: cpal::StreamConfig = supported_config.into();
        let source_rate = stream_config.sample_rate.0;
        let channels = stream_config.channels as usize;
        let identity = EventIdentity::from(&config);
        let running = Arc::new(AtomicBool::new(true));
        let (audio_tx, audio_rx) = sync_channel(AUDIO_QUEUE_CAPACITY);
        let dropped_buffers = Arc::new(AtomicU64::new(0));

        let worker_identity = identity.clone();
        let worker_sink = sink.clone();
        let worker_running = running.clone();
        let worker_dropped = dropped_buffers.clone();
        let worker = thread::Builder::new()
            .name("rambledesk-speech".to_owned())
            .spawn(move || {
                run_sherpa_worker(
                    sherpa,
                    audio_rx,
                    source_rate,
                    worker_identity,
                    worker_running,
                    worker_sink,
                    worker_dropped,
                );
            })
            .map_err(|error| SpeechError::InputStream(error.to_string()))?;

        let stream_sink = sink.clone();
        let stream_identity = identity.clone();
        let dropped_for_callback = dropped_buffers.clone();
        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_stream(
                &device,
                &stream_config,
                audio_tx,
                channels,
                |value: &f32| *value,
                dropped_for_callback,
                stream_identity,
                stream_sink,
            ),
            cpal::SampleFormat::I16 => build_stream(
                &device,
                &stream_config,
                audio_tx,
                channels,
                |value: &i16| *value as f32 / i16::MAX as f32,
                dropped_for_callback,
                stream_identity,
                stream_sink,
            ),
            cpal::SampleFormat::U16 => build_stream(
                &device,
                &stream_config,
                audio_tx,
                channels,
                |value: &u16| (*value as f32 - 32_768.0) / 32_768.0,
                dropped_for_callback,
                stream_identity,
                stream_sink,
            ),
            other => Err(SpeechError::UnsupportedSampleFormat(format!("{other:?}"))),
        };
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                running.store(false, Ordering::Release);
                let _ = worker.join();
                return Err(error);
            }
        };
        stream
            .play()
            .map_err(|error| SpeechError::InputStream(error.to_string()))?;

        sink(SpeechEvent::Started {
            request_id: identity.request_id.clone(),
            voice_session_id: identity.voice_session_id.clone(),
            input_device: device_name,
            provider: provider.id().to_owned(),
        });

        Ok(Self {
            identity,
            running,
            stream: Some(stream),
            worker: Some(worker),
            sink,
        })
    }

    fn stop(mut self) -> Result<(), SpeechError> {
        self.running.store(false, Ordering::Release);
        self.stream.take();
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| SpeechError::WorkerPanicked)?;
        }
        (self.sink)(SpeechEvent::Stopped {
            request_id: self.identity.request_id.clone(),
            voice_session_id: self.identity.voice_session_id.clone(),
        });
        Ok(())
    }
}

impl Drop for NativeSpeechSession {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        self.stream.take();
    }
}

/// Sendable control handle for a native audio session.
///
/// CoreAudio streams are thread-affine on macOS, so the native stream stays
/// on a dedicated owner thread. Tauri state stores only this control handle.
pub struct SpeechSession {
    stop_tx: Option<SyncSender<()>>,
    owner: Option<JoinHandle<Result<(), SpeechError>>>,
}

impl SpeechSession {
    pub fn start(config: SpeechSessionConfig, sink: SpeechEventSink) -> Result<Self, SpeechError> {
        let (startup_tx, startup_rx) = sync_channel(1);
        let (stop_tx, stop_rx) = sync_channel(1);
        let owner = thread::Builder::new()
            .name("rambledesk-speech-session".to_owned())
            .spawn(move || match NativeSpeechSession::start(config, sink) {
                Ok(session) => {
                    if startup_tx.send(Ok(())).is_err() {
                        return session.stop();
                    }
                    let _ = stop_rx.recv();
                    session.stop()
                }
                Err(error) => {
                    let _ = startup_tx.send(Err(error));
                    Ok(())
                }
            })
            .map_err(|error| SpeechError::InputStream(error.to_string()))?;

        match startup_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stop_tx: Some(stop_tx),
                owner: Some(owner),
            }),
            Ok(Err(error)) => {
                let _ = owner.join();
                Err(error)
            }
            Err(_) => {
                let _ = owner.join();
                Err(SpeechError::WorkerPanicked)
            }
        }
    }

    pub fn stop(mut self) -> Result<(), SpeechError> {
        self.signal_stop();
        self.join_owner()
    }

    fn signal_stop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
    }

    fn join_owner(&mut self) -> Result<(), SpeechError> {
        match self.owner.take() {
            Some(owner) => owner.join().map_err(|_| SpeechError::WorkerPanicked)?,
            None => Ok(()),
        }
    }
}

impl Drop for SpeechSession {
    fn drop(&mut self) {
        self.signal_stop();
        let _ = self.join_owner();
    }
}

#[allow(clippy::too_many_arguments)]
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    audio_tx: SyncSender<Vec<f32>>,
    channels: usize,
    normalize: impl Fn(&T) -> f32 + Send + Sync + 'static,
    dropped_buffers: Arc<AtomicU64>,
    identity: EventIdentity,
    sink: SpeechEventSink,
) -> Result<cpal::Stream, SpeechError>
where
    T: cpal::SizedSample + Send + 'static,
{
    let error_identity = identity.clone();
    let error_sink = sink.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mono = downmix(data, channels, &normalize);
                if audio_tx.try_send(mono).is_err() {
                    dropped_buffers.fetch_add(1, Ordering::Relaxed);
                }
            },
            move |error| {
                error_sink(SpeechEvent::Error {
                    request_id: error_identity.request_id.clone(),
                    voice_session_id: error_identity.voice_session_id.clone(),
                    code: "microphone_stream".to_owned(),
                    message: format!("麦克风输入中断：{error}"),
                });
            },
            None,
        )
        .map_err(|error| SpeechError::InputStream(error.to_string()))
}

fn run_sherpa_worker(
    mut sherpa: SherpaOnline,
    audio_rx: Receiver<Vec<f32>>,
    source_rate: u32,
    identity: EventIdentity,
    running: Arc<AtomicBool>,
    sink: SpeechEventSink,
    dropped_buffers: Arc<AtomicU64>,
) {
    loop {
        match audio_rx.recv_timeout(Duration::from_millis(80)) {
            Ok(samples) => {
                sink(SpeechEvent::Level {
                    request_id: identity.request_id.clone(),
                    voice_session_id: identity.voice_session_id.clone(),
                    rms: rms(&samples).clamp(0.0, 1.0),
                });
                let audio = resample_linear(&samples, source_rate, SPEECH_SAMPLE_RATE);
                sherpa.accept(&audio, &identity, &sink);
            }
            Err(RecvTimeoutError::Timeout) if !running.load(Ordering::Acquire) => break,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        emit_backpressure_warning(&identity, &sink, &dropped_buffers);
    }
    sherpa.finish(&identity, &sink);
}

fn emit_backpressure_warning(
    identity: &EventIdentity,
    sink: &SpeechEventSink,
    dropped_buffers: &AtomicU64,
) {
    let dropped = dropped_buffers.swap(0, Ordering::Relaxed);
    if dropped > 0 {
        sink(SpeechEvent::Warning {
            request_id: identity.request_id.clone(),
            voice_session_id: identity.voice_session_id.clone(),
            code: "audio_backpressure".to_owned(),
            message: format!("识别速度暂时跟不上，已跳过 {dropped} 个音频缓冲区"),
        });
    }
}
