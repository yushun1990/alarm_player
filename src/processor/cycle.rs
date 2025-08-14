use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{
        Mutex,
        mpsc::{Receiver, Sender},
    },
    time::sleep,
};
use tracing::{error, info};

use crate::{model::Alarm, service::AlarmService};

type NodePointor<T> = Arc<Mutex<Option<Arc<Node<T>>>>>;
pub struct Node<T>
where
    T: Clone + Send + Sync,
{
    data: T,
    prev: NodePointor<T>,
    next: NodePointor<T>,
}

impl<T> Node<T>
where
    T: Clone + Send + Sync,
{
    fn new(data: T) -> Arc<Self> {
        let node = Arc::new(Node {
            data,
            prev: Arc::new(Mutex::new(None)),
            next: Arc::new(Mutex::new(None)),
        });

        *node.next.blocking_lock() = Some(node.clone());
        *node.prev.blocking_lock() = Some(node.clone());

        node
    }

    async fn append(self: &Arc<Self>, data: T) -> Arc<Self> {
        let node = Arc::new(Node {
            data,
            prev: Arc::new(Mutex::new(None)),
            next: Arc::new(Mutex::new(None)),
        });

        let next = self.next.lock().await.as_ref().unwrap().clone();
        *node.next.lock().await = Some(next.clone());
        *node.prev.lock().await = Some(self.clone());

        *next.prev.lock().await = Some(node.clone());

        *self.next.lock().await = Some(node.clone());

        node
    }

    async fn remove(self: &Arc<Self>) -> Option<Arc<Self>> {
        let prev = self.prev.lock().await.as_ref().unwrap().clone();
        let next = self.next.lock().await.as_ref().unwrap().clone();

        if Arc::ptr_eq(self, &next) {
            return None;
        }

        *prev.next.lock().await = Some(next.clone());
        *next.prev.lock().await = Some(prev.clone());

        Some(next)
    }

    async fn init(data: Vec<T>) -> (Option<Arc<Self>>, Option<Arc<Self>>) {
        if data.is_empty() {
            return (None, None);
        }

        let header = Self::new(data[0].clone());
        let mut tail = header.clone();
        for item in &data[1..] {
            tail = tail.append(item.clone()).await;
        }

        (Some(header), Some(tail))
    }
}

pub struct Cycle<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    player_tx: Sender<Alarm>,
    cycle_rx: Receiver<Alarm>,
    check_interval: u64,
    header: Option<Arc<Node<Alarm>>>,
    tail: Option<Arc<Node<Alarm>>>,
    service: Arc<S>,
}

impl<S> Cycle<S>
where
    S: AlarmService + Send + Sync + 'static,
{
    pub async fn init(
        player_tx: Sender<Alarm>,
        cycle_rx: Receiver<Alarm>,
        check_interval: u64,
        service: Arc<S>,
    ) -> Self {
        let data = service.get_alarms().await;
        let (header, tail) = Node::init(data).await;
        Self {
            player_tx,
            cycle_rx,
            check_interval,
            header,
            tail,
            service,
        }
    }

    pub async fn run(&mut self) {
        let mut cursor = self.header.clone();
        loop {
            tokio::select! {
                alarm = self.cycle_rx.recv() => {
                    let alarm = match alarm {
                        Some(alarm) => alarm,
                        None => {
                            info!("Cycle queue closed, exit...");
                            return
                        }
                    };
                    match self.tail.clone() {
                        Some(tail) => {
                            self.tail = Some(tail.append(alarm).await);
                        },
                        None => {
                            self.header = Some(Node::new(alarm));
                            self.tail = self.header.clone();
                        }
                    }
                }
                _ = async {
                    cursor = self.play(cursor.clone().unwrap()).await;
                }, if !cursor.is_none() => {}
            }
        }
    }

    pub async fn play(&mut self, cursor: Arc<Node<Alarm>>) -> Option<Arc<Node<Alarm>>> {
        if !self.service.is_cycle_alarm_playable(&cursor.data).await {
            return cursor.remove().await;
        }

        if let Err(e) = self.player_tx.send(cursor.data.clone()).await {
            error!("Failed send alarm to player: {e}");
        }

        sleep(Duration::from_secs(self.check_interval)).await;

        Some(cursor.next.lock().await.as_ref().unwrap().clone())
    }
}
