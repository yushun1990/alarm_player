use std::{fs::File, sync::Arc};

use rodio::{Decoder, Source};
use tokio::sync::{
    Mutex, RwLock,
    mpsc::{self, Receiver, Sender},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    Recorder,
    config::PlayMode,
    model::Alarm,
    player::{
        Buffer, PlayCancelType, PlayContent, PlayResultType, Soundbox, Soundpost, SpeechLoop,
    },
    service::{AlarmService, AlarmStatus, BoxConfig, PlayResult, PostConfig},
};

#[derive(Default, Clone)]
pub struct Tx {
    test_tx: Option<Sender<PlayCancelType>>,
    alarm_tx: Option<Sender<PlayCancelType>>,
}

#[derive(Clone)]
pub struct Play {
    alarm_media_buffer: Buffer,
    test_media_buffer: Buffer,
    alarm_media_url: String,
    test_media_url: String,
    alarm_min_duration: u64,
    test_min_duration: u64,
    speech_min_duration: u64,
    play_mode: PlayMode,
    soundpost: Soundpost,
    recorder: Recorder,
    service: Arc<RwLock<AlarmService>>,
    box_tx: Arc<Mutex<Tx>>,
    post_tx: Arc<Mutex<Tx>>,
}

impl Play {
    pub fn new(
        alarm_media_path: String,
        test_media_path: String,
        alarm_media_url: String,
        test_media_url: String,
        alarm_min_duration: u64,
        test_min_duration: u64,
        speech_min_duration: u64,
        play_mode: PlayMode,
        soundpost: Soundpost,
        recorder: Recorder,
        service: Arc<RwLock<AlarmService>>,
    ) -> Self {
        Self {
            alarm_media_buffer: Self::get_buffer(alarm_media_path),
            test_media_buffer: Self::get_buffer(test_media_path),
            alarm_media_url,
            test_media_url,
            alarm_min_duration,
            test_min_duration,
            speech_min_duration,
            play_mode,
            soundpost,
            recorder,
            service,
            box_tx: Default::default(),
            post_tx: Default::default(),
        }
    }

    fn get_buffer(path: String) -> Buffer {
        let file = File::open(path).unwrap();
        Decoder::try_from(file).unwrap().buffered()
    }

    async fn cancel_test(&self, cancel_type: &PlayCancelType) {
        {
            let mut box_tx = self.box_tx.lock().await;
            if let Some(tx) = box_tx.test_tx.take() {
                info!("Cancel box test alarm playing...");
                if let Err(e) = tx.send(cancel_type.clone()).await {
                    warn!("Failed for signaling by box.test_tx: {:?}", e);
                }
            }
        }

        {
            let mut post_tx = self.post_tx.lock().await;
            if let Some(tx) = post_tx.test_tx.take() {
                info!("Cancel post test alarm playing...");
                if let Err(e) = tx.send(cancel_type.clone()).await {
                    warn!("Failed for signaling by post.test_tx: {:?}", e);
                }
            }
        }
    }

    async fn cancel_alarm(&self, cancel_type: &PlayCancelType) {
        {
            let mut box_tx = self.box_tx.lock().await;
            if let Some(tx) = box_tx.alarm_tx.take() {
                info!("Cancel box alarm playing...");
                if let Err(e) = tx.send(cancel_type.clone()).await {
                    warn!("Failed for signaling by box.alarm_tx: {:?}", e);
                }
            }
        }

        {
            let mut post_tx = self.post_tx.lock().await;
            if let Some(tx) = post_tx.alarm_tx.take() {
                info!("Cancel post alarm playing...");
                if let Err(e) = tx.send(cancel_type.clone()).await {
                    warn!("Failed for signaling by post.alarm_tx: {:?}", e);
                }
            }
        }
    }

    async fn cancel(&self, cancel_type: PlayCancelType) {
        match cancel_type {
            PlayCancelType::AlarmArrived => {
                self.cancel_test(&cancel_type).await;
            }
            PlayCancelType::Terminated => {
                self.cancel_test(&cancel_type).await;
                self.cancel_alarm(&cancel_type).await;
            }
        }
    }

    pub async fn cancel_test_play(&self) {
        self.cancel(PlayCancelType::AlarmArrived).await;
    }

