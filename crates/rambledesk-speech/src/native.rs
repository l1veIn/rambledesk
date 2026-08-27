mod worker;

use super::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sherpa_onnx::{
    OfflineFunASRNanoModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineSenseVoiceModelConfig, OnlineRecognizer, OnlineRecognizerConfig, OnlineStream,
    VadModelConfig, VoiceActivityDetector,
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
};
use worker::{WorkerContext, run_sherpa_worker_after_load};

// Large enough to hold ~30–40s of typical input callbacks while the ASR
// model loads. Recording starts before recognizer creation; dropping that
// warmup audio would make the first utterance disappear.
const AUDIO_QUEUE_CAPACITY: usize = 4096;
// Enforced at compile time rather than in a test: both sides are constants, so
// a runtime assertion is folded away to `assert!(true)` and proves nothing.
const _: () = assert!(AUDIO_QUEUE_CAPACITY >= 4096);
const SHERPA_SAMPLE_RATE: i32 = 16_000;
const SHERPA_FRAME_SAMPLES: usize = 800;
const SHERPA_TAIL_PADDING_SAMPLES: usize = 12_800;
const SHERPA_FINALIZE_ROUNDS: u32 = 256;
const VAD_MODEL_BYTES: u64 = 643_854;
const VAD_MODEL_FILE: &str = "silero_vad.onnx";
const VAD_BUNDLED_BYTES: &[u8] = include_bytes!("../assets/silero_vad.onnx");

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

pub fn ensure_vad_model(library_root: &Path) -> Result<PathBuf, SpeechError> {
    let directory = library_root
        .join("models")
        .join("speech")
        .join("silero-vad");
    let destination = directory.join(VAD_MODEL_FILE);
    if std::fs::metadata(&destination).is_ok_and(|metadata| metadata.len() == VAD_MODEL_BYTES) {
        return Ok(destination);
    }
    std::fs::create_dir_all(&directory)
        .map_err(|error| SpeechError::ModelLoad(format!("无法创建 VAD 模型目录：{error}")))?;
    let temporary = directory.join(format!("{VAD_MODEL_FILE}.tmp"));
    std::fs::write(&temporary, VAD_BUNDLED_BYTES)
        .map_err(|error| SpeechError::ModelLoad(format!("无法写入 VAD 模型：{error}")))?;
    if destination.exists() {
        std::fs::remove_file(&destination)
            .map_err(|error| SpeechError::ModelLoad(format!("无法替换 VAD 模型：{error}")))?;
    }
    std::fs::rename(&temporary, &destination)
        .map_err(|error| SpeechError::ModelLoad(format!("无法安装 VAD 模型：{error}")))?;
    Ok(destination)
}

struct SherpaOnline {
    recognizer: OnlineRecognizer,
    stream: OnlineStream,
    last_text: String,
    segment_index: u64,
    pending: Vec<f32>,
}

