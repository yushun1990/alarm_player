use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{
        RwLock,
        mpsc::{Receiver, Sender},
    },
    time::sleep,
};
use tracing::{error, info};

use crate::{
    model::Alarm,
    service::{AlarmService, AlarmStatus},
};

pub struct Player<S: AlarmService> {
    service: Arc<RwLock<S>>,
}

impl<S: AlarmService> Player<S> {
    pub fn new(service: Arc<RwLock<S>>) -> Self {
        Self { service }
    }

    async fn wait_for_finish(&self) {
        sleep(Duration::from_secs(5)).await;
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
        self.wait_for_finish().await;
    }
}
