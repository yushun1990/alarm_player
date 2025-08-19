use chrono::Utc;
use cron::Schedule;
use std::collections::HashMap;
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::error;

use crate::model::Alarm;

pub trait AlarmService: Clone + Send + Sync {
    fn get_alarms(&self) -> impl Future<Output = Vec<Alarm>> + Send;
    /// 测试报警定时任务下一次触发时间
    fn next_fire_time(&self) -> impl Future<Output = Option<OffsetDateTime>> + Send;
    /// 更新报警测试定时表达式
    fn update_crontab(&self, ct: String) -> impl Future<Output = ()> + Send;
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
pub struct DefaultAlarmServiceImpl {
    crontab: Option<String>,
    play_delay_secs: u64,
    is_alarm_paused: bool,
    alarm_set: HashMap<String, Alarm>,
}

impl Default for DefaultAlarmServiceImpl {
    fn default() -> Self {
        Self {
            crontab: None,
            play_delay_secs: 20,
            is_alarm_paused: false,
            alarm_set: HashMap::new(),
        }
    }
}

#[allow(unused_variables)]
impl AlarmService for DefaultAlarmServiceImpl {
    async fn get_alarms(&self) -> Vec<Alarm> {
        self.alarm_set.values().cloned().collect()
    }

    async fn next_fire_time(&self) -> Option<OffsetDateTime> {
        match &self.crontab {
            Some(crontab) => match Schedule::from_str(crontab.as_str()) {
                Ok(schedule) => {
                    if let Some(dt) = schedule.upcoming(Utc).next() {
                        todo!()
                    }
                    return None;
                }
                Err(e) => {
                    error!("Crontab parse failed: {e}");
                    return None;
                }
            },
            None => return None,
        }
    }

    async fn update_crontab(&self, ct: String) -> () {}

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
        false
    }
}
