use std::sync::Arc;

use tokio::sync::{
    Mutex,
    mpsc::{Receiver, Sender},
};

use crate::{model::Alarm, service::AlarmService};

pub struct Cycle<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    pub sender: Sender<Alarm>,
    pub receiver: Receiver<Alarm>,
    pub data: Arc<Mutex<Vec<Alarm>>>,
    pub service: Arc<S>,
}

impl<S> Cycle<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    pub async fn init(sender: Sender<Alarm>, receiver: Receiver<Alarm>, service: Arc<S>) -> Self {
        let data = Arc::new(Mutex::new(service.get_alarms().await));
        Self {
            sender,
            receiver,
            service,
            data,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
