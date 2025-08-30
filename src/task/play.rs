use std::sync::Arc;

use tokio::sync::{
    RwLock,
    mpsc::{Receiver, Sender},
};
use tracing::{error, info};

use crate::{
    Recorder,
    model::Alarm,
    player::{Soundbox, Soundpost},
    service::{AlarmService, AlarmStatus},
};

pub struct Play {
    soundpost: Soundpost,
    soundbox: Soundbox,
    recorder: Recorder,
    service: Arc<RwLock<AlarmService>>,
}

impl Play {
    pub fn new(
        api_host: String,
        api_login_token: String,
        media_name: String,
        storage_path: String,
        link_path: String,
        service: Arc<RwLock<AlarmService>>,
    ) -> Self {
        Self {
            soundpost: Soundpost::new(api_host, api_login_token),
            soundbox: Soundbox::new(media_name),
            recorder: Recorder::new(storage_path, link_path),
            service,
        }
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

    async fn play_alarm(&self, alarm: &Alarm) {}
}
