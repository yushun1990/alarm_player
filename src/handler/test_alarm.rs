use bytes::Bytes;
use std::{sync::Arc, time::Duration};
use time::OffsetDateTime;
use tokio::{
    sync::{
        RwLock,
        mpsc::{Receiver, Sender},
    },
    time::sleep,
};
use tracing::{error, info};

use crate::{model::Alarm, mqtt_client::MqttClient, service::AlarmService};

use super::Handler;

#[derive(Clone)]
pub struct TestAlarmHandler<H: Handler> {
    topic: &'static str,
    tx: Sender<String>,
    child_handler: Option<H>,
}

impl<H: Handler> TestAlarmHandler<H> {
    pub fn new(topic: &'static str, tx: Sender<String>) -> Self {
        Self {
            topic,
            tx,
            child_handler: None,
        }
    }

    pub fn handler(mut self, handler: H) -> Self {
        self.child_handler = Some(handler);
        self
    }

    fn mat(&self, topic: &str) -> bool {
        return topic.ends_with(self.topic);
    }

    fn deserialize(&self, data: Bytes) -> anyhow::Result<String> {
        let payload = std::str::from_utf8(&data)?;

        Ok(payload.to_string())
    }
}

#[allow(unreachable_code)]
impl<H: Handler> Handler for TestAlarmHandler<H> {
    async fn proc(&self, topic: String, payload: Bytes) -> anyhow::Result<()> {
        if !self.mat(&topic) {
            if let Some(child) = self.child_handler.clone() {
                return child.proc(topic, payload).await;
            }

            return anyhow::bail!("No handler matched for topic: {topic}");
        }

        let payload = self.deserialize(payload)?;
        self.tx
            .send(payload)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }
}

#[allow(unused)]
pub struct TestAlarm {
    empty_schedule_secs: u64,
    client: MqttClient,
    service: Arc<RwLock<AlarmService>>,
}

impl TestAlarm {
    pub fn new(
        empty_schedule_secs: u64,
        client: MqttClient,
        service: Arc<RwLock<AlarmService>>,
    ) -> Self {
        Self {
            empty_schedule_secs,
            client,
            service,
        }
    }

    pub async fn run(&mut self, tx: Sender<Alarm>, mut rx: Receiver<String>) {
        loop {
            tokio::select! {
                ct = rx.recv() => {
                    match ct {
                        Some(ct) => {
                            info!("Received crontab: {ct}");
                            let mut service = self.service.write().await;
                            service.set_crontab(ct);
                        }
                        None => {
                            info!("Crontab channle closed, exit...");
                            return;
                        }
                    }
                }
                _ = self.send_test_alarm(&tx) => {
                }
            }
        }
    }

    async fn send_test_alarm(&self, tx: &Sender<Alarm>) {
        let next_fire_time = {
            let service = self.service.read().await;
            service.next_fire_time()
        };

        match next_fire_time {
            Some(nt) => {
                info!("Next fire time: {:?}", nt);
                let duration = nt - OffsetDateTime::now_utc();
                sleep(Duration::from_nanos(duration.whole_nanoseconds() as u64)).await;
                if let Err(e) = tx.send(Alarm::default()).await {
                    error!("Failed send test alarm to real time queue: {e}");
                }
                // TODO: mqtt 发送测试结果确认消息
            }
            None => {
                info!("No test alarm schedules ...");
                sleep(Duration::from_secs(self.empty_schedule_secs)).await;
            }
        }
    }
}
