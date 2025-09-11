use crate::{Service, service::House};
use bytes::Bytes;

use super::Handler;

#[derive(Clone)]
pub struct HouseSetHandler<H: Handler> {
    topic: &'static str,
    service: Service,
    child_handler: Option<H>,
}

impl<H: Handler> HouseSetHandler<H> {
    pub fn new(service: Service) -> Self {
        Self {
            topic: "houses",
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

    fn deserialize(&self, data: Bytes) -> anyhow::Result<Vec<House>> {
        let payload = serde_json::from_slice::<Vec<House>>(&data)?;
        Ok(payload)
    }
}

#[allow(unreachable_code)]
impl<H: Handler> Handler for HouseSetHandler<H> {
    async fn proc(&self, topic: String, payload: Bytes) -> anyhow::Result<()> {
        if !self.mat(&topic) {
            if let Some(child) = self.child_handler.clone() {
                return child.proc(topic, payload).await;
            }

            return anyhow::bail!("No handler matched for topic: {topic}");
        }

        let houses = self.deserialize(payload)?;
        let mut service = self.service.write().await;
        service.set_houses(houses);

        Ok(())
    }
}