impl SherpaOnline {
    fn create(model_dir: &Path, hotwords: &[String]) -> Result<Self, SpeechError> {
        require_model_files(
            model_dir,
            &[
                "encoder.int8.onnx",
                "decoder.onnx",
                "joiner.int8.onnx",
                "tokens.txt",
            ],
        )?;

        let path = |name: &str| model_dir.join(name).to_string_lossy().into_owned();
        let mut config = OnlineRecognizerConfig::default();
        config.model_config.transducer.encoder = Some(path("encoder.int8.onnx"));
        config.model_config.transducer.decoder = Some(path("decoder.onnx"));
        config.model_config.transducer.joiner = Some(path("joiner.int8.onnx"));
        config.model_config.tokens = Some(path("tokens.txt"));
        config.model_config.num_threads = 2;
        config.model_config.provider = Some("cpu".to_owned());
        let bpe_vocab = model_dir.join("bpe.vocab");
        let has_bpe = is_valid_bpe_vocab(&bpe_vocab);
        if has_bpe {
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
        // Sherpa online transducers encode hotword *phrases* separated by '/'.
        // Without a BPE vocab the default English product names ("Claude", …)
        // are looked up raw in tokens.txt, fail, and spam the log every session.
        let stream = match (has_bpe, join_hotwords(hotwords, '/')) {
            (true, Some(text)) => recognizer.create_stream_with_hotwords(&text),
            _ => recognizer.create_stream(),
        };
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
                emit_partial(identity, sink, String::new());
            }
        }
    }

    fn emit_partial(&mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        let text = self.current_text();
        if !text.is_empty() && text != self.last_text {
            self.last_text = text.clone();
            emit_partial(identity, sink, text);
        }
    }

    fn commit_current(&mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        let text = self.current_text();
        if text.is_empty() {
            return;
        }
        emit_stable(identity, sink, self.segment_index, text);
        self.segment_index += 1;
    }

    fn finish(mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        if !self.pending.is_empty() {
            self.stream
                .accept_waveform(SHERPA_SAMPLE_RATE, &self.pending);
        }
        self.stream
            .accept_waveform(SHERPA_SAMPLE_RATE, &vec![0.0; SHERPA_TAIL_PADDING_SAMPLES]);
        self.stream.input_finished();
        let mut rounds = 0;
        while self.recognizer.is_ready(&self.stream) && rounds < SHERPA_FINALIZE_ROUNDS {
            self.recognizer.decode(&self.stream);
            rounds += 1;
        }
        self.commit_current(identity, sink);
        emit_partial(identity, sink, String::new());
    }

    fn current_text(&self) -> String {
        self.recognizer
            .get_result(&self.stream)
            .map(|result| result.text.trim().to_owned())
            .unwrap_or_default()
    }
}

struct SherpaOffline {
    recognizer: OfflineRecognizer,
    vad: VoiceActivityDetector,
    segment_index: u64,
}

impl SherpaOffline {
    fn create(config: &SpeechSessionConfig) -> Result<Self, SpeechError> {
        if !(0.05..=0.95).contains(&config.vad_threshold) {
            return Err(SpeechError::InvalidConfiguration(
                "VAD 声音阈值必须在 0.05 到 0.95 之间".to_owned(),
            ));
        }
        if !(200..=5_000).contains(&config.vad_silence_ms) {
            return Err(SpeechError::InvalidConfiguration(
                "VAD 静音分段时长必须在 200 到 5000 毫秒之间".to_owned(),
            ));
        }
        if std::fs::metadata(&config.vad_model_path)
            .map(|metadata| metadata.len())
            .unwrap_or_default()
            != VAD_MODEL_BYTES
        {
            return Err(SpeechError::ModelIncomplete(format!(
                "VAD 模型缺失或损坏：{}",
                config.vad_model_path.display()
            )));
        }

        let recognizer =
            create_offline_recognizer(config.provider, &config.model_path, &config.hotwords)?;
        let mut vad_config = VadModelConfig::default();
        vad_config.silero_vad.model = Some(config.vad_model_path.to_string_lossy().into_owned());
        vad_config.silero_vad.threshold = config.vad_threshold;
        vad_config.silero_vad.min_silence_duration = config.vad_silence_ms as f32 / 1_000.0;
        vad_config.silero_vad.min_speech_duration = 0.15;
        vad_config.silero_vad.window_size = 512;
        vad_config.silero_vad.max_speech_duration = 30.0;
        vad_config.sample_rate = SHERPA_SAMPLE_RATE;
        vad_config.num_threads = 1;
        vad_config.provider = Some("cpu".to_owned());
        let vad = VoiceActivityDetector::create(&vad_config, 120.0).ok_or_else(|| {
            SpeechError::ModelLoad(format!(
                "Silero VAD 创建失败：{}",
                config.vad_model_path.display()
            ))
        })?;
        Ok(Self {
            recognizer,
            vad,
            segment_index: 0,
        })
    }

    fn accept(&mut self, samples: &[f32], identity: &EventIdentity, sink: &SpeechEventSink) {
        self.vad.accept_waveform(samples);
        self.decode_ready_segments(identity, sink);
    }

    fn finish(mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        self.vad.flush();
        self.decode_ready_segments(identity, sink);
        emit_partial(identity, sink, String::new());
    }

