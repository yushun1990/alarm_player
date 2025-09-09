use bytes::Bytes;
use std::{sync::Arc, time::Duration};
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio::{
    sync::{
        RwLock,
        mpsc::{Receiver, Sender},
    },
    time::sleep,
};
use tracing::{error, info};

use crate::{
    TOPIC_RESULT_CRONTAB,
    model::{Alarm, TestAlarmConfig},
    service::AlarmService,
};

use super::Handler;

#[derive(Clone)]
pub struct TestAlarmHandler<H: Handler> {
    topic: &'static str,
    tx: Sender<TestAlarmConfig>,
    child_handler: Option<H>,
}

impl<H: Handler> TestAlarmHandler<H> {
    pub fn new(tx: Sender<TestAlarmConfig>) -> Self {
        Self {
            topic: "crontab",
            tx,
            child_handler: None,
        }
    }

    pub fn handler(mut self, handler: H) -> Self {
        self.child_handler = Some(handler);
        self
    }

    fn mat(&self, topic: &str) -> bool {
        return topic.ends_with(self.topic);
    }

    fn deserialize(&self, data: Bytes) -> anyhow::Result<TestAlarmConfig> {
        let config = serde_json::from_slice::<TestAlarmConfig>(&data)?;
        Ok(config)
    }
}

#[allow(unreachable_code)]
impl<H: Handler> Handler for TestAlarmHandler<H> {
    async fn proc(&self, topic: String, payload: Bytes) -> anyhow::Result<()> {
        if !self.mat(&topic) {
            if let Some(child) = self.child_handler.clone() {
                return child.proc(topic, payload).await;
            }

            return anyhow::bail!("No handler matched for topic: {topic}");
        }

        let payload = self.deserialize(payload)?;
        self.tx
            .send(payload)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }
}

#[allow(unused)]
pub struct TestAlarm {
    crontab: Option<String>,
    service: Arc<RwLock<AlarmService>>,
}

impl TestAlarm {
    pub fn new(service: Arc<RwLock<AlarmService>>) -> Self {
        Self {
            crontab: None,
            service,
        }
    }

    pub async fn init(&mut self) {
        let service = self.service.read().await;
        self.crontab = service.get_crontab()
    }

    pub async fn run(&mut self, tx: Sender<Alarm>, mut rx: Receiver<TestAlarmConfig>) {
        loop {
            tokio::select! {
                ct = rx.recv() => {
                    match ct {
                        Some(ct) => {
                            info!("Received test alarm config: {:?}", ct);
                            if ct.play_now {
                                let is_ongoing_alarm_exist = {
                                    let service = self.service.read().await;
                                    service.is_ongoing_alarm_exist()
                                };

                                let result = "{\"code\": 1, \"message\": \"当前有未取消的报警\", \"data\": {}}".to_string();
                                if is_ongoing_alarm_exist {
                                    {
                                        let mut service = self.service.write().await;
                                        service.publish(TOPIC_RESULT_CRONTAB, result).await;
                                    }
                                    continue;

                                }

                                let now = match OffsetDateTime::now_local() {
                                    Ok(local) => local,
                                    Err(e) => {
                                        error!("Can't read local time: {}", e);
                                        OffsetDateTime::now_utc()
                                    }
                                };
                                let mut alarm = Alarm::default();
                                alarm.test_plan_time = Some(PrimitiveDateTime::new(now.date(), now.time()));
                                if let Err(e) = tx.send(alarm).await {
                                    error!("Failed send test alarm to real time queue: {e}");
                                }

                                continue;
                            }
                            let config = ct.clone();
                            {
                                let mut service = self.service.write().await;
                                service.test_alarm_config(config);
                            }
                            self.crontab = ct.crontab;
                        }
                        None => {
                            info!("Crontab channle closed, exit...");
                            return;
                        }
                    }
                }
                _ = self.send_test_alarm(&tx), if self.crontab.is_some() => {
                }
            }
        }
    }

    async fn send_test_alarm(&self, tx: &Sender<Alarm>) {
        info!("Calculate crontab...");
        let next_fire_time = {
            let service = self.service.read().await;
            service.next_fire_time()
        };

        match next_fire_time {
            Some(nt) => {
                info!("Next fire time: {:?}", nt);
                let duration = nt - OffsetDateTime::now_utc();
                sleep(Duration::from_nanos(duration.whole_nanoseconds() as u64)).await;
                let now = match OffsetDateTime::now_local() {
                    Ok(local) => local,
                    Err(e) => {
                        error!("Can't read local time: {}", e);
                        OffsetDateTime::now_utc()
                    }
                };
                let mut alarm = Alarm::default();
                alarm.test_plan_time = Some(PrimitiveDateTime::new(now.date(), now.time()));
                if let Err(e) = tx.send(alarm).await {
                    error!("Failed send test alarm to real time queue: {e}");
                }
            }
            None => {
                info!("No test alarm schedules ...");
                // sleep(Duration::from_secs(self.empty_schedule_secs)).await;
            }
        }
    }
}
