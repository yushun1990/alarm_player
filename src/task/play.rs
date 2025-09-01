use std::{fs::File, sync::Arc};

use rodio::{Decoder, Source};
use tokio::sync::{
    RwLock,
    mpsc::{Receiver, Sender},
};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    Recorder,
    config::PlayMode,
    model::Alarm,
    player::{Buffer, PlayContent, Soundbox, Soundpost, SpeechLoop},
    service::{AlarmService, AlarmStatus, BoxConfig, PlayResult, PostConfig},
};

pub struct Play {
    alarm_media_buffer: Buffer,
    test_media_buffer: Buffer,
    alarm_media_url: String,
    test_media_url: String,
    soundpost: Soundpost,
    recorder: Recorder,
    service: Arc<RwLock<AlarmService>>,
}

impl Play {
    pub fn new(
        alarm_media_name: String,
        test_media_name: String,
        alarm_media_url: String,
        test_media_url: String,
        soundpost: Soundpost,
        recorder: Recorder,
        service: Arc<RwLock<AlarmService>>,
    ) -> Self {
        Self {
            alarm_media_buffer: Self::get_buffer(alarm_media_name),
            test_media_buffer: Self::get_buffer(test_media_name),
            alarm_media_url,
            test_media_url,
            soundpost,
            recorder,
            service,
        }
    }

    fn get_buffer(name: String) -> Buffer {
        let file = File::open(format!("resource/{}", name)).unwrap();
        Decoder::try_from(file).unwrap().buffered()
    }

    pub async fn run(&self, alarm_tx: Sender<Alarm>, mut alarm_rx: Receiver<Alarm>) {
        loop {
            let alarm = match alarm_rx.recv().await {
                Some(alarm) => alarm,
                None => {
                    info!("Play queue was closed, exit...");
                    return;
                }
            };

            let alarm_status = {
                let service = self.service.read().await;
                service.get_alarm_status(&alarm)
            };

            let box_config = {
                let service = self.service.read().await;
                service.get_soundbox()
            };

            let posts_config = {
                let service = self.service.read().await;
                service.get_soundposts()
            };

            let test_play_duration = {
                let service = self.service.read().await;
                service.get_test_play_duration()
            };

            let play_interval = {
                let service = self.service.read().await;
                service.get_play_interval_secs()
            };

            if alarm.is_test {
                // 測試報警，直接播放
                info!("Play test alarm: {:?}", alarm);
                let result = self
                    .play_test_alarm(
                        box_config,
                        posts_config,
                        SpeechLoop {
                            duration: test_play_duration,
                            times: 1000,
                            gap: play_interval,
                        },
                    )
                    .await;

                let mut service = self.service.write().await;
                service.play_record(&alarm, result).await;
                continue;
            }

            match alarm_status {
                AlarmStatus::Canceled => {
                    info!("Alarm canceled, continue...");
                    continue;
                }
                AlarmStatus::Paused => {
                    info!("Alarm was paused, don't play, continue...");
                    if let Err(e) = alarm_tx.send(alarm).await {
                        error!("Failed to send alarm to cycle queue: {e}");
                    }
                    continue;
                }
                AlarmStatus::Playable => {
                    info!("Play alarm: {:?}", alarm);
                    let content = {
                        let service = self.service.read().await;
                        match posts_config.play_mode {
                            PlayMode::Music => PlayContent::Url(self.alarm_media_url.clone()),
                            PlayMode::Tts => {
                                let content = match service.get_alarm_content(&alarm) {
                                    Ok(content) => content,
                                    Err(e) => {
                                        error!(
                                            "Can't extract alarm content: {e}, don't play, skip!!!"
                                        );
                                        continue;
                                    }
                                };
                                PlayContent::Tts(content)
                            }
                        }
                    };

                    let duration = posts_config.duration;
                    let result = self
                        .play_alarm(
                            box_config,
                            posts_config,
                            content,
                            SpeechLoop {
                                duration,
                                times: 1,
                                gap: 10,
                            },
                        )
                        .await;
                    {
                        let mut service = self.service.write().await;
                        service.play_record(&alarm, result).await;
                    }
                    if let Err(e) = alarm_tx.send(alarm).await {
                        error!("Failed to send alarm to cycle queue: {e}");
                    }
                }
            }
        }
    }

