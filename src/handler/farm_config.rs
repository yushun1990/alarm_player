use std::sync::Arc;

use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::{
    service::{AlarmService, BoxConfig},
    task::Play,
};

use super::Handler;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FarmConfig {
    pub pause: Option<bool>,
    pub lang: Option<String>,
    pub enable_box: Option<bool>,
}

#[derive(Clone)]
pub struct FarmConfigHandler<H: Handler> {
    topic: &'static str,
    play: Play,
    child_handler: Option<H>,
    service: Arc<RwLock<AlarmService>>,
}

impl<H: Handler> FarmConfigHandler<H> {
    pub fn new(play: Play, service: Arc<RwLock<AlarmService>>) -> Self {
        Self {
            topic: "farm_config",
            play,
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

    fn deserialize(&self, data: Bytes) -> anyhow::Result<FarmConfig> {
        let payload = serde_json::from_slice::<FarmConfig>(&data)?;
        Ok(payload)
    }
}

#[allow(unreachable_code)]
impl<H: Handler> Handler for FarmConfigHandler<H> {
    async fn proc(&self, topic: String, payload: Bytes) -> anyhow::Result<()> {
        if !self.mat(&topic) {
            if let Some(child) = self.child_handler.clone() {
                return child.proc(topic, payload).await;
            }

            return anyhow::bail!("No handler matched for topic: {topic}");
        }

        let fc = self.deserialize(payload)?;
        if let Some(pause) = fc.pause {
            {
                let mut service = self.service.write().await;
                service.set_alarm_pause(pause);
            }

            if pause {
                self.play.cancel_play().await;
            }
        }

        if let Some(lang) = fc.lang {
            {
                let mut service = self.service.write().await;
                service.set_language(lang);
            }
        }

        if let Some(enable_box) = fc.enable_box {
            {
                let mut service = self.service.write().await;
                service.set_soundbox(BoxConfig {
                    enabled: enable_box,
                    volume: 50,
                });
            }
        }

        Ok(())
    }
}
