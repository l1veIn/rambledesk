use crate::{NativeAudioSourceConfig, PcmAudioChunk, SpeechError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;

pub(crate) type PcmAudioSink = Arc<dyn Fn(PcmAudioChunk) + Send + Sync + 'static>;
pub(crate) type NativeAudioSourceFaultSink =
    Arc<dyn Fn(NativeAudioSourceFault) + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeAudioSourceFault {
    pub message: String,
}

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

/// An active native Audio Source.
///
/// The cpal stream stays on the thread that creates this value. It only emits
/// normalized mono PCM and source faults; request and recognition semantics
/// belong to the facade and Speech Engine respectively.
pub(crate) struct NativeAudioSourceSession {
    input_device: String,
    stream: Option<cpal::Stream>,
}

impl NativeAudioSourceSession {
    pub(crate) fn input_device(&self) -> &str {
        &self.input_device
    }

    pub(crate) fn stop(mut self) {
        self.stream.take();
    }
}

impl Drop for NativeAudioSourceSession {
    fn drop(&mut self) {
        self.stream.take();
    }
}

pub(crate) struct PreparedNativeAudioSource {
    device: cpal::Device,
    input_device: String,
    stream_config: cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
}

impl PreparedNativeAudioSource {
    pub(crate) fn open(config: NativeAudioSourceConfig) -> Result<Self, SpeechError> {
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
        let input_device = device
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
        Ok(Self {
            device,
            input_device,
            stream_config: supported_config.into(),
            sample_format,
        })
    }

    pub(crate) fn start(
        self,
        pcm_sink: PcmAudioSink,
        fault_sink: NativeAudioSourceFaultSink,
    ) -> Result<NativeAudioSourceSession, SpeechError> {
        let source_rate = self.stream_config.sample_rate.0;
        let channels = self.stream_config.channels as usize;
        let stream = match self.sample_format {
            cpal::SampleFormat::F32 => build_stream(
                &self.device,
                &self.stream_config,
                channels,
                source_rate,
                pcm_sink,
                fault_sink,
                |value: &f32| *value,
            ),
            cpal::SampleFormat::I16 => build_stream(
                &self.device,
                &self.stream_config,
                channels,
                source_rate,
                pcm_sink,
                fault_sink,
                |value: &i16| *value as f32 / i16::MAX as f32,
            ),
            cpal::SampleFormat::U16 => build_stream(
                &self.device,
                &self.stream_config,
                channels,
                source_rate,
                pcm_sink,
                fault_sink,
                |value: &u16| (*value as f32 - 32_768.0) / 32_768.0,
            ),
            other => Err(SpeechError::UnsupportedSampleFormat(format!("{other:?}"))),
        }?;
        stream
            .play()
            .map_err(|error| SpeechError::InputStream(error.to_string()))?;
        Ok(NativeAudioSourceSession {
            input_device: self.input_device,
            stream: Some(stream),
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    sample_rate_hz: u32,
    pcm_sink: PcmAudioSink,
    fault_sink: NativeAudioSourceFaultSink,
    normalize: impl Fn(&T) -> f32 + Send + Sync + 'static,
) -> Result<cpal::Stream, SpeechError>
where
    T: cpal::SizedSample + Send + 'static,
{
    let data_fault_sink = Arc::clone(&fault_sink);
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let samples = downmix(data, channels, &normalize);
                if samples.is_empty() {
                    return;
                }
                match PcmAudioChunk::try_new(samples, sample_rate_hz) {
                    Ok(chunk) => pcm_sink(chunk),
                    Err(error) => data_fault_sink(NativeAudioSourceFault {
                        message: error.to_string(),
                    }),
                }
            },
            move |error| {
                fault_sink(NativeAudioSourceFault {
                    message: error.to_string(),
                });
            },
            None,
        )
        .map_err(|error| SpeechError::InputStream(error.to_string()))
}

fn downmix<T>(input: &[T], channels: usize, normalize: impl Fn(&T) -> f32) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    input
        .chunks(channels)
        .map(|frame| frame.iter().map(&normalize).sum::<f32>() / frame.len() as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_and_normalizes_stereo_frames() {
        let input = [1.0_f32, -1.0, 0.5, 0.25];
        assert_eq!(downmix(&input, 2, |value| *value), vec![0.0, 0.375]);
    }

    #[test]
    fn downmix_rejects_zero_channels_without_panicking() {
        assert!(downmix(&[1.0_f32], 0, |value| *value).is_empty());
    }
}
