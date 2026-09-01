use crate::{
    EventIdentity, NativeAudioSourceConfig, PcmAudioChunk, SpeechEngineConfig, SpeechEngineSession,
    SpeechError, SpeechEvent, SpeechEventSink,
    native_audio_source::{
        NativeAudioSourceFault, NativeAudioSourceFaultSink, NativeAudioSourceSession, PcmAudioSink,
        PreparedNativeAudioSource,
    },
};
use std::{
    sync::{Arc, mpsc::SyncSender, mpsc::sync_channel},
    thread::{self, JoinHandle},
};

/// Compatibility facade that composes the native Audio Source with the Speech
/// Engine while preserving the existing Tauri-facing start/stop interface.
///
/// CoreAudio remains owned by the dedicated session thread. Shutdown always
/// stops the producer before draining and finalizing the Speech Engine.
pub struct SpeechSession {
    stop_tx: Option<SyncSender<()>>,
    owner: Option<JoinHandle<Result<(), SpeechError>>>,
}

impl SpeechSession {
    pub fn start(
        config: crate::SpeechSessionConfig,
        sink: SpeechEventSink,
    ) -> Result<Self, SpeechError> {
        let (startup_tx, startup_rx) = sync_channel(1);
        let (stop_tx, stop_rx) = sync_channel(1);
        let owner_stop_tx = stop_tx.clone();
        let owner = thread::Builder::new()
            .name("rambledesk-speech-session".to_owned())
            .spawn(move || run_owned_session(config, sink, owner_stop_tx, stop_rx, startup_tx))
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

fn run_owned_session(
    config: crate::SpeechSessionConfig,
    sink: SpeechEventSink,
    model_abort_tx: SyncSender<()>,
    stop_rx: std::sync::mpsc::Receiver<()>,
    startup_tx: SyncSender<Result<(), SpeechError>>,
) -> Result<(), SpeechError> {
    let source_config = NativeAudioSourceConfig::from(&config);
    let engine_config = SpeechEngineConfig::from(&config);
    let identity = EventIdentity::from(&engine_config);
    let provider = engine_config.provider;

    // Resolve the native device before starting the recognizer so invalid
    // device configuration has the same synchronous failure semantics as the
    // previous facade.
    let prepared_source = match PreparedNativeAudioSource::open(source_config) {
        Ok(source) => source,
        Err(error) => {
            let _ = startup_tx.send(Err(error));
            return Ok(());
        }
    };
    let (engine_handle, engine_session) = match SpeechEngineSession::start_with_abort(
        engine_config,
        Arc::clone(&sink),
        Some(model_abort_tx),
    ) {
        Ok(engine) => engine,
        Err(error) => {
            let _ = startup_tx.send(Err(error));
            return Ok(());
        }
    };

    let pcm_sink: PcmAudioSink = Arc::new(move |chunk: PcmAudioChunk| {
        let _ = engine_handle.try_push(chunk);
    });
    let fault_identity = identity.clone();
    let fault_event_sink = Arc::clone(&sink);
    let fault_sink: NativeAudioSourceFaultSink = Arc::new(move |fault: NativeAudioSourceFault| {
        fault_event_sink(source_fault_event(&fault_identity, fault));
    });

    let source_session = match prepared_source.start(pcm_sink, fault_sink) {
        Ok(source) => source,
        Err(error) => {
            let _ = engine_session.stop();
            let _ = startup_tx.send(Err(error));
            return Ok(());
        }
    };

    sink(SpeechEvent::Started {
        request_id: identity.request_id.clone(),
        voice_session_id: identity.voice_session_id.clone(),
        input_device: source_session.input_device().to_owned(),
        provider: provider.id().to_owned(),
    });

    if startup_tx.send(Ok(())).is_err() {
        return stop_owned_session(source_session, engine_session, identity, sink);
    }
    let _ = stop_rx.recv();
    stop_owned_session(source_session, engine_session, identity, sink)
}

fn stop_owned_session(
    source_session: NativeAudioSourceSession,
    engine_session: SpeechEngineSession,
    identity: EventIdentity,
    sink: SpeechEventSink,
) -> Result<(), SpeechError> {
    stop_components(
        || source_session.stop(),
        || engine_session.stop(),
        identity,
        sink,
    )
}

fn stop_components(
    stop_source: impl FnOnce(),
    stop_engine: impl FnOnce() -> Result<(), SpeechError>,
    identity: EventIdentity,
    sink: SpeechEventSink,
) -> Result<(), SpeechError> {
    stop_source();
    let engine_result = stop_engine();
    sink(SpeechEvent::Stopped {
        request_id: identity.request_id,
        voice_session_id: identity.voice_session_id,
    });
    engine_result
}

fn source_fault_event(identity: &EventIdentity, fault: NativeAudioSourceFault) -> SpeechEvent {
    SpeechEvent::Error {
        request_id: identity.request_id.clone(),
        voice_session_id: identity.voice_session_id.clone(),
        code: "microphone_stream".to_owned(),
        message: format!("麦克风输入中断：{}", fault.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn test_identity() -> EventIdentity {
        EventIdentity {
            request_id: "request".to_owned(),
            voice_session_id: "voice".to_owned(),
        }
    }

    #[test]
    fn source_fault_keeps_the_existing_tauri_event_contract() {
        let event = source_fault_event(
            &test_identity(),
            NativeAudioSourceFault {
                message: "device lost".to_owned(),
            },
        );
        assert_eq!(
            event,
            SpeechEvent::Error {
                request_id: "request".to_owned(),
                voice_session_id: "voice".to_owned(),
                code: "microphone_stream".to_owned(),
                message: "麦克风输入中断：device lost".to_owned(),
            }
        );
    }

    #[test]
    fn shutdown_stops_source_before_engine_tail_and_stopped_event() {
        let sequence = Arc::new(Mutex::new(Vec::new()));
        let source_sequence = Arc::clone(&sequence);
        let engine_sequence = Arc::clone(&sequence);
        let event_sequence = Arc::clone(&sequence);
        let sink: SpeechEventSink = Arc::new(move |event| {
            let label = match event {
                SpeechEvent::Stable { .. } => "stable",
                SpeechEvent::Stopped { .. } => "stopped",
                _ => "other",
            };
            event_sequence.lock().unwrap().push(label);
        });

        let engine_sink = Arc::clone(&sink);
        stop_components(
            move || source_sequence.lock().unwrap().push("source"),
            move || {
                engine_sequence.lock().unwrap().push("engine");
                engine_sink(SpeechEvent::Stable {
                    request_id: "request".to_owned(),
                    voice_session_id: "voice".to_owned(),
                    chunk_index: 0,
                    text: "tail".to_owned(),
                });
                Ok(())
            },
            test_identity(),
            sink,
        )
        .unwrap();

        assert_eq!(
            *sequence.lock().unwrap(),
            vec!["source", "engine", "stable", "stopped"]
        );
    }

    #[test]
    fn shutdown_emits_stopped_once_when_engine_reports_failure() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded_events = Arc::clone(&events);
        let sink: SpeechEventSink = Arc::new(move |event| {
            recorded_events.lock().unwrap().push(event);
        });

        let result = stop_components(
            || {},
            || Err(SpeechError::WorkerPanicked),
            test_identity(),
            sink,
        );

        assert!(matches!(result, Err(SpeechError::WorkerPanicked)));
        assert_eq!(
            *events.lock().unwrap(),
            vec![SpeechEvent::Stopped {
                request_id: "request".to_owned(),
                voice_session_id: "voice".to_owned(),
            }]
        );
    }
}
