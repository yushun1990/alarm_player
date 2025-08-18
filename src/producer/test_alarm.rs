use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use time::OffsetDateTime;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::sleep,
};
use tracing::{error, info};

use crate::{model::Alarm, mqtt_client::Produce, service::AlarmService};

pub struct Producer<S>
where
    S: AlarmService + 'static,
{
    topic: &'static str,
    alarm_tx: Sender<Alarm>,
    service: Arc<S>,
    ct_tx: Sender<String>,
    ct_rx: Receiver<String>,
}

#[async_trait]
impl<S> Produce for Producer<S>
where
    S: AlarmService + 'static,
{
    async fn mat(&self, topic: &str) -> bool {
        return topic.ends_with(self.topic);
    }

    async fn proc(&self, payload: Bytes) -> anyhow::Result<()> {
        let ct = std::str::from_utf8(&payload)?;
        self.ct_tx
            .send(ct.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }
}

impl<S> Producer<S>
where
    S: AlarmService + 'static,
{
    pub fn new(topic: &'static str, alarm_tx: Sender<Alarm>, service: Arc<S>) -> Self {
        let (tx, rx) = mpsc::channel(10);
        Self {
            topic,
            alarm_tx,
            service,
            ct_tx: tx,
            ct_rx: rx,
        }
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                ct = self.ct_rx.recv() => {
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
                _ = Self::send_test_alarm(&self.service, &self.alarm_tx) => {
                }
            }
        }
    }

    async fn send_test_alarm(service: &S, tx: &Sender<Alarm>) {
        if let Some(nt) = service.next_fire_time().await {
            let secs = (nt - OffsetDateTime::now_utc()).whole_seconds();
            sleep(Duration::from_secs(secs as u64)).await;

            if let Err(e) = tx.send(Alarm::default()).await {
                error!("Failed send test alarm to real time queue: {e}");
            }
        }
    }
}
