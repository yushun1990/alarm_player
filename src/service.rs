use async_trait::async_trait;
use time::OffsetDateTime;

use crate::model::Alarm;

#[async_trait]
pub trait AlarmService {
    async fn get_alarms(&self) -> Vec<Alarm>;
    /// 测试报警定时任务下一次触发时间
    async fn next_fire_time(&self) -> OffsetDateTime;
    /// 读取报警播放延时
    async fn get_play_delay(&self) -> time::Duration;
    /// 是否存在正在进行的报警
    async fn is_ongoing_alarm_exist(&self) -> bool;
    /// 播放队列中的报警是否具备播放条件
    async fn is_alarm_playable(&self, alarm: &Alarm) -> bool;
    /// 实时队列中的报警是否具备播放条件
    async fn is_realtime_alarm_playable(&self, alarm: &Alarm) -> bool;
    /// 循环队列中的报警是否具备播放条件
    async fn is_recur_alarm_playable(&self, alarm: &Alarm) -> bool;
}

pub struct DefaultAlarmServiceImpl {}

impl AlarmService for DefaultAlarmServiceImpl {
    #[doc = " 测试报警定时任务下一次触发时间"]
    #[must_use]
    #[allow(
        elided_named_lifetimes,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds
    )]
    fn next_fire_time<'life0, 'async_trait>(
        &'life0 self,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = OffsetDateTime>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    #[doc = " 读取报警播放延时"]
    #[must_use]
    #[allow(
        elided_named_lifetimes,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds
    )]
    fn get_play_delay<'life0, 'async_trait>(
        &'life0 self,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = time::Duration>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    #[doc = " 是否存在正在进行的报警"]
    #[must_use]
    #[allow(
        elided_named_lifetimes,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds
    )]
    fn is_ongoing_alarm_exist<'life0, 'async_trait>(
        &'life0 self,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = bool> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    #[doc = " 播放队列中的报警是否具备播放条件"]
    #[must_use]
    #[allow(
        elided_named_lifetimes,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds
    )]
    fn is_alarm_playable<'life0, 'life1, 'async_trait>(
        &'life0 self,
        alarm: &'life1 Alarm,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = bool> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    #[doc = " 实时队列中的报警是否具备播放条件"]
    #[must_use]
    #[allow(
        elided_named_lifetimes,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds
    )]
    fn is_realtime_alarm_playable<'life0, 'life1, 'async_trait>(
        &'life0 self,
        alarm: &'life1 Alarm,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = bool> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    #[doc = " 循环队列中的报警是否具备播放条件"]
    #[must_use]
    #[allow(
        elided_named_lifetimes,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds
    )]
    fn is_recur_alarm_playable<'life0, 'life1, 'async_trait>(
        &'life0 self,
        alarm: &'life1 Alarm,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = bool> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    #[must_use]
    #[allow(
        elided_named_lifetimes,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds
    )]
    fn get_alarms<'life0, 'async_trait>(
        &'life0 self,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = Vec<Alarm>> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }
}
