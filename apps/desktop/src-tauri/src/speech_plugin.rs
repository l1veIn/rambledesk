use rambledesk_speech::SpeechEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(super) struct StartVoiceRambleInput {
    pub recognition_session_id: String,
    pub input_device: Option<String>,
    pub model_id: String,
    pub vad_threshold: f32,
    pub vad_silence_ms: u32,
    pub hotwords: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct SpeechRecognitionSessionView {
    pub recognition_session_id: String,
    pub provider: String,
    pub model_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum SpeechRecognitionEventView {
    Started {
        recognition_session_id: String,
        input_device: String,
        provider: String,
    },
    Partial {
        recognition_session_id: String,
        text: String,
    },
    Level {
        recognition_session_id: String,
        rms: f32,
    },
    Processing {
        recognition_session_id: String,
        chunk_index: u64,
    },
    Stable {
        recognition_session_id: String,
        chunk_index: u64,
        text: String,
    },
    Warning {
        recognition_session_id: String,
        code: String,
        message: String,
    },
    Stopped {
        recognition_session_id: String,
    },
    Error {
        recognition_session_id: String,
        code: String,
        message: String,
    },
}

impl From<SpeechEvent> for SpeechRecognitionEventView {
    fn from(event: SpeechEvent) -> Self {
        match event {
            SpeechEvent::Started {
                voice_session_id,
                input_device,
                provider,
                ..
            } => Self::Started {
                recognition_session_id: voice_session_id,
                input_device,
                provider,
            },
            SpeechEvent::Partial {
                voice_session_id,
                text,
                ..
            } => Self::Partial {
                recognition_session_id: voice_session_id,
                text,
            },
            SpeechEvent::Level {
                voice_session_id,
                rms,
                ..
            } => Self::Level {
                recognition_session_id: voice_session_id,
                rms,
            },
            SpeechEvent::Processing {
                voice_session_id,
                chunk_index,
                ..
            } => Self::Processing {
                recognition_session_id: voice_session_id,
                chunk_index,
            },
            SpeechEvent::Stable {
                voice_session_id,
                chunk_index,
                text,
                ..
            } => Self::Stable {
                recognition_session_id: voice_session_id,
                chunk_index,
                text,
            },
            SpeechEvent::Warning {
                voice_session_id,
                code,
                message,
                ..
            } => Self::Warning {
                recognition_session_id: voice_session_id,
                code,
                message,
            },
            SpeechEvent::Stopped {
                voice_session_id, ..
            } => Self::Stopped {
                recognition_session_id: voice_session_id,
            },
            SpeechEvent::Error {
                voice_session_id,
                code,
                message,
                ..
            } => Self::Error {
                recognition_session_id: voice_session_id,
                code,
                message,
            },
        }
    }
}
