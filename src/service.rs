use chrono::Utc;
use cron::Schedule;
use std::collections::HashMap;
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::{error, warn};

use crate::model::Alarm;

pub enum AlarmStatus {
    Playable,
    Canceled,
    Paused,
}

pub trait AlarmService: Clone + Send + Sync {
    /// 设置鸡舍暂停状态
    fn set_house_status(&mut self, house_code: String, paused: bool);
    /// 获取当前报警列表
    fn get_alarms(&self) -> Vec<Alarm>;
    /// 添加报警
    fn set_alarm(&mut self, alarm: Alarm) -> bool;
    /// 是否存在正在进行的报警
    fn is_ongoing_alarm_exist(&self) -> bool;
    /// 获取报警状态
    fn get_alarm_status(&self, alarm: &Alarm) -> AlarmStatus;

    /// 测试报警定时任务下一次触发时间
    fn next_fire_time(&self) -> Option<OffsetDateTime>;
    /// 更新报警测试定时表达式
    fn set_crontab(&mut self, ct: String);
    /// 读取报警播放延时
    fn get_play_delay(&self) -> time::Duration;
    /// 更新报警播放延时
    fn set_play_delay(&mut self, play_delay_secs: u64);
}

#[derive(Clone)]
pub struct DefaultAlarmServiceImpl {
    // 测试报警触发 crontab 表达方式
    crontab: Option<String>,
    // 报警播放延时
    play_delay_secs: u64,
    // 报警暂停
    is_alarm_paused: bool,
    // 报警快照集合，已取消报警直接移除
    alarm_set: HashMap<String, Alarm>,
    /// 鸡舍状态
    house_status: HashMap<String, bool>,
}

impl DefaultAlarmServiceImpl {
    pub fn new(
        crontab: Option<String>,
        play_delay_secs: u64,
        is_alarm_paused: bool,
        alarms: Vec<Alarm>,
        empty_house_codes: Vec<String>,
    ) -> Self {
        let mut alarm_service: Self = Default::default();
        if let Some(crontab) = crontab {
            alarm_service.crontab = Some(crontab);
        }

        alarm_service.play_delay_secs = play_delay_secs;
        alarm_service.is_alarm_paused = is_alarm_paused;

        for alarm in &alarms {
            alarm_service
                .alarm_set
                .insert(Self::get_alarm_set_key(&alarm), alarm.clone());
        }

        for code in &empty_house_codes {
            alarm_service.house_status.insert(code.clone(), true);
        }

        alarm_service
    }

    fn get_alarm_set_key(alarm: &Alarm) -> String {
        format!("{}_{}", alarm.house_code, alarm.target_name)
    }
}

impl Default for DefaultAlarmServiceImpl {
    fn default() -> Self {
        Self {
            crontab: None,
            play_delay_secs: 20,
            is_alarm_paused: false,
            alarm_set: HashMap::new(),
            house_status: HashMap::new(),
        }
    }
}

impl AlarmService for DefaultAlarmServiceImpl {
    fn set_house_status(&mut self, house_code: String, paused: bool) {
        self.house_status.insert(house_code, paused);
    }

    fn get_alarms(&self) -> Vec<Alarm> {
        self.alarm_set.values().cloned().collect()
    }

    fn set_alarm(&mut self, alarm: Alarm) -> bool {
        let key = Self::get_alarm_set_key(&alarm);
        match self.alarm_set.get(&key) {
            Some(last_alarm) => {
                if alarm.timestamp < last_alarm.timestamp {
                    warn!(
                        "Invalid alarm timestamp: {}, last_alarm timestamp: {}",
                        alarm.timestamp, last_alarm.timestamp
                    );
                    return false;
                }
                if !alarm.is_alarm {
                    // 消警，删除报警缓存
                    self.alarm_set.remove(&key);
                }
                return false;
            }
            None => {
                let _ = self.alarm_set.insert(key, alarm);
                true
            }
        }
    }

    fn is_ongoing_alarm_exist(&self) -> bool {
        !self.alarm_set.is_empty()
    }

    fn get_alarm_status(&self, alarm: &Alarm) -> AlarmStatus {
        let key = Self::get_alarm_set_key(&alarm);
        if !self.alarm_set.contains_key(&key) {
            // 不存在，说明报警已经被取消
            return AlarmStatus::Canceled;
        }

        // 报警暂停
        if self.is_alarm_paused {
            return AlarmStatus::Paused;
        }

        // 空舍
        let paused = match self.house_status.get(&alarm.house_code) {
            Some(&paused) => paused,
            None => false,
        };
        if self.is_alarm_paused || alarm.is_confirmed || paused {
            return AlarmStatus::Paused;
        }

        return AlarmStatus::Playable;
    }

    fn next_fire_time(&self) -> Option<OffsetDateTime> {
        match &self.crontab {
            Some(crontab) => match Schedule::from_str(crontab.as_str()) {
                Ok(schedule) => {
                    if let Some(dt) = schedule.upcoming(Utc).next() {
                        match OffsetDateTime::from_unix_timestamp(dt.timestamp()) {
                            Ok(t) => return Some(t),
                            Err(e) => {
                                error!("Datetime convert failed: {e}");
                                return None;
                            }
                        }
                    }
                    error!("Invalid crontab...");
                    return None;
                }
                Err(e) => {
                    error!("Crontab parse failed: {e}");
                    return None;
                }
            },
            None => {
                warn!("Crontab is empty...");
                return None;
            }
        }
    }

    fn set_crontab(&mut self, ct: String) {
        self.crontab = Some(ct);
    }

    fn get_play_delay(&self) -> time::Duration {
        time::Duration::seconds(self.play_delay_secs as i64)
    }

    fn set_play_delay(&mut self, play_delay_secs: u64) {
        self.play_delay_secs = play_delay_secs;
    }
}
