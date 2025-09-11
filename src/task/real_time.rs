use std::time::Duration;

use time::OffsetDateTime;
use tokio::sync::mpsc::{Receiver, Sender, error::TryRecvError};
use tracing::{error, info};

use crate::{Service, model::Alarm};

pub struct RealTime {
    service: Service,
}

impl RealTime {
    pub fn new(service: Service) -> Self {
        Self { service }
    }

    pub async fn run(
        &mut self,
        tx: Sender<Alarm>,
        mut act_rx: Receiver<Alarm>,
        mut test_rx: Receiver<Alarm>,
    ) {
        loop {
            tokio::select! {
                alarm = act_rx.recv() => {
                    info!("Received real alarm: {:?} ...", alarm);
                    if alarm.is_none() {
                        info!("Act channel closed, exit realtime run ...");
                        return;
                    }
                    let alarm = alarm.unwrap();
                    let alarm_time = match alarm.received_time {
                        Some(received_time) => received_time,
                        None => alarm.timestamp,
                    };
                    let is_new_alarm = {
                        let mut service = self.service.write().await;
                        service.set_alarm(alarm.clone())
                    };

                    if !is_new_alarm {
                        info!("Alarm: {:?} isn't new alarm, skipped", alarm);
                        continue;
                    }

                    let play_delay = {
                        let service = self.service.read().await;
                        service.get_play_delay()
                    };

                    let play_time = alarm_time.saturating_add(play_delay);
                    let current_time = OffsetDateTime::now_utc();
                    if play_time > OffsetDateTime::now_utc() {
                        let delay =
                            Duration::from_millis((play_time - current_time).whole_milliseconds() as u64);
                        info!("Delay: {:?} to play...", delay);
                        tokio::time::sleep(delay).await;
                    }
                    Self::alarm_to_play(&tx, alarm).await;

                },
                alarm = test_rx.recv(), if act_rx.is_empty() && !self.service.read().await.is_ongoing_alarm_exist() => {
                    if alarm.is_none() {
                        info!("Test channel closed, exit realtime run ...");
                        return;
                    }
                    let mut alarm = alarm.unwrap();
                    let alarm = loop {
                        match test_rx.try_recv() {
                            Ok(next) => {
                                alarm = next;
                            },
                            Err(TryRecvError::Empty) => break alarm,
                            Err(TryRecvError::Disconnected) => {
                                info!("Test channel closed, exit realtime run ...");
                                return;
                            }
                        }
                    };

                    Self::alarm_to_play(&tx, alarm).await;
                }
            }
        }
    }

    async fn alarm_to_play(tx: &Sender<Alarm>, alarm: Alarm) {
        info!("Send alarm: {:?} to realtime play queue...", alarm);
        if let Err(e) = tx.send(alarm).await {
            error!("Failed to send alarm to play queue: {}", e);
        }
    }
}
