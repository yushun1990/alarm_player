use std::{fs::File, io::BufReader, time::Duration};

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, source::Buffered};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, error, info, warn};

use super::SpeechLoop;

pub type Buffer = Buffered<Decoder<BufReader<File>>>;

pub enum ControlMessage {
    Play(Buffer, SpeechLoop),
    Cancel,
}

#[derive(Default, Clone)]
pub struct Soundbox {
    sender: Option<Sender<ControlMessage>>,
}

impl Soundbox {
    fn create_sink() -> anyhow::Result<(OutputStream, Sink)> {
        let handler = OutputStreamBuilder::open_default_stream()
            .inspect_err(|e| error!("Failed open default stream: {e}"))?;

        let sink = Sink::connect_new(&handler.mixer());
        Ok((handler, sink))
    }

    pub async fn cancel(&self) -> anyhow::Result<()> {
        match self.sender.as_ref() {
            Some(sender) => {
                sender
                    .send(ControlMessage::Cancel)
                    .await
                    .inspect_err(|e| error!("Failed for send cancel: {e}"))?;
                Ok(())
            }
            None => {
                warn!("No play task running.");
                Ok(())
            }
        }
    }

    async fn run(mut receiver: Receiver<ControlMessage>) -> anyhow::Result<()> {
        let (stream, sink) = Self::create_sink()?;
        // 保持stream生命周期
        let _stream = stream;

        loop {
            match receiver.recv().await {
                Some(ControlMessage::Play(buffer, speech_loop)) => {
                    let mut canceled = false;
                    for _ in 0..speech_loop.times {
                        if canceled {
                            break;
                        }
                        sink.append(buffer.clone());

                        while !sink.empty() {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            if let Ok(ControlMessage::Cancel) = receiver.try_recv() {
                                debug!("Play task canceled during iteration...");
                                sink.stop();
                                canceled = true;
                                break;
                            }
                        }
                    }

                    info!("Sink play finished!");
                }
                Some(ControlMessage::Cancel) => {
                    debug!("Received cancel message");
                    sink.stop();
                }
                None => {
                    sink.stop();
                    info!("Close soundbox...");
                    return Ok(());
                }
            }
        }
    }

    #[allow(unreachable_code)]
    pub async fn play(&mut self, buffer: Buffer, speech_loop: SpeechLoop) -> anyhow::Result<()> {
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::channel::<ControlMessage>(10);
            self.sender = Some(sender);
            // start player
            tokio::spawn(Self::run(receiver));
        }

        match &self.sender {
            Some(sender) => {
                sender
                    .send(ControlMessage::Play(buffer, speech_loop.clone()))
                    .await
                    .inspect_err(|e| error!("Failed for sending play message: {e}"))?;
                let duration = speech_loop.duration;
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(duration)) => {
                        warn!("Soundbox did not finish in {} secs, cancelling.", speech_loop.duration);
                        self.cancel().await?;
                    },
                    _ = sender.closed() => {
                        info!("Play task completed.");
                    }
                }
            }
            None => {
                return anyhow::bail!("No sender avalibale");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod soundbox_tests {

    use std::{fs::File, time::Duration};

    use rodio::{Decoder, Source};

    use crate::player::SpeechLoop;

    use super::Soundbox;

    #[tokio::test]
    async fn test_play() {
        let file = File::open("resource/please-calm-my-mind-125566.wav").unwrap();
        let source = Decoder::try_from(file).unwrap();
        let sb = Soundbox::default();

        let mut sb2 = sb.clone();
        tokio::spawn(async move {
            sb2.play(
                source.buffered(),
                SpeechLoop {
                    duration: 180,
                    times: 1,
                    gap: 2,
                },
            )
            .await
        });

        tokio::time::sleep(Duration::from_secs(360)).await;
        let _ = sb.cancel().await;
    }
}
