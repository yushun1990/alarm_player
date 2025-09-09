use std::sync::Arc;

use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::{model::Alarm, service::AlarmService};

use super::Handler;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmConfirm {
    pub house_code: String,
    pub target_name: String,
    pub is_confirmed: bool,
}

#[derive(Clone)]
pub struct AlarmConfirmHandler<H: Handler> {
    topic: &'static str,
    service: Arc<RwLock<AlarmService>>,
    child_handler: Option<H>,
}

impl<H: Handler> AlarmConfirmHandler<H> {
    pub fn new(service: Arc<RwLock<AlarmService>>) -> Self {
        Self {
            topic: "confirm",
            service,
            child_handler: None,
        }
    }

    pub fn handler(mut self, handler: H) -> Self {
        self.child_handler = Some(handler);
        self
    }

    pub fn mat(&self, topic: &str) -> bool {
        return topic.ends_with(self.topic);
    }

    fn deserialize(&self, data: Bytes) -> anyhow::Result<Vec<AlarmConfirm>> {
        let payload = serde_json::from_slice::<Vec<AlarmConfirm>>(&data)?;
        Ok(payload)
    }
}

#[allow(unreachable_code)]
impl<H: Handler> Handler for AlarmConfirmHandler<H> {
    async fn proc(&self, topic: String, payload: Bytes) -> anyhow::Result<()> {
        if !self.mat(&topic) {
            if let Some(child) = self.child_handler.clone() {
                return child.proc(topic, payload).await;
            }

            return anyhow::bail!("No handler matched for topic: {topic}");
        }

        let confirms = self.deserialize(payload)?;
        let mut alarms = Vec::new();
        for c in confirms {
            let mut alarm = Alarm::default();
            alarm.house_code = c.house_code;
            alarm.target_name = c.target_name;
            alarm.is_confirmed = c.is_confirmed;
            alarms.push(alarm);
        }
        let mut service = self.service.write().await;
        service.confirm_alarms(alarms);

        Ok(())
    }
}
