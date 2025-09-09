use bytes::Bytes;
use time::OffsetDateTime;
use tokio::sync::mpsc::Sender;
use tracing::info;

use crate::{model::Alarm, task::Play};

use super::Handler;

#[derive(Clone)]
pub struct ActAlarmHandler<H: Handler> {
    topic: &'static str,
    repub_topic: &'static str,
    tx: Sender<Alarm>,
    child_handler: Option<H>,
    play: Play,
}

impl<H: Handler> ActAlarmHandler<H> {
    pub fn new(tx: Sender<Alarm>, play: Play) -> Self {
        Self {
            topic: "alarm",
            repub_topic: "repub_alarms",
            tx,
            child_handler: None,
            play,
        }
    }

    pub fn handler(mut self, handler: H) -> Self {
        self.child_handler = Some(handler);
        self
    }

    fn mat(&self, topic: &str) -> bool {
        return topic.ends_with(self.topic) || topic.ends_with(self.repub_topic);
    }

    fn deserialize(&self, data: Bytes) -> anyhow::Result<Alarm> {
        let payload = serde_json::from_slice::<Alarm>(&data)?;
        Ok(payload)
    }
}

#[allow(unreachable_code)]
impl<H: Handler> Handler for ActAlarmHandler<H> {
    async fn proc(&self, topic: String, payload: Bytes) -> anyhow::Result<()> {
        if !self.mat(&topic) {
            if let Some(child) = self.child_handler.clone() {
                return child.proc(topic, payload).await;
            }

            return anyhow::bail!("No handler matched for topic: {topic}");
        }

        let mut alarm = self.deserialize(payload)?;
        alarm.received_time = Some(OffsetDateTime::now_utc());
        if let Some(house_code) = topic.split("/").next() {
            alarm.house_code = house_code.to_string();
        }

        info!("Received alarm: {:?}", alarm);
        self.tx.send(alarm).await.map_err(|e| anyhow::anyhow!(e))?;

        self.play.cancel_test_play().await;

        Ok(())
    }
}
