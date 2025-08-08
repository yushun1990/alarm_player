use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{
        Notify,
        mpsc::{Receiver, Sender, channel, error::TryRecvError},
    },
    time::sleep,
};

use crate::config::Alarm;

pub struct RealTime {
    sender: Sender<Alarm>,
    receiver: Receiver<Alarm>,
    size: usize,
    test_alarm: Option<Alarm>,
    real_alarm: Option<Alarm>,
    check_interval: Duration,
}

impl RealTime {
    pub fn new(size: usize, check_interval: u64) -> Self {
        let (sender, receiver) = channel::<Alarm>(100);

        Self {
            sender,
            receiver,
            size,
            test_alarm: None,
            real_alarm: None,
            check_interval: Duration::from_secs(check_interval),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let alarm = match self.receiver.try_recv() {
            Ok(alarm) => Some(alarm),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => return Ok(()),
        };

        if alarm.is_none() {
            if self.test_alarm.clone().is_none() {
                let alarm = match self.receiver.recv().await {
                    Some(alarm) => alarm,
                    None => return Ok(()),
                };
                // TODO: process alarm
            } else {
                if self.is_ongoing_alarm_exist().await {
                    sleep(self.check_interval).await;
                    self.sender.send(self.test_alarm.clone().unwrap()).await?;
                } else {
                    // TODO: write to player queue.
                }
            }
        }

        Ok(())
    }

    pub async fn is_ongoing_alarm_exist(&self) -> bool {
        false
    }
}
