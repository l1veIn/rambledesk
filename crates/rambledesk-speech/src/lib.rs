use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;

pub const SPEECH_SAMPLE_RATE: u32 = 16_000;

pub type SpeechEventSink = Arc<dyn Fn(SpeechEvent) + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechProvider {
    XAsr,
    SenseVoice,
    FunAsrNano,
}

impl SpeechProvider {
    pub const fn id(self) -> &'static str {
        match self {
            Self::XAsr => "sherpa-onnx-x-asr-zh-en",
            Self::SenseVoice => "sherpa-onnx-sensevoice",
            Self::FunAsrNano => "sherpa-onnx-funasr-nano",
        }
    }

    pub fn from_model_id(model_id: &str) -> Result<Self, SpeechError> {
        match model_id {
            model::X_ASR_MODEL_ID => Ok(Self::XAsr),
            model::SENSEVOICE_MODEL_ID => Ok(Self::SenseVoice),
            model::FUNASR_NANO_MODEL_ID => Ok(Self::FunAsrNano),
            _ => Err(SpeechError::InvalidConfiguration(format!(
                "未知语音模型：{model_id}"
            ))),
        }
    }

    pub const fn streaming(self) -> bool {
        matches!(self, Self::XAsr)
    }
}

#[derive(Debug, Clone)]
pub struct SpeechSessionConfig {
    pub request_id: String,
    pub voice_session_id: String,
    pub provider: SpeechProvider,
    pub model_path: PathBuf,
    pub vad_model_path: PathBuf,
    pub vad_threshold: f32,
    pub vad_silence_ms: u32,
    pub input_device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpeechEvent {
    Started {
        request_id: String,
        voice_session_id: String,
        input_device: String,
        provider: String,
    },
    Partial {
        request_id: String,
        voice_session_id: String,
        text: String,
    },
    Level {
        request_id: String,
        voice_session_id: String,
        rms: f32,
    },
    Processing {
        request_id: String,
        voice_session_id: String,
        chunk_index: u64,
    },
    Stable {
        request_id: String,
        voice_session_id: String,
        chunk_index: u64,
        text: String,
    },
    Warning {
        request_id: String,
        voice_session_id: String,
        code: String,
        message: String,
    },
    Stopped {
        request_id: String,
        voice_session_id: String,
    },
    Error {
        request_id: String,
        voice_session_id: String,
        code: String,
        message: String,
    },
}

#[derive(Debug, Error)]
pub enum SpeechError {
    #[error("语音设置无效：{0}")]
    InvalidConfiguration(String),
    #[error("无法加载语音模型：{0}")]
    ModelLoad(String),
    #[error("Sherpa 模型目录不完整：{0}")]
    ModelIncomplete(String),
    #[error("没有可用的麦克风输入设备")]
    NoInputDevice,
    #[error("无法读取麦克风配置：{0}")]
    InputConfiguration(String),
    #[error("暂不支持麦克风采样格式：{0}")]
    UnsupportedSampleFormat(String),
    #[error("无法启动麦克风：{0}")]
    InputStream(String),
    #[error("语音识别工作线程异常退出")]
    WorkerPanicked,
    #[error("当前平台尚未实现本地语音识别")]
    UnsupportedPlatform,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Clone)]
struct EventIdentity {
    request_id: String,
    voice_session_id: String,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl From<&SpeechSessionConfig> for EventIdentity {
    fn from(config: &SpeechSessionConfig) -> Self {
        Self {
            request_id: config.request_id.clone(),
            voice_session_id: config.voice_session_id.clone(),
        }
    }
}

pub fn resample_linear(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if input.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return input.to_vec();
    }

    let output_len = ((input.len() as u64 * target_rate as u64) / source_rate as u64) as usize;
    if output_len == 0 {
        return Vec::new();
    }
    let ratio = source_rate as f64 / target_rate as f64;
    (0..output_len)
        .map(|index| {
            let source_position = index as f64 * ratio;
            let left = source_position.floor() as usize;
            let right = (left + 1).min(input.len() - 1);
            let fraction = (source_position - left as f64) as f32;
            input[left] + (input[right] - input[left]) * fraction
        })
        .collect()
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

#[cfg(any(target_os = "windows", target_os = "macos", test))]
fn downmix<T>(input: &[T], channels: usize, normalize: impl Fn(&T) -> f32) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    input
        .chunks(channels)
        .map(|frame| frame.iter().map(&normalize).sum::<f32>() / frame.len() as f32)
        .collect()
}

pub mod model;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use native::{SpeechSession, ensure_vad_model, list_input_devices};

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub struct SpeechSession;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn list_input_devices() -> Result<Vec<String>, SpeechError> {
    Ok(Vec::new())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl SpeechSession {
    pub fn start(
        _config: SpeechSessionConfig,
        _sink: SpeechEventSink,
    ) -> Result<Self, SpeechError> {
        Err(SpeechError::UnsupportedPlatform)
    }

    pub fn stop(self) -> Result<(), SpeechError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_preserves_duration_and_endpoints() {
        let input = vec![0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25];
        let output = resample_linear(&input, 8_000, 16_000);
        assert_eq!(output.len(), 16);
        assert_eq!(output[0], 0.0);
        assert!((output[8] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn downmixes_stereo_frames() {
        let input = [1.0_f32, -1.0, 0.5, 0.25];
        assert_eq!(downmix(&input, 2, |value| *value), vec![0.0, 0.375]);
    }

    #[test]
    fn rms_handles_empty_and_constant_audio() {
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[0.5, -0.5]) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn event_contract_serializes_with_stable_discriminator() {
        let event = SpeechEvent::Stable {
            request_id: "request".to_owned(),
            voice_session_id: "session".to_owned(),
            chunk_index: 2,
            text: "你好".to_owned(),
        };
        let value = serde_json::to_value(event).expect("serialize event");
        assert_eq!(value["type"], "stable");
        assert_eq!(value["text"], "你好");
    }
}