    fn decode_ready_segments(&mut self, identity: &EventIdentity, sink: &SpeechEventSink) {
        while let Some(segment) = self.vad.front() {
            let samples = segment.samples().to_vec();
            drop(segment);
            self.vad.pop();
            if samples.is_empty() {
                continue;
            }
            sink(SpeechEvent::Processing {
                request_id: identity.request_id.clone(),
                voice_session_id: identity.voice_session_id.clone(),
                chunk_index: self.segment_index,
            });
            let stream = self.recognizer.create_stream();
            stream.accept_waveform(SHERPA_SAMPLE_RATE, &samples);
            self.recognizer.decode(&stream);
            let text = stream
                .get_result()
                .map(|result| result.text.trim().to_owned())
                .unwrap_or_default();
            if !text.is_empty() {
                emit_stable(identity, sink, self.segment_index, text);
            }
            self.segment_index += 1;
        }
    }
}

/// Join a hotword list into the format a sherpa-onnx recognizer expects.
/// Returns `None` when there is nothing to bias after trimming empty entries.
fn join_hotwords(hotwords: &[String], separator: char) -> Option<String> {
    let separator = separator.to_string();
    let cleaned: Vec<&str> = hotwords
        .iter()
        .map(|word| word.trim())
        .filter(|word| !word.is_empty())
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.join(&separator))
    }
}

fn create_offline_recognizer(
    provider: SpeechProvider,
    model_dir: &Path,
    hotwords: &[String],
) -> Result<OfflineRecognizer, SpeechError> {
    let path = |name: &str| model_dir.join(name).to_string_lossy().into_owned();
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.num_threads = 2;
    config.model_config.provider = Some("cpu".to_owned());
    match provider {
        SpeechProvider::SenseVoice => {
            require_model_files(model_dir, &["model.int8.onnx", "tokens.txt"])?;
            config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                model: Some(path("model.int8.onnx")),
                language: Some("auto".to_owned()),
                use_itn: true,
            };
            config.model_config.tokens = Some(path("tokens.txt"));
        }
        SpeechProvider::FunAsrNano => {
            require_model_files(
                model_dir,
                &[
                    "encoder_adaptor.int8.onnx",
                    "llm.int8.onnx",
                    "embedding.int8.onnx",
                    "Qwen3-0.6B/merges.txt",
                    "Qwen3-0.6B/tokenizer.json",
                    "Qwen3-0.6B/vocab.json",
                ],
            )?;
            config.model_config.funasr_nano = OfflineFunASRNanoModelConfig {
                encoder_adaptor: Some(path("encoder_adaptor.int8.onnx")),
                llm: Some(path("llm.int8.onnx")),
                embedding: Some(path("embedding.int8.onnx")),
                tokenizer: Some(path("Qwen3-0.6B")),
                system_prompt: Some("You are a helpful assistant.".to_owned()),
                user_prompt: Some("语音转写：".to_owned()),
                max_new_tokens: 512,
                temperature: 1e-6,
                top_p: 0.8,
                seed: 42,
                language: None,
                itn: 1,
                // FunASR-Nano (Qwen3 ASR) contextual hotwords are comma-separated.
                hotwords: join_hotwords(hotwords, ','),
            };
        }
        SpeechProvider::XAsr => {
            return Err(SpeechError::InvalidConfiguration(
                "X-ASR 应使用流式 recognizer".to_owned(),
            ));
        }
    }
    OfflineRecognizer::create(&config).ok_or_else(|| {
        SpeechError::ModelLoad(format!(
            "{} recognizer 创建失败：{}",
            provider.id(),
            model_dir.display()
        ))
    })
}

fn require_model_files(model_dir: &Path, required: &[&str]) -> Result<(), SpeechError> {
    let missing = required
        .iter()
        .filter(|name| !model_dir.join(name).is_file())
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(SpeechError::ModelIncomplete(format!(
            "{} 缺少 {}",
            model_dir.display(),
            missing.join("、")
        )))
    }
}

fn emit_partial(identity: &EventIdentity, sink: &SpeechEventSink, text: String) {
    sink(SpeechEvent::Partial {
        request_id: identity.request_id.clone(),
        voice_session_id: identity.voice_session_id.clone(),
        text,
    });
}

