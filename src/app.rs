use std::sync::Arc;

use mimalloc::MiMalloc;
use tokio::{
    signal::{
        self,
        unix::{SignalKind, signal},
    },
    sync::{Notify, RwLock, mpsc::channel},
};
use tracing::{error, info};

use crate::{
    handler::{ActAlarmHandler, DefaultHandler, TestAlarm, TestAlarmHandler},
    model::Alarm,
    mqtt_client::MqttClient,
    service::AlarmService,
    task::{Cycle, Play, RealTime},
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub async fn run<S>(service: Arc<RwLock<S>>, config: crate::config::Config)
where
    S: AlarmService + 'static,
{
    tracing_subscriber::fmt()
        .with_env_filter(config.tracing.level())
        .init();

    let (real_time_tx, real_time_rx) = channel::<Alarm>(config.queue.real_time_size());
    let (cycle_tx, cycle_rx) = channel::<Alarm>(config.queue.real_time_size());
    let (player_tx, player_rx) = channel::<Alarm>(config.queue.real_time_size());
    let (ct_tx, ct_rx) = channel::<String>(10);

    let play_serivce = service.clone();
    let api_host = config.soundpost.api_host();
    let api_login_token = config.soundpost.api_login_token();
    let play_handle = tokio::spawn(async move {
        Play::new(api_host, api_login_token, play_serivce)
            .run(cycle_tx, player_rx)
            .await;
    });

    let shutdown = Arc::new(Notify::new());
    let sd = shutdown.clone();
    let cycle_service = service.clone();
    let alarm_tx = player_tx.clone();
    let cycle_interval_secs = config.alarm.cycle_interval_secs();
    let cycle_handle = tokio::spawn(async move {
        Cycle::init(cycle_interval_secs, cycle_service)
            .await
            .run(alarm_tx, cycle_rx, sd)
            .await;
    });
    let real_time_service = service.clone();
    let asc_interval_secs = config.alarm.asc_interval_secs();
    let real_time_handle = tokio::spawn(async move {
        RealTime::new(asc_interval_secs, real_time_service)
            .run(player_tx, real_time_rx)
            .await;
    });

    let handler = DefaultHandler::default();

    type TAH = TestAlarmHandler<DefaultHandler>;
    let handler = TAH::new("crontab", ct_tx).handler(handler);

    type AAH = ActAlarmHandler<TAH>;
    let handler = AAH::new("repub_alarms", real_time_tx.clone()).handler(handler);
    let handler = ActAlarmHandler::<AAH>::new("alarm", real_time_tx.clone()).handler(handler);

    let (client, eventloop) = MqttClient::new(config.mqtt);
    let mqtt_client = client.clone();
    let empty_schedule_secs = config.alarm.empty_schedule_secs();
    let test_alarm_handle = tokio::spawn(async move {
        TestAlarm::new(empty_schedule_secs, mqtt_client, service)
            .run(real_time_tx, ct_rx)
            .await;
    });

    let topics: Vec<String> = vec![
        "$share/ap/+/+/alarm".to_string(),
        "$share/ap/+/+/repub_alarms".to_string(),
        "ap/test_alarm/crontab".to_string(),
    ];

    let mqtt_shutdown = shutdown.clone();
    let mqtt_subscribe_handle = tokio::spawn(async move {
        if let Err(e) = client
            .subscribe(eventloop, topics, &handler, mqtt_shutdown.clone())
            .await
        {
            error!("Mqtt client subscribe failed: {e}");
            mqtt_shutdown.notify_waiters();
        }
    });

    #[cfg(unix)]
    let mut term_signal = signal(SignalKind::terminate()).unwrap();

    let st = shutdown.clone();
    tokio::select! {
        _ = signal::ctrl_c() => info!("Received Ctrl+C"),
        _ = term_signal.recv() => info!("Received SIGTERM"),
        _ = st.notified() => info!("Some error happend, exit...")
    }

    shutdown.notify_waiters();
    let _ = tokio::join!(
        mqtt_subscribe_handle,
        real_time_handle,
        cycle_handle,
        play_handle,
        test_alarm_handle
    );

    info!("==================== Alarm player exited ====================");
}