    async fn play_test_alarm(
        &self,
        sbox: BoxConfig,
        posts: PostConfig,
        speech_loop: SpeechLoop,
    ) -> PlayResult {
        let id = Self::get_record_id();
        let filename = format!("{}.wav", id);

        let record = self
            .recorder
            .start(filename)
            .inspect_err(|e| error!("Recorder start failed: {e}"));
        let mut js = tokio::task::JoinSet::new();
        if sbox.enabled {
            let audio_data = self.test_media_buffer.clone();
            let volume = sbox.volume.clone();
            let sl = speech_loop.clone();
            js.spawn(async move {
                let mut sb = Soundbox::default();
                sb.play(audio_data, sl).await
            });
        }

        if !posts.device_ids.is_empty() {
            let device_ids = posts.device_ids;
            let content = PlayContent::Url(self.test_media_url.clone());
            let soundpost = self.soundpost.clone();
            js.spawn(async move { soundpost.play(device_ids, content, None, speech_loop).await });
        }

        let mut has_error = false;
        for result in js.join_all().await {
            if result.is_err() {
                has_error = true;
                break;
            }
        }

        if let Ok((stream, writer)) = record {
            let _ = self
                .recorder
                .stop(stream, writer)
                .inspect_err(|e| error!("Close record writer failed: {e}"));
        }

        PlayResult { id, has_error }
    }

    async fn play_alarm(
        &self,
        sbox: BoxConfig,
        posts: PostConfig,
        content: PlayContent,
        speech_loop: SpeechLoop,
    ) -> PlayResult {
        let id = Self::get_record_id();

        let filename = format!("{}.wav", id);
        let record = self
            .recorder
            .start(filename)
            .inspect_err(|e| error!("Recorder start failed: {e}"));

        let mut js = tokio::task::JoinSet::new();
        if sbox.enabled {
            let audio_data = self.alarm_media_buffer.clone();
            let volume = sbox.volume.clone();
            let sl = speech_loop.clone();
            js.spawn(async move {
                let mut sb = Soundbox::default();
                sb.play(audio_data, sl).await
            });
        }

        if !posts.device_ids.is_empty() {
            let device_ids = posts.device_ids.clone();
            let speed = match posts.play_mode {
                PlayMode::Tts => Some(posts.speed),
                PlayMode::Music => None,
            };
            let soundpost = self.soundpost.clone();
            js.spawn(async move {
                soundpost
                    .play(device_ids, content, speed, speech_loop)
                    .await
            });
        }

        let mut has_error = false;

        for result in js.join_all().await {
            if result.is_err() {
                has_error = true;
                break;
            }
        }

        if let Ok((stream, writer)) = record {
            let _ = self
                .recorder
                .stop(stream, writer)
                .inspect_err(|e| error!("Close record writer failed: {e}"));
        }

        PlayResult { id, has_error }
    }

    fn get_record_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod play_tests {
    use std::sync::Arc;

    use tokio::sync::RwLock;

    use crate::{
        config::PlayMode,
        player::{Soundpost, SpeechLoop},
        recorder::Recorder,
        service::{AlarmService, PostConfig},
    };

    use super::Play;

    #[ctor::ctor]
    fn init() {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }

    fn create_play() -> Play {
        let test_media_name = "please-calm-my-mind-125566.wav".to_string();
        let alarm_media_name = "new-edm-music-beet-mr-sandeep-rock-141616.mp3".to_string();
        let alarm_media_url =
            "http://192.168.77.14:8080/music/ed4b5d1af2ab7a1d921d16a857988620.mp3".to_string();
        let test_media_url =
            "http://192.168.77.14:8080/music/aabf0edb191d352cd535aa1f185d5209.mp3".to_string();
        let soundpost = Soundpost::new(
            "http://192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
        );

        let recorder = Recorder::new("/tmp".to_string(), "/tmp".to_string());
        let mut service = AlarmService::new(5, "zh_CN".to_string(), PlayMode::Music, 60, 2);
        service.set_soundposts(PostConfig {
            device_ids: vec![1, 2],
            speed: 50,
            duration: 30,
            play_mode: PlayMode::Music,
        });

        Play::new(
            alarm_media_name,
            test_media_name,
            alarm_media_url,
            test_media_url,
            soundpost,
            recorder,
            Arc::new(RwLock::new(service)),
        )
    }

    #[tokio::test]
    async fn test_play_test() {
        let play = create_play();
        let box_config = {
            let service = play.service.read().await;
            service.get_soundbox()
        };

        let posts_config = {
            let service = play.service.read().await;
            service.get_soundposts()
        };

        let test_play_duration = {
            let service = play.service.read().await;
            service.get_test_play_duration()
        };

        let play_interval = {
            let service = play.service.read().await;
            service.get_play_interval_secs()
        };

        play.play_test_alarm(
            box_config,
            posts_config,
            SpeechLoop {
                duration: test_play_duration,
                times: 1000,
                gap: play_interval,
            },
        )
        .await;
    }
}
