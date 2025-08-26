use std::sync::Arc;

use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::Serialize;
use tokio::sync::{
    RwLock,
    mpsc::{Receiver, Sender},
};
use tracing::{error, info};

use crate::{
    model::Alarm,
    service::{AlarmService, AlarmStatus, PlayContent},
};

pub struct Player<S: AlarmService> {
    api_addr: String,
    client: Client,
    service: Arc<RwLock<S>>,
}

#[derive(Clone, Serialize)]
pub struct SpeechRequest {
    pub device_ids: Vec<u32>,
    pub url: Option<String>,
    pub text: Option<String>,
    #[serde(rename = "loop")]
    pub play_loop: Option<PlayLoop>,
}

#[derive(Clone, Serialize)]
pub struct PlayLoop {
    pub duration: u64,
    pub times: u32,
    pub gap: u64,
}

impl<S: AlarmService> Player<S> {
    pub fn new(
        api_addr: String,
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
            api_addr,
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

    async fn play_test_alarm(&self, alarm: &Alarm) {}

    async fn play_alarm(&self, alarm: &Alarm) {
        let request = self.build_speech_request(alarm, None).await;
        tokio::join!(self.soundpost_play(request), self.soundbox_play());
    }

    async fn soundpost_play(&self, data: SpeechRequest) {
        let result = match self
            .client
            .delete(format!(
                "{}/v1/speech?device_ids={}",
                self.api_addr,
                data.device_ids
                    .clone()
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>()
                    .join(",")
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

        let result = match self
            .client
            .post(format!("{}/v1/speech", self.api_addr))
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
            play_loop,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::RwLock;

    use crate::{model::Alarm, service::DefaultAlarmServiceImpl, task::Player};

    #[tokio::test]
    async fn test_play() {
        tracing_subscriber::fmt().with_env_filter("info").init();
        let mut service = DefaultAlarmServiceImpl::default();
        service.alarm_media_url =
            "http://192.168.77.14:8080/music/ed4b5d1af2ab7a1d921d16a857988620.mp3".into();
        service.soundposts = vec![1, 2];
        service.alarm_play_mode = "music".into();
        let player = Player::new(
            "http://192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
            Arc::new(RwLock::new(service)),
        )
        .unwrap();

        let alarm = Alarm::default();

        player.play_alarm(&alarm).await;
    }
}
