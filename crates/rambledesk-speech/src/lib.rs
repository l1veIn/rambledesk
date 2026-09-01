use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;

pub const SPEECH_SAMPLE_RATE: u32 = 16_000;
const MIN_PCM_SAMPLE_RATE_HZ: u32 = 8_000;
const MAX_PCM_SAMPLE_RATE_HZ: u32 = 192_000;
const MAX_PCM_CHUNK_SECONDS: usize = 10;

pub type SpeechEventSink = Arc<dyn Fn(SpeechEvent) + Send + Sync + 'static>;

/// A normalized mono PCM buffer produced by an Audio Source.
///
/// Audio Sources retain their native sample rate. The Speech Engine owns
/// resampling so browser and native producers share the same recognition path.
#[derive(Debug, Clone, PartialEq)]
pub struct PcmAudioChunk {
    samples: Vec<f32>,
    sample_rate_hz: u32,
}

impl PcmAudioChunk {
    pub fn try_new(samples: Vec<f32>, sample_rate_hz: u32) -> Result<Self, SpeechError> {
        if !(MIN_PCM_SAMPLE_RATE_HZ..=MAX_PCM_SAMPLE_RATE_HZ).contains(&sample_rate_hz) {
            return Err(SpeechError::InvalidConfiguration(format!(
                "PCM 采样率必须在 {MIN_PCM_SAMPLE_RATE_HZ} 到 {MAX_PCM_SAMPLE_RATE_HZ} Hz 之间"
            )));
        }
        if samples.is_empty() {
            return Err(SpeechError::InvalidConfiguration(
                "PCM 音频块不能为空".to_owned(),
            ));
        }
        let max_samples = sample_rate_hz as usize * MAX_PCM_CHUNK_SECONDS;
        if samples.len() > max_samples {
            return Err(SpeechError::InvalidConfiguration(format!(
                "PCM 音频块不能超过 {MAX_PCM_CHUNK_SECONDS} 秒"
            )));
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(SpeechError::InvalidConfiguration(
                "PCM 音频块包含非有限采样值".to_owned(),
            ));
        }
        Ok(Self {
            samples,
            sample_rate_hz,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeAudioSourceConfig {
    pub input_device: Option<String>,
}

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
    pub hotwords: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SpeechEngineConfig {
    pub request_id: String,
    pub voice_session_id: String,
    pub provider: SpeechProvider,
    pub model_path: PathBuf,
    pub vad_model_path: PathBuf,
    pub vad_threshold: f32,
    pub vad_silence_ms: u32,
    pub hotwords: Vec<String>,
}

impl From<&SpeechSessionConfig> for SpeechEngineConfig {
    fn from(config: &SpeechSessionConfig) -> Self {
        Self {
            request_id: config.request_id.clone(),
            voice_session_id: config.voice_session_id.clone(),
            provider: config.provider,
            model_path: config.model_path.clone(),
            vad_model_path: config.vad_model_path.clone(),
            vad_threshold: config.vad_threshold,
            vad_silence_ms: config.vad_silence_ms,
            hotwords: config.hotwords.clone(),
        }
    }
}

impl From<&SpeechSessionConfig> for NativeAudioSourceConfig {
    fn from(config: &SpeechSessionConfig) -> Self {
        Self {
            input_device: config.input_device.clone(),
        }
    }
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
impl From<&SpeechEngineConfig> for EventIdentity {
    fn from(config: &SpeechEngineConfig) -> Self {
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

pub mod model;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native_audio_source;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod session;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod speech_engine;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use native_audio_source::list_input_devices;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use session::SpeechSession;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use speech_engine::{
    SpeechEngineHandle, SpeechEngineInputResult, SpeechEngineSession, ensure_vad_model,
};

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub struct SpeechSession;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn list_input_devices() -> Result<Vec<String>, SpeechError> {
    Ok(Vec::new())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn ensure_vad_model(_library_root: &std::path::Path) -> Result<PathBuf, SpeechError> {
    Err(SpeechError::UnsupportedPlatform)
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

    #[test]
    fn legacy_session_config_projects_independent_source_and_engine_configs() {
        let config = SpeechSessionConfig {
            request_id: "request".to_owned(),
            voice_session_id: "voice".to_owned(),
            provider: SpeechProvider::XAsr,
            model_path: PathBuf::from("model"),
            vad_model_path: PathBuf::from("vad"),
            vad_threshold: 0.5,
            vad_silence_ms: 800,
            input_device: Some("Microphone".to_owned()),
            hotwords: vec!["RambleDesk".to_owned()],
        };

        let source = NativeAudioSourceConfig::from(&config);
        let engine = SpeechEngineConfig::from(&config);
        assert_eq!(source.input_device.as_deref(), Some("Microphone"));
        assert_eq!(engine.request_id, "request");
        assert_eq!(engine.voice_session_id, "voice");
        assert_eq!(engine.provider, SpeechProvider::XAsr);
        assert_eq!(engine.hotwords, vec!["RambleDesk"]);
    }

    #[test]
    fn pcm_audio_chunk_rejects_unbounded_or_invalid_input() {
        assert!(PcmAudioChunk::try_new(vec![0.0], 0).is_err());
        assert!(PcmAudioChunk::try_new(vec![0.0], 7_999).is_err());
        assert!(PcmAudioChunk::try_new(Vec::new(), SPEECH_SAMPLE_RATE).is_err());
        assert!(PcmAudioChunk::try_new(vec![f32::NAN], SPEECH_SAMPLE_RATE).is_err());
        assert!(
            PcmAudioChunk::try_new(
                vec![0.0; SPEECH_SAMPLE_RATE as usize * MAX_PCM_CHUNK_SECONDS + 1],
                SPEECH_SAMPLE_RATE,
            )
            .is_err()
        );
    }
}
