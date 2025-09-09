use bytes::Bytes;

/// 消息处理器
pub trait Handler: Clone + Send + Sync {
    /// 消息处理
    fn proc(&self, topic: String, payload: Bytes) -> impl Future<Output = anyhow::Result<()>>;
}

/// DefaultHandler, don't match any topic.
#[derive(Clone, Default)]
pub struct DefaultHandler;

impl Handler for DefaultHandler {
    async fn proc(&self, topic: String, _: Bytes) -> anyhow::Result<()> {
        anyhow::bail!("No handler matched for topic: {topic}")
    }
}

mod act_alarm;
pub use act_alarm::ActAlarmHandler;

mod test_alarm;
pub use test_alarm::{TestAlarm, TestAlarmHandler};

mod farm_config;
pub use farm_config::{FarmConfig, FarmConfigHandler};

mod sound_posts;
pub use sound_posts::{Soundposts, SoundpostsHandler};

mod house_set;
pub use house_set::HouseSetHandler;

mod alarm_confirm;
pub use alarm_confirm::{AlarmConfirm, AlarmConfirmHandler};
