use std::{fs::File, io::BufWriter, sync::Arc, time::Duration};

use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use hound::WavWriter;
use tokio::sync::Mutex;
use tracing::{error, info};

type Writer = WavWriter<BufWriter<File>>;

// 用于持有活动记录的状态
pub struct RecordingState {
    // writer 需要在多个异步任务间共享，并可能被修改
    writer: Arc<Mutex<Writer>>,
    // stream 对象本身控制着音频流的生命周期，当它被 drop 时，流会停止
    // 它不需要在多个线程中被修改，所以不需要 Mutex
    // 但我们需要一种方式在 stop 时拿走它的所有权，所以用 Option
    stream: Option<Stream>,
}

pub struct Recorder {
    pub storage_path: String,
    pub link_path: String,
    state: Arc<Mutex<Option<RecordingState>>>,
}

impl Recorder {
    pub fn new(storage_path: String, link_path: String) -> Self {
        Self {
            storage_path,
            link_path,
            state: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self, filename: String) -> anyhow::Result<()> {
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(device) => device,
            None => {
                return anyhow::bail!("No default input device found.");
            }
        };

        info!("Got device: {}", device.name()?);
        let config = device
            .default_input_config()
            .inspect_err(|e| error!("No input config found: {e}"))?;

        // settting for wav param.
        let spec = hound::WavSpec {
            channels: config.channels(),
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        // create wav file
        let writer = File::create(format!("{}/{}", self.storage_path, filename))
            .inspect_err(|e| error!("Failed to create wav file: {e}"))?;
        let writer = BufWriter::new(writer);
        let writer = match WavWriter::new(writer, spec) {
            Ok(writer) => writer,
            Err(e) => {
                anyhow::bail!("Writer create failed: {e}");
            }
        };

        let writer = Arc::new(Mutex::new(writer));
        let writer_clone = Arc::clone(&writer);

        // Create input stream
        let stream = tokio::task::spawn_blocking(move || {
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut writer = futures::executor::block_on(writer_clone.lock());
                    for &sample in data {
                        match writer.write_sample(sample) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("Input write failed: {e}");
                                return;
                            }
                        }
                    }
                },
                |err| error!("Input stream write failed: {err}"),
                None,
            )
        })
        .await?
        .inspect_err(|e| error!("Stream build faild: {e}"))?;

        stream.play()?;

        Ok(())
    }

    pub async fn stop(&self, writer: Arc<Mutex<Writer>>) {
        let writer = writer.lock().await;
        match writer.finalize() {
            Ok(_) => {}
            Err(e) => {
                error!("Close writer failed: {e}");
                return;
            }
        }
    }
}

#[cfg(test)]
mod recorder_tests {
    use std::{ops::DerefMut, time::Duration};

    use crate::recorder::Recorder;

    #[tokio::test]
    async fn record_test() {
        let recorder = Recorder::new("/tmp".to_string(), "/tmp".to_string());

        let writer = recorder.start("test.wav".to_string()).await.unwrap();

        tokio::time::sleep(Duration::from_secs(5));

        recorder.stop(writer).await;
    }
}