fn emit_stable(identity: &EventIdentity, sink: &SpeechEventSink, chunk_index: u64, text: String) {
    sink(SpeechEvent::Stable {
        request_id: identity.request_id.clone(),
        voice_session_id: identity.voice_session_id.clone(),
        chunk_index,
        text,
    });
}

fn is_valid_bpe_vocab(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    !bytes.is_empty()
        && !bytes.contains(&0)
        && String::from_utf8(bytes)
            .is_ok_and(|text| text.lines().take(32).any(|line| line.contains('\t')))
}

enum RecognitionEngine {
    Online(SherpaOnline),
    Offline(SherpaOffline),
}

impl RecognitionEngine {
    fn create(config: &SpeechSessionConfig) -> Result<Self, SpeechError> {
        if config.provider.streaming() {
            Ok(Self::Online(SherpaOnline::create(
                &config.model_path,
                &config.hotwords,
            )?))
        } else {
            Ok(Self::Offline(SherpaOffline::create(config)?))
        }
    }

    fn accept(&mut self, samples: &[f32], identity: &EventIdentity, sink: &SpeechEventSink) {
        match self {
            Self::Online(engine) => engine.accept(samples, identity, sink),
            Self::Offline(engine) => engine.accept(samples, identity, sink),
        }
    }

    fn finish(self, identity: &EventIdentity, sink: &SpeechEventSink) {
        match self {
            Self::Online(engine) => engine.finish(identity, sink),
            Self::Offline(engine) => engine.finish(identity, sink),
        }
    }
}

struct NativeSpeechSession {
    identity: EventIdentity,
    running: Arc<AtomicBool>,
    stream: Option<cpal::Stream>,
    worker: Option<JoinHandle<()>>,
    sink: SpeechEventSink,
}

impl NativeSpeechSession {
    fn start(
        config: SpeechSessionConfig,
        sink: SpeechEventSink,
        abort_tx: SyncSender<()>,
    ) -> Result<Self, SpeechError> {
        let provider = config.provider;
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
                run_sherpa_worker_after_load(
                    config,
                    audio_rx,
                    WorkerContext {
                        source_rate,
                        identity: worker_identity,
                        running: worker_running,
                        sink: worker_sink,
                        dropped_buffers: worker_dropped,
                    },
                    abort_tx,
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
        if let Err(error) = stream.play() {
            running.store(false, Ordering::Release);
            drop(stream);
            let _ = worker.join();
            return Err(SpeechError::InputStream(error.to_string()));
        }

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
        let abort_tx = stop_tx.clone();
        let owner = thread::Builder::new()
            .name("rambledesk-speech-session".to_owned())
            .spawn(
                move || match NativeSpeechSession::start(config, sink, abort_tx) {
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
                },
            )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_hotwords_trims_and_skips_empty() {
        assert_eq!(join_hotwords(&[], ' '), None);
        assert_eq!(join_hotwords(&["  ".to_owned(), "".to_owned()], ' '), None);
        assert_eq!(
            join_hotwords(
                &[
                    "Claude Code".to_owned(),
                    "Codex".to_owned(),
                    " Grok ".to_owned()
                ],
                ','
            ),
            Some("Claude Code,Codex,Grok".to_owned())
        );
        assert_eq!(
            join_hotwords(&["Claude Code".to_owned(), "Codex".to_owned()], ' '),
            Some("Claude Code Codex".to_owned())
        );
        assert_eq!(
            join_hotwords(
                &[
                    "Claude Code".to_owned(),
                    "Codex".to_owned(),
                    "Grok".to_owned(),
                    "Gemini".to_owned()
                ],
                '/'
            ),
            Some("Claude Code/Codex/Grok/Gemini".to_owned())
        );
    }

    #[test]
    fn bundled_vad_model_has_expected_size() {
        assert_eq!(VAD_BUNDLED_BYTES.len() as u64, VAD_MODEL_BYTES);
    }

    #[test]
    fn ensure_vad_model_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let first = ensure_vad_model(temp.path()).unwrap();
        assert_eq!(std::fs::metadata(&first).unwrap().len(), VAD_MODEL_BYTES);
        std::fs::write(&first, vec![0; VAD_MODEL_BYTES as usize]).unwrap();
        let second = ensure_vad_model(temp.path()).unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read(&second).unwrap()[0], 0);
    }
}
