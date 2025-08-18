use std::{collections::VecDeque, sync::Arc, time::Duration};

use tokio::{
    sync::{
        Mutex, Notify,
        mpsc::{Receiver, Sender},
    },
    time::sleep,
};
use tracing::{error, info};

use crate::{model::Alarm, service::AlarmService};

pub struct Cycle<S>
where
    S: AlarmService + 'static,
{
    check_interval: u64,
    alarms: Mutex<VecDeque<Alarm>>,
    service: Arc<S>,
}

impl<S> Cycle<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    pub async fn init(check_interval: u64, service: Arc<S>) -> Self {
        let initial_alarms = service.get_alarms().await;
        Self {
            check_interval,
            alarms: Mutex::new(VecDeque::from(initial_alarms)),
            service,
        }
    }

    pub async fn run(
        &self,
        alarm_tx: Sender<Alarm>,
        mut alarm_rx: Receiver<Alarm>,
        shutdown: Arc<Notify>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    info!("Shutdown cycle processor...");
                    return;
                }
                alarm = alarm_rx.recv() => {
                    match alarm {
                        Some(alarm) => {
                            self.alarms.lock().await.push_back(alarm);
                            info!("Received new alarm, and added to the cycle-play queue!");
                        },
                        None => {
                            info!("Cycle alarm channel closed, exit!");
                            return;
                        }
                    };
                },
                _ = self.play(&alarm_tx), if !self.alarms.lock().await.is_empty() => {}
            }
        }
    }

    pub async fn play(&self, alarm_tx: &Sender<Alarm>) {
        let alarm = {
            let mut alarms = self.alarms.lock().await;
            alarms.pop_front().unwrap()
        };

        if self.service.is_cycle_alarm_playable(&alarm).await {
            sleep(Duration::from_secs(self.check_interval)).await;

            info!("Send alarm to player: {:?}", alarm);
            if let Err(e) = alarm_tx.send(alarm.clone()).await {
                error!("Failed to send alarm to player: {e}");
            }

            // push alarm back to alarms
            self.alarms.lock().await.push_back(alarm);
        }
    }
}
