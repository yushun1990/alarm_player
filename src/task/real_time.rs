use std::{sync::Arc, time::Duration};

use time::OffsetDateTime;
use tokio::{
    sync::{
        RwLock,
        mpsc::{Receiver, Sender, error::TryRecvError},
    },
    task::JoinHandle,
};
use tracing::{error, info};

use crate::{model::Alarm, service::AlarmService};

pub struct RealTime<S: AlarmService> {
    test_alarm: Option<Alarm>,
    service: Arc<RwLock<S>>,
    check_interval: u64,
    sleep_task_handle: Option<JoinHandle<()>>,
}

impl<S: AlarmService> RealTime<S> {
    pub fn new(check_interval: u64, service: Arc<RwLock<S>>) -> Self {
        Self {
            test_alarm: None,
            service,
            check_interval,
            sleep_task_handle: None,
        }
    }

    pub async fn run(&mut self, alarm_tx: Sender<Alarm>, mut alarm_rx: Receiver<Alarm>) {
        loop {
            let alarm = match alarm_rx.try_recv() {
                Ok(alarm) => Some(alarm),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    if let Some(handler) = &self.sleep_task_handle {
                        handler.abort();
                    }
                    return;
                }
            };

            if let Some(alarm) = alarm {
                self.process(&alarm_tx, alarm).await;
                continue;
            }

            if let Some(test_alarm) = self.test_alarm.clone() {
                let is_ongoing_alarm_exist = {
                    let service = self.service.read().await;
                    service.is_ongoing_alarm_exist()
                };

                if is_ongoing_alarm_exist {
                    info!("There alarms in playing, wait for it finished...");
                    self.test_alarm = None;
                    self.test_alarm_retry(&alarm_tx, test_alarm).await;

                    continue;
                }

                Self::alarm_to_play(&alarm_tx, test_alarm).await;
                self.test_alarm = None;
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
    async fn test_alarm_retry(&mut self, sender: &Sender<Alarm>, alarm: Alarm) {
        let next_check_time = OffsetDateTime::now_utc()
            .saturating_add(time::Duration::seconds(self.check_interval as i64));

        let next_fire_time = {
            let service = self.service.read().await;
            service.next_fire_time()
        };

        if let Some(next_fire_time) = next_fire_time {
            if next_check_time >= next_fire_time {
                info!(
                    "Next-check-time({:?}) > Next-fire-time({:?}); canceled!",
                    next_check_time, next_fire_time
                );
                return;
            }
        }

        // 新任務中等待進行中的報警結束
        let interval = self.check_interval;
        let sender = sender.clone();
        let alarm = alarm.clone();
        self.sleep_task_handle = Some(tokio::spawn(async move {
            info!("Sleep {} secs...", interval);
            tokio::time::sleep(Duration::from_secs(interval)).await;

            // 測試報警重新寫入實時隊列
            if let Err(e) = sender.send(alarm).await {
                error!("Failed re-entry alarm to real queue: {e}");
            }
        }));
    }

    /// 处理报警
    async fn process(&mut self, alarm_tx: &Sender<Alarm>, alarm: Alarm) {
        if alarm.is_test {
            // 收到測試報警則繼續接收下一個，直到收到不是測試報警爲止
            info!("Received test alarm: {:?}, check for nexted one...", alarm);
            self.test_alarm = Some(alarm);
            return;
        }

        info!("Received real alarm: {:?} ...", alarm);
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
            return;
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
            info!("Delay: {:?}", delay);
            tokio::time::sleep(delay).await;
        }
        info!("Check alarm palyablility after delay...");

        Self::alarm_to_play(alarm_tx, alarm).await;
    }

    async fn alarm_to_play(alarm_tx: &Sender<Alarm>, alarm: Alarm) {
        info!("Send alarm: {:?} to play queue...", alarm);
        if let Err(e) = alarm_tx.send(alarm).await {
            error!("Failed to send alarm to play queue: {}", e);
        }
    }
}
