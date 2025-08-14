use std::{sync::Arc, time::Duration};

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::sleep,
};
use tracing::{error, info};

use crate::{model::Alarm, service::AlarmService};

pub struct Player<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    player_rx: Receiver<Alarm>,
    cycle_tx: Sender<Alarm>,
    service: Arc<S>,
}

impl<S> Player<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    pub fn new(player_rx: Receiver<Alarm>, cycle_tx: Sender<Alarm>, service: Arc<S>) -> Self {
        Self {
            player_rx,
            cycle_tx,
            service,
        }
    }

    async fn wait_for_finish(&self) {
        sleep(Duration::from_secs(5)).await;
    }

    pub async fn run(&mut self) {
        loop {
            let mut alarm = match self.player_rx.recv().await {
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
                if let Err(e) = self.cycle_tx.send(alarm).await {
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
