mod player;

use std::sync::Arc;

use reqwest::{
    Client, StatusCode,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{
    RwLock,
    mpsc::{Receiver, Sender},
};
use tracing::{error, info};

use crate::{
    model::Alarm,
    service::{AlarmService, AlarmStatus, PlayContent},
};

pub struct Play<S: AlarmService> {
    api_host: String,
    client: Client,
    service: Arc<RwLock<S>>,
}

#[derive(Clone, Serialize)]
pub struct SpeechRequest {
    pub device_ids: Vec<u32>,
    pub url: Option<String>,
    pub text: Option<String>,
    pub volume: u8,
    #[serde(rename = "loop")]
    pub play_loop: Option<PlayLoop>,
}

#[derive(Clone, Serialize)]
pub struct PlayLoop {
    pub duration: u64,
    pub times: u32,
    pub gap: u64,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct PlayInfo {
    pub code: u16,
    pub message: String,
    #[serde(default)]
    pub data: Vec<PlayInfoData>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct PlayInfoData {
    pub code: u16,
    pub message: String,
    pub body: String,
    pub id: u32,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct PlayStatus {
    pub code: u16,
    pub message: String,
    pub data: Option<PlayStatusData>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct PlayStatusData {
    #[serde(default)]
    #[serde(rename = "speech")]
    pub playing: bool,
}

impl<S: AlarmService> Play<S> {
    pub fn new(
        api_host: String,
        api_login_token: String,
        service: Arc<RwLock<S>>,
    ) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(format!("Bearer {api_login_token}").as_str())?,
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            api_host,
            client,
            service,
        })
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

            if alarm.is_test {
                // 測試報警，直接播放
                info!("Play test alarm: {:?}", alarm);
                self.play(&alarm).await;
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
                    self.play(&alarm).await;
                    if let Err(e) = alarm_tx.send(alarm).await {
                        error!("Failed to send alarm to cycle queue: {e}");
                    }
                }
            }
        }
    }

    async fn play(&self, alarm: &Alarm) {
        info!("Play alarm: {:?}", alarm);
        if alarm.is_test {
            self.play_test_alarm(alarm).await;
        } else {
            self.play_alarm(alarm).await;
        }
    }

    async fn play_test_alarm(&self, _alarm: &Alarm) {}

    async fn play_alarm(&self, alarm: &Alarm) {
        let request = self.build_speech_request(alarm, None).await;
        tokio::join!(self.soundpost_play(&request), self.soundbox_play());
    }

    fn get_device_id_params(device_ids: &Vec<u32>) -> String {
        device_ids
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(",")
    }

    async fn soundpost_cancel(&self, device_ids: &Vec<u32>) {
        let result = match self
            .client
            .delete(format!(
                "http://{}/v1/speech?device_ids={}",
                self.api_host,
                Self::get_device_id_params(device_ids)
            ))
            .send()
            .await
        {
            Ok(res) => match res.text().await {
                Ok(res) => res,
                Err(e) => {
                    error!("Cancel play failed: {e}");
                    return;
                }
            },
            Err(e) => {
                error!("Soundpost cancel play failed: {e}");
                return;
            }
        };

        info!("Cancel result: {result}");
    }

    // 读取 音柱播放状态， 任何错误或有一个处于播放中都当作未播放完成处理
    async fn get_soundpost_play_status(&self, device_ids: &Vec<u32>) -> bool {
        let result: PlayInfo = match self
            .client
            .get(format!(
                "http://{}/v1/play_status?device_ids={}",
                self.api_host,
                Self::get_device_id_params(device_ids)
            ))
            .send()
            .await
        {
            Ok(res) => match res.json().await {
                Ok(res) => res,
                Err(e) => {
                    error!("Soundpost play status deserial failed: {e}");
                    return false;
                }
            },
            Err(e) => {
                error!("Failed to get soundpost play status: {e}");
                return false;
            }
        };

        info!("Soundpost play status: {:?}", result);

        if result.code != StatusCode::OK {
            error!("Get play status failed: {}", result.message);
            return false;
        }

        for pid in result.data {
            if pid.code != StatusCode::OK {
                error!(
                    "Get play status failed for device:{}, err: {}",
                    pid.id, pid.message
                );
                return false;
            }

            match serde_json::from_str::<PlayStatus>(pid.body.as_str()) {
                Ok(status) => {
                    if let Some(s) = status.data {
                        if s.playing {
                            return false;
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Soundpost status body deserialize failed for device: {}, err: {}",
                        pid.id, e
                    );
                    return false;
                }
            }
        }

        return true;
    }

    async fn soundpost_play(&self, data: &SpeechRequest) {
        self.soundpost_cancel(&data.device_ids).await;
        let result = match self
            .client
            .post(format!("http://{}/v1/speech", self.api_host))
            .json(&data)
            .send()
            .await
        {
            Ok(res) => match res.text().await {
                Ok(res) => res,
                Err(e) => {
                    error!("Soundpost play failed: {e}");
                    return;
                }
            },
            Err(e) => {
                error!("Soundpost play failed: {e}");
                return;
            }
        };

        info!("Soundpost play result: {result}");
    }

    async fn soundbox_play(&self) {}

    async fn build_speech_request(
        &self,
        alarm: &Alarm,
        play_loop: Option<PlayLoop>,
    ) -> SpeechRequest {
        let (url, text) = {
            let service = self.service.read().await;
            match service.get_alarm_content(alarm) {
                PlayContent::Url(url) => (Some(url), None),
                PlayContent::TTS(tts) => (None, Some(tts)),
            }
        };

        let device_ids = {
            let service = self.service.read().await;
            service.get_soundposts()
        };

        SpeechRequest {
            device_ids,
            url,
            text,
            volume: 100,
            play_loop,
        }
    }
}

#[cfg(test)]
mod player_tests {
    use std::{sync::Arc, time::Duration};

    use tokio::sync::RwLock;

    use crate::{model::Alarm, service::DefaultAlarmServiceImpl, task::Play};

    #[ctor::ctor]
    fn init() {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }

    #[tokio::test]
    async fn test_play() {
        let mut service = DefaultAlarmServiceImpl::default();
        service.alarm_media_url =
            "http://192.168.77.14:8080/music/ed4b5d1af2ab7a1d921d16a857988620.mp3".into();
        service.soundposts = vec![1, 2];
        service.alarm_play_mode = "music".into();
        let player = Play::new(
            "192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
            Arc::new(RwLock::new(service)),
        )
        .unwrap();

        let alarm = Alarm::default();

        player.play_alarm(&alarm).await;
    }

    #[tokio::test]
    async fn test_play_status() {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let service = DefaultAlarmServiceImpl::default();
        let player = Play::new(
            "192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
            Arc::new(RwLock::new(service)),
        )
        .unwrap();

        assert_eq!(player.get_soundpost_play_status(&vec![1, 2]).await, false);
    }

    #[tokio::test]
    async fn test_cancel() {
        let service = DefaultAlarmServiceImpl::default();
        let player = Play::new(
            "192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
            Arc::new(RwLock::new(service)),
        )
        .unwrap();
        player.soundpost_cancel(&vec![1, 2]).await;
    }
}