    pub async fn terminate_play(&self) {
        self.cancel(PlayCancelType::Terminated).await;
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
                    .play_test(
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
                    let (content, duration) = {
                        let service = self.service.read().await;
                        match self.play_mode {
                            PlayMode::Music => (
                                PlayContent::Url(self.alarm_media_url.clone()),
                                self.alarm_min_duration,
                            ),
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
                                (PlayContent::Tts(content), self.speech_min_duration)
                            }
                        }
                    };

                    let result = self
                        .play_alarm(
                            box_config,
                            posts_config,
                            content,
                            SpeechLoop {
                                duration,
                                times: 1,
                                gap: 2,
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

    async fn play_test(
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
            let sl = speech_loop.clone();
            let duration = self.test_min_duration;
            let (tx, rx) = mpsc::channel(1);
            {
                let mut box_tx = self.box_tx.lock().await;
                box_tx.test_tx = Some(tx);
            }
            js.spawn(async move {
                let sb = Soundbox::new(duration);
                sb.play(audio_data, sl, rx).await
            });
        }

        if !posts.device_ids.is_empty() {
            let device_ids = posts.device_ids;
            let content = PlayContent::Url(self.test_media_url.clone());
            let soundpost = self.soundpost.clone();
            let (tx, rx) = mpsc::channel(1);
            {
                let mut post_tx = self.post_tx.lock().await;
                post_tx.test_tx = Some(tx);
            }
            js.spawn(async move {
                soundpost
                    .play(device_ids, content, None, speech_loop, rx)
                    .await
            });
        }

        let mut has_error = false;

        debug!("waitting for playing task to complete...");
        let mut result_type = PlayResultType::Normal;
        while let Some(res) = js.join_next().await {
            match res {
                Ok(Ok(t)) => {
                    result_type = t;
                }
                Ok(Err(e)) => {
                    error!("Task failed: {e}");
                    has_error = true;
                }
                Err(e) => {
                    error!("Task failed: {e}");
                    has_error = true;
                }
            }
        }

        debug!("playing task finished, write record...");

        if let Ok((stream, writer)) = record {
            let _ = self
                .recorder
                .stop(stream, writer)
                .inspect_err(|e| error!("Close record writer failed: {e}"));
        }

        debug!("Recorder stopped, playing task finished!");

        PlayResult {
            id,
            has_error,
            result_type,
        }
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
            let sl = speech_loop.clone();
            let duration = self.alarm_min_duration;
            let (tx, rx) = mpsc::channel(1);
            {
                let mut box_tx = self.box_tx.lock().await;
                box_tx.alarm_tx = Some(tx);
            }
            js.spawn(async move {
                let sb = Soundbox::new(duration);
                sb.play(audio_data, sl, rx).await
            });
        }

        if !posts.device_ids.is_empty() {
            let device_ids = posts.device_ids.clone();
            let speed = match self.play_mode {
                PlayMode::Tts => Some(posts.speed),
                PlayMode::Music => None,
            };
            let (tx, rx) = mpsc::channel(1);
            {
                let mut post_tx = self.post_tx.lock().await;
                post_tx.alarm_tx = Some(tx);
            }
            let soundpost = self.soundpost.clone();
            js.spawn(async move {
                soundpost
                    .play(device_ids, content, speed, speech_loop, rx)
                    .await
            });
        }

        let mut has_error = false;
        let mut result_type = PlayResultType::Normal;
        debug!("waitting for playing task to complete...");
        while let Some(res) = js.join_next().await {
            match res {
                Ok(Ok(t)) => {
                    result_type = t;
                }
                Ok(Err(e)) => {
                    error!("Task failed: {e}");
                    has_error = true;
                }
                Err(e) => {
                    error!("Task failed: {e}");
                    has_error = true;
                }
            }
        }

        if let Ok((stream, writer)) = record {
            let _ = self
                .recorder
                .stop(stream, writer)
                .inspect_err(|e| error!("Close record writer failed: {e}"));
        }

        PlayResult {
            id,
            has_error,
            result_type,
        }
    }

    fn get_record_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod play_tests {
    use std::sync::Arc;

    use tokio::sync::RwLock;
    use tracing::info;

    use crate::{
        config::{DbConfig, PlayMode},
        player::{PlayContent, Soundpost, SpeechLoop},
        recorder::Recorder,
        service::{AlarmService, PostConfig},
    };

    use super::Play;

    fn create_play() -> Play {
        let test_media_name = "resource/please-calm-my-mind-125566.wav".to_string();
        let alarm_media_name = "resource/new-edm-music-beet-mr-sandeep-rock-141616.mp3".to_string();
        let alarm_media_url =
            "http://192.168.77.14:8080/music/ed4b5d1af2ab7a1d921d16a857988620.mp3".to_string();
        let test_media_url =
            "http://192.168.77.14:8080/music/aabf0edb191d352cd535aa1f185d5209.mp3".to_string();
        let soundpost = Soundpost::new(
            "192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
        );

        let recorder = Recorder::new("/tmp".to_string(), "/tmp".to_string());
        let mut service = AlarmService::new(
            5,
            "zh_CN".to_string(),
            60,
            2,
            "http://192.168.77.34/api/IB/alarm-info/current-alarm-info-page-list-with-no-auth"
                .to_string(),
            DbConfig::default(),
        );
        service.set_soundposts(PostConfig {
            device_ids: vec![1, 2],
            speed: 1,
        });

        Play::new(
            alarm_media_name,
            test_media_name,
            alarm_media_url,
            test_media_url,
            30,
            30,
            10,
            PlayMode::Music,
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

        play.play_test(
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

    #[tokio::test]
    async fn test_play_alarm() {
        let mut play = create_play();
        play.play_mode = PlayMode::Tts;
        let box_config = {
            let service = play.service.read().await;
            service.get_soundbox()
        };

        let posts_config = {
            let service = play.service.read().await;
            service.get_soundposts()
        };

        info!("post: {:?}", posts_config);

        let play_interval = {
            let service = play.service.read().await;
            service.get_play_interval_secs()
        };

        play.play_alarm(
            box_config,
            posts_config,
            PlayContent::Tts("[9999] 温度传感器09故障 状态:报警".to_string()),
            SpeechLoop {
                duration: 10,
                times: 1,
                gap: play_interval,
            },
        )
        .await;
    }
}
