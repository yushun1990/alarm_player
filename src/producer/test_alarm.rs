use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use rumqttc::v5::AsyncClient;
use time::OffsetDateTime;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::sleep,
};
use tracing::{error, info};

use crate::{model::Alarm, mqtt_client::Produce, service::AlarmService};

pub struct Producer {
    topic: &'static str,
    tx: Sender<String>,
}

impl Producer {
    pub fn new(topic: &'static str, tx: Sender<String>) -> Self {
        Self { topic, tx }
    }
}

#[async_trait]
impl Produce for Producer {
    async fn mat(&self, topic: &str) -> bool {
        return topic.ends_with(self.topic);
    }

    async fn proc(&self, payload: Bytes) -> anyhow::Result<()> {
        let ct = std::str::from_utf8(&payload)?;
        self.tx
            .send(ct.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }
}

#[allow(unused)]
pub struct TestAlarm<S>
where
    S: AlarmService + 'static,
{
    empty_schedule_secs: u64,
    client: AsyncClient,
    service: Arc<S>,
}

impl<S> TestAlarm<S>
where
    S: AlarmService + 'static,
{
    pub fn new(empty_schedule_secs: u64, client: AsyncClient, service: Arc<S>) -> Self {
        Self {
            empty_schedule_secs,
            client,
            service,
        }
    }

    pub async fn run(&self, tx: Sender<Alarm>, mut rx: Receiver<String>) {
        loop {
            tokio::select! {
                ct = rx.recv() => {
                    match ct {
                        Some(ct) => {
                            info!("Received crontab: {ct}");
                            self.service.update_crontab(ct).await;
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
        match self.service.next_fire_time().await {
            Some(nt) => {
                let secs = (nt - OffsetDateTime::now_utc()).whole_seconds();
                sleep(Duration::from_secs(secs as u64)).await;

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
