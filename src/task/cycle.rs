use std::{collections::VecDeque, time::Duration};

use tokio::{
    sync::{
        Mutex,
        mpsc::{Receiver, Sender},
    },
    time::sleep,
};
use tracing::{error, info};

use crate::{Service, model::Alarm, service::AlarmStatus};

pub struct Cycle {
    check_interval: u64,
    alarms: Mutex<VecDeque<Alarm>>,
    service: Service,
}

impl Cycle {
    pub async fn init(check_interval: u64, service: Service) -> Self {
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

    pub async fn run(&self, tx: Sender<Alarm>, mut rx: Receiver<Alarm>) {
        loop {
            tokio::select! {
                alarm = rx.recv() => {
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
                _ = self.play(&tx), if !self.alarms.lock().await.is_empty() => {}
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
        for a in alarms.iter() {
            if Self::get_alarm_set_key(&alarm) == Self::get_alarm_set_key(&a) {
                return;
            }
        }
        alarms.push_back(alarm);
    }

    fn get_alarm_set_key(alarm: &Alarm) -> String {
        format!("{}_{}", alarm.house_code, alarm.target_name)
    }
}
