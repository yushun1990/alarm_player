use async_trait::async_trait;
use bytes::Bytes;
use time::OffsetDateTime;
use tokio::sync::mpsc::Sender;
use tracing::info;

use crate::{model::Alarm, mqtt_client::Produce};

pub struct Producer {
    topic: &'static str,
    tx: Sender<Alarm>,
}

impl Producer {
    pub fn new(topic: &'static str, tx: Sender<Alarm>) -> Self {
        Self { topic, tx }
    }
}

#[async_trait]
impl Produce for Producer {
    async fn mat(&self, topic: &str) -> bool {
        return topic.ends_with(self.topic);
    }

    async fn proc(&self, payload: Bytes) -> anyhow::Result<()> {
        let mut alarm = serde_json::from_slice::<Alarm>(&payload)?;
        alarm.received_time = Some(OffsetDateTime::now_utc());
        alarm.is_new = true;

        info!("Received alarm: {:?}", alarm);

        self.tx.send(alarm).await.map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }
}
