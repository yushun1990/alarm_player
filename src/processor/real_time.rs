use std::{sync::Arc, time::Duration};

use time::OffsetDateTime;
use tokio::sync::mpsc::{Receiver, Sender, error::TryRecvError};
use tracing::{error, info};

use crate::{model::Alarm, service::AlarmService};

#[derive(Clone)]
pub struct RealTime<S>
where
    S: AlarmService + 'static,
{
    test_alarm: Option<Alarm>,
    service: Arc<S>,
    check_interval: u64,
}

impl<S> RealTime<S>
where
    S: AlarmService + 'static,
{
    pub fn new(check_interval: u64, service: Arc<S>) -> Self {
        Self {
            test_alarm: None,
            service,
            check_interval,
        }
    }

    pub async fn run(&mut self, alarm_tx: Sender<Alarm>, mut alarm_rx: Receiver<Alarm>) {
        loop {
            let alarm = match alarm_rx.try_recv() {
                Ok(alarm) => Some(alarm),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    return;
                }
            };

            if let Some(alarm) = alarm {
                self.process(&alarm_tx, alarm).await;
                continue;
            }

            if let Some(test_alarm) = self.test_alarm.clone() {
                if self.service.is_ongoing_alarm_exist().await {
                    info!("There alarms in playing, wait for it finished...");
                    self.test_alarm = None;
                    self.test_alarm_retry(&alarm_tx, test_alarm).await;

                    continue;
                }

                Self::alarm_to_play(&alarm_tx, test_alarm).await;
                continue;
            }

            info!("No real alarm, and no test alarm, wait for new alarm...");
            let alarm = match alarm_rx.recv().await {
                Some(alarm) => alarm,
                None => {
                    info!("Real time queue closed, exit...");
                    return;
                }
            };

            self.process(&alarm_tx, alarm).await;
        }
    }

    /// 测试报警重新写入实时队列
    async fn test_alarm_retry(&self, sender: &Sender<Alarm>, alarm: Alarm) {
        let next_check_time = OffsetDateTime::now_utc()
            .saturating_add(time::Duration::seconds(self.check_interval as i64));
        if let Some(next_fire_time) = self.service.next_fire_time().await {
            if next_check_time >= next_fire_time {
                info!(
                    "Next-check-time({:?}) > Next-fire-time({:?}); canceled!",
                    next_check_time, next_fire_time
                );
                return;
            }
        }

        info!("Sleep {} secs...", self.check_interval);
        tokio::time::sleep(Duration::from_secs(self.check_interval)).await;

        if let Err(e) = sender.send(alarm).await {
            error!("Failed re-entry alarm to real queue: {e}");
        }
    }

    /// 处理报警
    async fn process(&mut self, alarm_tx: &Sender<Alarm>, alarm: Alarm) {
        if alarm.alarm_type == crate::contract::ALARM_TYPE_TEST {
            info!("Received test alarm: {:?}, check for nexted one...", alarm);
            self.test_alarm = Some(alarm);
            return;
        }

        info!("Received real alarm: {:?} ...", alarm);

        if !self.service.is_alarm_playable(&alarm).await {
            info!("Alarm: {:?} not playable, Skiped!", alarm);
            return;
        }

        let alarm_time = match alarm.received_time {
            Some(received_time) => received_time,
            None => alarm.timestamp,
        };

        let play_time = alarm_time.saturating_add(self.service.get_play_delay().await);
        let current_time = OffsetDateTime::now_utc();
        if play_time > OffsetDateTime::now_utc() {
            let delay =
                Duration::from_millis((play_time - current_time).whole_milliseconds() as u64);
            info!("Delay: {:?}", delay);
            tokio::time::sleep(delay).await;
        }
        info!("Check alarm palyablility after delay...");
        if !self.service.is_alarm_playable(&alarm).await {
            info!("Alarm: {:?} not playable, Skiped!", alarm);
            return;
        }

        Self::alarm_to_play(alarm_tx, alarm).await;
    }

    async fn alarm_to_play(alarm_tx: &Sender<Alarm>, alarm: Alarm) {
        info!("Send alarm: {:?} to play queue...", alarm);
        if let Err(e) = alarm_tx.send(alarm).await {
            error!("Failed to send alarm to play queue: {}", e);
        }
    }
}
