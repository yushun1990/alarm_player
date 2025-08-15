use std::{sync::Arc, time::Duration};

use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::sleep,
};
use tracing::{error, info};

use crate::{model::Alarm, service::AlarmService};

pub struct Player<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    service: Arc<S>,
}

impl<S> Player<S>
where
    S: AlarmService + 'static,
{
    pub fn new(service: Arc<S>) -> Self {
        Self { service }
    }

    async fn wait_for_finish(&self) {
        sleep(Duration::from_secs(5)).await;
    }

    pub async fn run(&self, alarm_tx: Sender<Alarm>, mut alarm_rx: Receiver<Alarm>) {
        loop {
            let mut alarm = match alarm_rx.recv().await {
                Some(alarm) => alarm,
                None => {
                    info!("Play queue was closed, exit...");
                    return;
                }
            };

            if !self.service.is_alarm_playable(&alarm).await {
                info!("Alarm not playable, skiped!");
                continue;
            }

            self.play(&alarm).await;

            if self.service.is_alarm_playable(&alarm).await && alarm.is_new {
                alarm.is_new = false;
                info!("Alarm 写入循环播放列表");
                if let Err(e) = alarm_tx.send(alarm).await {
                    error!("Failt send alarm to cycle queue: {e}");
                }
            }
        }
    }

    async fn play(&self, alarm: &Alarm) {
        info!("Play alarm: {:?}", alarm);
        self.wait_for_finish().await;
    }
}
