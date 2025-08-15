use time::OffsetDateTime;

use crate::model::Alarm;

pub trait AlarmService: Clone + Send + Sync {
    fn get_alarms(&self) -> impl Future<Output = Vec<Alarm>> + Send;
    /// 测试报警定时任务下一次触发时间
    fn next_fire_time(&self) -> impl Future<Output = OffsetDateTime> + Send;
    /// 读取报警播放延时
    fn get_play_delay(&self) -> impl Future<Output = time::Duration> + Send;
    /// 是否存在正在进行的报警
    fn is_ongoing_alarm_exist(&self) -> impl Future<Output = bool> + Send;
    /// 播放队列中的报警是否具备播放条件
    fn is_alarm_playable(&self, alarm: &Alarm) -> impl Future<Output = bool> + Send;
    /// 实时队列中的报警是否具备播放条件
    fn is_realtime_alarm_playable(&self, alarm: &Alarm) -> impl Future<Output = bool> + Send;
    /// 循环队列中的报警是否具备播放条件
    fn is_cycle_alarm_playable(&self, alarm: &Alarm) -> impl Future<Output = bool> + Send;
}

#[derive(Clone)]
pub struct DefaultAlarmServiceImpl {}

#[allow(unused_variables)]
impl AlarmService for DefaultAlarmServiceImpl {
    async fn get_alarms(&self) -> Vec<Alarm> {
        vec![]
    }

    async fn next_fire_time(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    async fn get_play_delay(&self) -> time::Duration {
        time::Duration::seconds(20)
    }

    async fn is_ongoing_alarm_exist(&self) -> bool {
        false
    }

    async fn is_alarm_playable(&self, alarm: &Alarm) -> bool {
        true
    }

    async fn is_realtime_alarm_playable(&self, alarm: &Alarm) -> bool {
        true
    }

    async fn is_cycle_alarm_playable(&self, alarm: &Alarm) -> bool {
        true
    }
}

impl DefaultAlarmServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}
