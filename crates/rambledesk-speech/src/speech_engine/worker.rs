//! The transcription worker thread: it owns the recognizer, drains the audio
//! channel, and emits speech events until the session stops.
//!
//! Split out of `speech_engine.rs` to keep that module within the repository's Rust
//! module size budget.

use super::RecognitionEngine;
use crate::{
    EventIdentity, PcmAudioChunk, SPEECH_SAMPLE_RATE, SpeechEngineConfig, SpeechEvent,
    SpeechEventSink, resample_linear, rms,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender},
    },
    time::Duration,
};

/// Everything the worker needs besides the recognizer and its audio feed.
/// Grouped rather than passed one by one: the two entry points otherwise take
/// eight and seven positional arguments of largely interchangeable types.
pub(super) struct SpeechEngineWorkerContext {
    pub identity: EventIdentity,
    pub running: Arc<AtomicBool>,
    pub sink: SpeechEventSink,
    pub dropped_buffers: Arc<AtomicU64>,
}

/// Load the recognizer, then transcribe. Loading runs concurrently with native
/// source startup; the bounded input queue preserves warmup audio meanwhile.
pub(super) fn run_speech_engine_after_load(
    config: SpeechEngineConfig,
    audio_rx: Receiver<PcmAudioChunk>,
    context: SpeechEngineWorkerContext,
    abort_tx: Option<SyncSender<()>>,
) {
    (context.sink)(SpeechEvent::Warning {
        request_id: context.identity.request_id.clone(),
        voice_session_id: context.identity.voice_session_id.clone(),
        code: "recognizer_loading".to_owned(),
        message: "正在加载语音识别模型…".to_owned(),
    });
    match RecognitionEngine::create(&config) {
        Ok(engine) => run_sherpa_worker(engine, audio_rx, context),
        Err(error) => {
            context.running.store(false, Ordering::Release);
            (context.sink)(SpeechEvent::Error {
                request_id: context.identity.request_id.clone(),
                voice_session_id: context.identity.voice_session_id.clone(),
                code: "model_load".to_owned(),
                message: error.to_string(),
            });
            if let Some(abort_tx) = abort_tx {
                let _ = abort_tx.try_send(());
            }
        }
    }
}

fn run_sherpa_worker(
    mut engine: RecognitionEngine,
    audio_rx: Receiver<PcmAudioChunk>,
    context: SpeechEngineWorkerContext,
) {
    let SpeechEngineWorkerContext {
        identity,
        running,
        sink,
        dropped_buffers,
    } = context;
    loop {
        match audio_rx.recv_timeout(Duration::from_millis(80)) {
            Ok(chunk) => {
                sink(SpeechEvent::Level {
                    request_id: identity.request_id.clone(),
                    voice_session_id: identity.voice_session_id.clone(),
                    rms: rms(&chunk.samples).clamp(0.0, 1.0),
                });
                let audio =
                    resample_linear(&chunk.samples, chunk.sample_rate_hz, SPEECH_SAMPLE_RATE);
                engine.accept(&audio, &identity, &sink);
            }
            Err(RecvTimeoutError::Timeout) if !running.load(Ordering::Acquire) => break,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        emit_backpressure_warning(&identity, &sink, &dropped_buffers);
    }
    engine.finish(&identity, &sink);
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
