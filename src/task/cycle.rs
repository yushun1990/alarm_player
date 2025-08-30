use std::{collections::VecDeque, sync::Arc, time::Duration};

use tokio::{
    sync::{
        Mutex, Notify, RwLock,
        mpsc::{Receiver, Sender},
    },
    time::sleep,
};
use tracing::{error, info};

use crate::{
    model::Alarm,
    service::{AlarmService, AlarmStatus},
};

pub struct Cycle {
    check_interval: u64,
    alarms: Mutex<VecDeque<Alarm>>,
    service: Arc<RwLock<AlarmService>>,
}

impl Cycle {
    pub async fn init(check_interval: u64, service: Arc<RwLock<AlarmService>>) -> Self {
        let initial_alarms = {
            let service = service.read().await;
            service.get_alarms()
        };
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
                            self.push(alarm).await;
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
            alarms.pop_front()
        };

        let alarm = match alarm {
            Some(alarm) => alarm,
            None => {
                sleep(Duration::from_secs(self.check_interval)).await;
                return;
            }
        };

        let alarm_status = {
            let service = self.service.read().await;
            service.get_alarm_status(&alarm)
        };

        match alarm_status {
            AlarmStatus::Canceled => {
                info!("Alarm was canceled, try next one...");
                return;
            }
            _ => {
                sleep(Duration::from_secs(self.check_interval)).await;

                info!("Send alarm to player: {:?}", alarm);
                if let Err(e) = alarm_tx.send(alarm.clone()).await {
                    error!("Failed to send alarm to player: {e}");
                }
            }
        }
    }

    pub async fn push(&self, alarm: Alarm) {
        let mut alarms = self.alarms.lock().await;
        alarms.push_back(alarm);
    }
}
