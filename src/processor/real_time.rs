use std::{sync::Arc, time::Duration};

use time::OffsetDateTime;
use tokio::sync::mpsc::{Receiver, Sender, channel, error::TryRecvError};
use tracing::{error, info};

use crate::{model::Alarm, service::AlarmService};

pub struct RealTime<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    sender: Sender<Alarm>,
    receiver: Receiver<Alarm>,
    play_sender: Sender<Alarm>,
    test_alarm: Option<Alarm>,
    service: Arc<S>,
    check_interval: u64,
}

impl<S> RealTime<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    pub fn new(
        size: usize,
        check_interval: u64,
        play_sender: Sender<Alarm>,
        service: Arc<S>,
    ) -> Self {
        let (sender, receiver) = channel::<Alarm>(size);

        Self {
            sender,
            receiver,
            play_sender,
            test_alarm: None,
            service,
            check_interval,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let alarm = match self.receiver.try_recv() {
                Ok(alarm) => Some(alarm),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return Ok(()),
            };

            if let Some(alarm) = alarm {
                self.process(alarm).await;
                continue;
            }

            if let Some(test_alarm) = self.test_alarm.clone() {
                if self.service.is_ongoing_alarm_exist().await {
                    info!("There alarms in playing, wait for it finished...");
                    self.test_alarm = None;
                    let sender = self.sender.clone();
                    let check_interval = self.check_interval;
                    let service = self.service.clone();
                    tokio::spawn(async move {
                        Self::test_alarm_retry(service, sender, check_interval, test_alarm).await;
                    });

                    continue;
                }

                self.alarm_to_play(test_alarm).await;
                continue;
            }

            info!("No real alarm, and no test alarm, wait for new alarm...");
            let alarm = match self.receiver.recv().await {
                Some(alarm) => alarm,
                None => {
                    info!("Real time queue closed, exit...");
                    return Ok(());
                }
            };

            self.process(alarm).await;
        }
    }

    /// 测试报警重新写入实时队列
    async fn test_alarm_retry(
        service: Arc<S>,
        sender: Sender<Alarm>,
        check_interval: u64,
        alarm: Alarm,
    ) {
        // TODO: if now() + act >= next_fire_time: exit
        // TODO: sleep act
        // TODO: re-entry
        let next_check_time = OffsetDateTime::now_utc()
            .saturating_add(time::Duration::seconds(check_interval as i64));
        let next_fire_time = service.next_fire_time().await;

        if next_check_time >= next_fire_time {
            info!(
                "Next-check-time({:?}) > Next-fire-time({:?}); canceled!",
                next_check_time, next_fire_time
            );
            return;
        }

        info!("Sleep {} secs...", check_interval);
        tokio::time::sleep(Duration::from_secs(check_interval)).await;

        if let Err(e) = sender.send(alarm).await {
            error!("Failed re-entry alarm to real queue: {e}");
        }
    }

    /// 处理报警
    async fn process(&mut self, alarm: Alarm) {
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

        let play_time = alarm
            .timestamp
            .saturating_add(self.service.get_play_delay().await);
        let current_time = OffsetDateTime::now_utc();
        if play_time > OffsetDateTime::now_utc() {
            let delay =
                Duration::from_millis((play_time - current_time).whole_milliseconds() as u64);
            info!("Delay: {:?}", delay);
            tokio::time::sleep(delay).await;
        }
        info!("Check alarm palyablility after daley...");
        if !self.service.is_alarm_playable(&alarm).await {
            info!("Alarm: {:?} not playable, Skiped!", alarm);
            return;
        }

        self.alarm_to_play(alarm).await;
    }

    async fn alarm_to_play(&self, alarm: Alarm) {
        info!("Send alarm: {:?} to play queue...", alarm);
        if let Err(e) = self.play_sender.send(alarm).await {
            error!("Failed to send alarm to play queue: {}", e);
        }
    }

    pub async fn get_sender(&self) -> Sender<Alarm> {
        self.sender.clone()
    }
}
