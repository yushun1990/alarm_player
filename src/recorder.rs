use std::{
    fs::File,
    io::BufWriter,
    sync::{Arc, Mutex},
};

use cpal::{
    FromSample, Sample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use tracing::{error, info};

type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

// Recorder 现在作为一个状态管理器
pub struct Recorder {
    pub storage_path: String,
    pub link_path: String, // 这个字段在您的代码中未使用，但我们保留它
}

impl Recorder {
    pub fn new(storage_path: String, link_path: String) -> Self {
        Self {
            storage_path,
            link_path,
        }
    }

    pub fn start(&self, filename: String) -> anyhow::Result<(cpal::Stream, WavWriterHandle)> {
        let device = match cpal::default_host().default_input_device() {
            Some(device) => device,
            None => return anyhow::bail!("No default input device found."),
        };

        let config = device
            .default_input_config()
            .inspect_err(|e| error!("No default input config found."))?;

        let path = format!("{}/{}", self.storage_path, filename);
        let spec = Self::wav_format_from_config(&config);
        let writer = hound::WavWriter::create(path, spec)?;
        let writer = Arc::new(Mutex::new(Some(writer)));

        let writer_clone = writer.clone();
        let err_fn = move |e| {
            error!("Stream build failed: {e}");
        };

        info!("config.sample_format: {:?}", config.sample_format());

        let stream = match config.sample_format() {
            cpal::SampleFormat::I8 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| Self::write_input_data::<i8, i8>(data, &writer_clone),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| Self::write_input_data::<i16, i16>(data, &writer_clone),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I32 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| Self::write_input_data::<i32, i32>(data, &writer_clone),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| Self::write_input_data::<f32, f32>(data, &writer_clone),
                err_fn,
                None,
            )?,
            sample_format => {
                return anyhow::bail!("Unsupported sample format: {sample_format}");
            }
        };

        stream
            .play()
            .inspect_err(|e| error!("Record failed: {e}"))?;

        Ok((stream, writer))
    }

    fn wav_format_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
        hound::WavSpec {
            channels: config.channels() as _,
            sample_rate: config.sample_rate().0 as _,
            bits_per_sample: (config.sample_format().sample_size() * 8) as _,
            sample_format: Self::sample_format(config.sample_format()),
        }
    }

    fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
        if format.is_float() {
            hound::SampleFormat::Float
        } else {
            hound::SampleFormat::Int
        }
    }

    fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
    where
        T: Sample,
        U: Sample + hound::Sample + FromSample<T>,
    {
        if let Ok(mut guard) = writer.try_lock() {
            if let Some(writer) = guard.as_mut() {
                for &sample in input.iter() {
                    let sample: U = U::from_sample(sample);
                    writer.write_sample(sample).ok();
                }
            }
        }
    }

    pub fn stop(&self, stream: cpal::Stream, writer: WavWriterHandle) -> anyhow::Result<()> {
        drop(stream);
        let mut writer = writer
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock writer failed: {e}"))?;
        let writer = writer
            .take()
            .ok_or_else(|| anyhow::anyhow!("Writer is None!"))?;
        writer
            .finalize()
            .map_err(|e| anyhow::anyhow!("Writer finalize failed: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod recorder_tests {
    use tracing::info;

    use crate::recorder::Recorder;
    use std::{thread::sleep, time::Duration};

    // 辅助函数：初始化日志，方便调试
    fn setup_tracing() {
        let subscriber = tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    }

    #[test]
    fn record_test() {
        setup_tracing(); // 初始化日志记录器

        // 确保 /tmp 目录存在
        std::fs::create_dir_all("/tmp").unwrap();

        let recorder = Recorder::new("/tmp".to_string(), "/tmp".to_string());

        // 开始录制
        let (stream, writer) = recorder.start("test.wav".to_string()).unwrap();

        info!("Recording for 5 seconds...");
        sleep(Duration::from_secs(30));

        // 停止录制
        info!("Stopping recording...");
        recorder.stop(stream, writer).unwrap();
        info!("Recording stopped and file saved.");
    }
}
