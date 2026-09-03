mod worker;

use super::*;
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
        mpsc::{SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
};
use worker::{SpeechEngineWorkerContext, run_speech_engine_after_load};

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
    speech_detected: bool,
}

impl SherpaOffline {
    fn create(config: &SpeechEngineConfig) -> Result<Self, SpeechError> {
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
            speech_detected: false,
        })
    }

    fn accept(&mut self, samples: &[f32], identity: &EventIdentity, sink: &SpeechEventSink) {
        self.vad.accept_waveform(samples);
        let detected = self.vad.detected();
        if detected && !self.speech_detected {
            sink(SpeechEvent::SpeechStarted {
                request_id: identity.request_id.clone(),
                voice_session_id: identity.voice_session_id.clone(),
                chunk_index: self.segment_index,
            });
        }
        self.speech_detected = detected;
        let previous_segment = self.segment_index;
        self.decode_ready_segments(identity, sink);
        if self.segment_index != previous_segment {
            // A maximum-duration split may happen without silence. The next
            // audio frame must pin a fresh destination for the new segment.
            self.speech_detected = false;
        }
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
            // Also finish empty results so clients can leave the processing
            // state and release this segment's pinned destination.
            emit_stable(identity, sink, self.segment_index, text);
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
    fn create(config: &SpeechEngineConfig) -> Result<Self, SpeechError> {
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

/// Non-blocking PCM input for a Speech Engine session.
///
/// A full queue drops the new chunk and records one backpressure incident.
/// Once the session begins stopping, submissions are rejected.
#[derive(Clone)]
pub struct SpeechEngineHandle {
    audio_tx: SyncSender<PcmAudioChunk>,
    running: Arc<AtomicBool>,
    dropped_buffers: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechEngineInputResult {
    Accepted,
    Backpressured,
    Closed,
}

impl SpeechEngineHandle {
    pub fn try_push(&self, chunk: PcmAudioChunk) -> SpeechEngineInputResult {
        if !self.running.load(Ordering::Acquire) {
            return SpeechEngineInputResult::Closed;
        }
        match self.audio_tx.try_send(chunk) {
            Ok(()) => SpeechEngineInputResult::Accepted,
            Err(TrySendError::Full(_)) => {
                self.dropped_buffers.fetch_add(1, Ordering::Relaxed);
                SpeechEngineInputResult::Backpressured
            }
            Err(TrySendError::Disconnected(_)) => SpeechEngineInputResult::Closed,
        }
    }
}

/// An active Speech Engine worker and its bounded PCM input queue.
pub struct SpeechEngineSession {
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl SpeechEngineSession {
    pub fn start(
        config: SpeechEngineConfig,
        sink: SpeechEventSink,
    ) -> Result<(SpeechEngineHandle, Self), SpeechError> {
        Self::start_with_abort(config, sink, None)
    }

    pub(crate) fn start_with_abort(
        config: SpeechEngineConfig,
        sink: SpeechEventSink,
        abort_tx: Option<SyncSender<()>>,
    ) -> Result<(SpeechEngineHandle, Self), SpeechError> {
        let identity = EventIdentity::from(&config);
        let running = Arc::new(AtomicBool::new(true));
        let (audio_tx, audio_rx) = sync_channel(AUDIO_QUEUE_CAPACITY);
        let dropped_buffers = Arc::new(AtomicU64::new(0));
        let worker_running = Arc::clone(&running);
        let worker_dropped = Arc::clone(&dropped_buffers);
        let worker = thread::Builder::new()
            .name("rambledesk-speech".to_owned())
            .spawn(move || {
                run_speech_engine_after_load(
                    config,
                    audio_rx,
                    SpeechEngineWorkerContext {
                        identity,
                        running: worker_running,
                        sink,
                        dropped_buffers: worker_dropped,
                    },
                    abort_tx,
                );
            })
            .map_err(|error| SpeechError::InputStream(error.to_string()))?;
        Ok((
            SpeechEngineHandle {
                audio_tx,
                running: Arc::clone(&running),
                dropped_buffers,
            },
            Self {
                running,
                worker: Some(worker),
            },
        ))
    }

    pub fn stop(mut self) -> Result<(), SpeechError> {
        self.stop_worker()
    }

    fn stop_worker(&mut self) -> Result<(), SpeechError> {
        self.running.store(false, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker.join().map_err(|_| SpeechError::WorkerPanicked),
            None => Ok(()),
        }
    }
}

impl Drop for SpeechEngineSession {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;

    fn test_handle(
        capacity: usize,
    ) -> (
        SpeechEngineHandle,
        std::sync::mpsc::Receiver<PcmAudioChunk>,
        Arc<AtomicBool>,
        Arc<AtomicU64>,
    ) {
        let (audio_tx, audio_rx) = sync_channel(capacity);
        let running = Arc::new(AtomicBool::new(true));
        let dropped_buffers = Arc::new(AtomicU64::new(0));
        (
            SpeechEngineHandle {
                audio_tx,
                running: Arc::clone(&running),
                dropped_buffers: Arc::clone(&dropped_buffers),
            },
            audio_rx,
            running,
            dropped_buffers,
        )
    }

    fn chunk() -> PcmAudioChunk {
        PcmAudioChunk::try_new(vec![0.25, -0.25], 48_000).unwrap()
    }

    #[test]
    fn pcm_input_reports_acceptance_backpressure_and_closed_state() {
        let (handle, audio_rx, running, dropped_buffers) = test_handle(1);
        assert_eq!(handle.try_push(chunk()), SpeechEngineInputResult::Accepted);
        assert_eq!(
            handle.try_push(chunk()),
            SpeechEngineInputResult::Backpressured
        );
        assert_eq!(dropped_buffers.load(Ordering::Relaxed), 1);
        assert_eq!(audio_rx.recv().unwrap(), chunk());

        running.store(false, Ordering::Release);
        assert_eq!(handle.try_push(chunk()), SpeechEngineInputResult::Closed);
    }

    #[test]
    fn pcm_input_reports_closed_when_worker_is_gone() {
        let (handle, audio_rx, _, dropped_buffers) = test_handle(1);
        drop(audio_rx);
        assert_eq!(handle.try_push(chunk()), SpeechEngineInputResult::Closed);
        assert_eq!(dropped_buffers.load(Ordering::Relaxed), 0);
    }

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
