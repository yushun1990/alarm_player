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
    pub sender: Sender<Alarm>,
    pub receiver: Receiver<Alarm>,
    pub service: Arc<S>,
    pub cycle_tx: Sender<Alarm>,
    pub cycle_rx: Receiver<Alarm>,
}

impl<S> Player<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    pub fn new(size: usize, service: Arc<S>) -> Self {
        let (sender, receiver) = mpsc::channel::<Alarm>(size);
        let (cycle_tx, cycle_rx) = mpsc::channel::<Alarm>(10);
        Self {
            sender,
            receiver,
            service,
            cycle_tx,
            cycle_rx,
        }
    }

    async fn wait_for_finish(&self) {
        sleep(Duration::from_secs(5)).await;
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let mut alarm = match self.receiver.recv().await {
                Some(alarm) => alarm,
                None => {
                    info!("Play queue was closed, exit...");
                    return Ok(());
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

    pub fn sender(&self) -> Sender<Alarm> {
        self.sender.clone()
    }

    async fn play(&self, alarm: &Alarm) {
        info!("Play alarm: {:?}", alarm);
        self.wait_for_finish().await;
    }
}
