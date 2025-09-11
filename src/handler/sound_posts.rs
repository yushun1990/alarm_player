use crate::{Service, service::PostConfig};
use bytes::Bytes;
use serde::Deserialize;

use super::Handler;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Soundposts {
    pub device_ids: Option<Vec<u32>>,
    pub speed: Option<u8>,
}

#[derive(Clone)]
pub struct SoundpostsHandler<H: Handler> {
    topic: &'static str,
    service: Service,
    child_handler: Option<H>,
}

impl<H: Handler> SoundpostsHandler<H> {
    pub fn new(service: Service) -> Self {
        Self {
            topic: "sound_posts",
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

    pub fn deserialize(&self, data: Bytes) -> anyhow::Result<Soundposts> {
        let payload = serde_json::from_slice::<Soundposts>(&data)?;
        Ok(payload)
    }
}

#[allow(unreachable_code)]
impl<H: Handler> Handler for SoundpostsHandler<H> {
    async fn proc(&self, topic: String, payload: Bytes) -> anyhow::Result<()> {
        if !self.mat(&topic) {
            if let Some(child) = self.child_handler.clone() {
                return child.proc(topic, payload).await;
            }

            return anyhow::bail!("No handler matched for topic: {topic}");
        }

        let sp = self.deserialize(payload)?;
        if let Some(device_ids) = sp.device_ids {
            let mut service = self.service.write().await;
            service.set_soundposts(PostConfig {
                device_ids,
                speed: match sp.speed {
                    Some(speed) => speed,
                    None => 50,
                },
            });
        }

        Ok(())
    }
}
