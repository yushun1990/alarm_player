use std::{fs::File, io::BufReader, sync::Arc, time::Duration};

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, source::Buffered};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::{PlayCancelType, PlayResultType, SpeechLoop};

pub type Buffer = Buffered<Decoder<BufReader<File>>>;

#[derive(Default, Clone)]
pub struct Soundbox(u64);

impl Soundbox {
    pub fn new(duration: u64) -> Self {
        Self(duration)
    }

    fn create_sink() -> anyhow::Result<(OutputStream, Sink)> {
        let handler = OutputStreamBuilder::open_default_stream()
            .inspect_err(|e| error!("Failed open default stream: {e}"))?;

        let sink = Sink::connect_new(&handler.mixer());
        Ok((handler, sink))
    }

    #[allow(unreachable_code)]
    pub async fn play(
        &self,
        buffer: Buffer,
        speech_loop: SpeechLoop,
        mut rx: mpsc::Receiver<PlayCancelType>,
    ) -> anyhow::Result<PlayResultType> {
        let (stream, sink) = Self::create_sink()?;
        let _stream = stream;
        let sink = Arc::new(sink);
        let sink_clone = sink.clone();

        let mut result_type = PlayResultType::Normal;

        let duration = speech_loop.duration;
        tokio::select! {
            cancel_type = rx.recv() => {
                info!("Soundbox canceld by rx singnal.");
                sink.stop();
                match cancel_type {
                    Some(cancel_type) => result_type = PlayResultType::Canceled(cancel_type),
                    None => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(duration)) => {
                info!("Soundbox was playing over {} secs, cancelling it.", duration);
                sink.stop();
                result_type = PlayResultType::Timeout;
            }
            _ = async move {
                for i in 0..speech_loop.times {
                    sink_clone.append(buffer.clone());
                    tokio::time::sleep(Duration::from_secs(self.0)).await;
                    while !sink_clone.empty() {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    if i+1 < speech_loop.times {
                        tokio::time::sleep(Duration::from_secs(speech_loop.gap)).await;
                    }
                }
            } => {
                info!("Soundbox finished playing.");
            }
        }

        debug!("Soundbox playing task finished!");

        Ok(result_type)
    }
}

#[cfg(test)]
mod soundbox_tests {

    use std::fs::File;

    use rodio::{Decoder, Source};

    use crate::player::SpeechLoop;

    use super::Soundbox;

    #[tokio::test]
    async fn test_play() {
        let file = File::open("resource/please-calm-my-mind-125566.wav").unwrap();
        let source = Decoder::try_from(file).unwrap();

        let sb = Soundbox(150);
        let (_, rx) = tokio::sync::mpsc::channel(1);
        let _ = sb
            .play(
                source.buffered(),
                SpeechLoop {
                    duration: 360,
                    times: 1,
                    gap: 2,
                },
                rx,
            )
            .await;
    }
}
