use std::{fs::File, io::BufReader, time::Duration};

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source, source::Buffered};
use tracing::{debug, error, info, warn};

use super::SpeechLoop;

pub struct Soundbox {
    audio_data: Buffered<Decoder<BufReader<File>>>,
}

impl Soundbox {
    pub fn new(media_name: String) -> Self {
        let file = File::open(format!("resource/{}", media_name)).unwrap();

        let source = Decoder::try_from(file).unwrap();

        Self {
            audio_data: source.buffered(),
        }
    }

    fn create_sink() -> anyhow::Result<(OutputStream, Sink)> {
        let handler = OutputStreamBuilder::open_default_stream()
            .inspect_err(|e| error!("Failed open default stream: {e}"))?;

        let sink = Sink::connect_new(&handler.mixer());
        Ok((handler, sink))
    }

    pub fn cancel(sink: Option<&Sink>) {
        match sink {
            Some(sink) => {
                sink.stop();
            }
            None => match Self::create_sink() {
                Ok((_handler, sink)) => sink.stop(),
                Err(e) => error!("Cancel failed: {e}"),
            },
        }
    }

    pub async fn play(&self, speech_loop: SpeechLoop) -> anyhow::Result<()> {
        let (_handler, sink) = Self::create_sink()?;
        let audio_data = self.audio_data.clone();

        // Because rodio::Sink are sync, we can only run it
        // under blocking mode.
        let playback_task = tokio::task::spawn_blocking(move || {
            // The OutputStream (_handler) is moved here, ensuring it lives for the
            // entire duration of the playback.
            let _stream = _handler;

            for _ in 0..speech_loop.times {
                debug!("Begin playing ...");
                sink.append(audio_data.clone());
                sink.sleep_until_end();
            }
        });

        // Apply the timeout to the await on the blocking task

        match tokio::time::timeout(Duration::from_secs(speech_loop.duration), playback_task).await {
            Ok(_) => {
                info!("Play finished.");
            }
            Err(_) => {
                warn!(
                    "Soundbox did not finish playing in {} secs.",
                    speech_loop.duration
                );
                // Calling the original cancel function here will not work as intended.
                Self::cancel(None);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod soundbox_tests {

    use crate::player::SpeechLoop;

    use super::Soundbox;

    #[ctor::ctor]
    fn init() {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    }

    #[tokio::test]
    async fn test_play() {
        let player = Soundbox::new("please-calm-my-mind-125566.wav".to_string());
        assert!(
            player
                .play(SpeechLoop {
                    duration: 60,
                    times: 1,
                    gap: 2
                })
                .await
                .is_ok()
        );
    }
}
