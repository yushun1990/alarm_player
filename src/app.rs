use std::sync::Arc;

use clap::{Parser, command};
use mimalloc::MiMalloc;
use tokio::{
    signal::{
        self,
        unix::{SignalKind, signal},
    },
    sync::{Notify, mpsc::channel},
};
use tracing::{error, info};

use crate::{
    model::Alarm,
    mqtt_client::MqttClient,
    player::Player,
    processor::{cycle::Cycle, real_time::RealTime},
    producer::act_alarm,
    service::AlarmService,
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "config.toml")]
    pub config: String,
}

pub async fn run<S>(service: Arc<S>)
where
    S: AlarmService + 'static,
{
    let args = Args::parse();
    let config = crate::config::Config::new(args.config.as_str()).unwrap();

    match config.tracing.level {
        Some(level) => tracing_subscriber::fmt().with_env_filter(level).init(),
        None => tracing_subscriber::fmt().with_env_filter("info").init(),
    };

    let (real_time_tx, real_time_rx) = channel::<Alarm>(config.queue.real_time_size);
    let (cycle_tx, cycle_rx) = channel::<Alarm>(config.queue.real_time_size);
    let (player_tx, player_rx) = channel::<Alarm>(config.queue.real_time_size);

    let player_serivce = service.clone();
    let player_handle = tokio::spawn(async move {
        Player::new(player_serivce).run(cycle_tx, player_rx).await;
    });

    let shutdown = Arc::new(Notify::new());
    let sd = shutdown.clone();
    let cycle_service = service.clone();
    let alarm_tx = player_tx.clone();
    let cycle_handle = tokio::spawn(async move {
        Cycle::init(config.alarm.cycle_interval_secs, cycle_service)
            .await
            .run(alarm_tx, cycle_rx, sd)
            .await;
    });
    let real_time_handle = tokio::spawn(async move {
        RealTime::new(config.alarm.asc_interval_secs, service)
            .run(player_tx, real_time_rx)
            .await;
    });

    let alarm_producer = act_alarm::Producer::new("alarm", real_time_tx.clone());
    let repub_alarm_producer = act_alarm::Producer::new("repub_alarms", real_time_tx);

    let mut client = MqttClient::new(config.mqtt)
        .produce(alarm_producer)
        .produce(repub_alarm_producer);

    #[cfg(unix)]
    let mut term_signal = signal(SignalKind::terminate()).unwrap();

    let topics = vec![
        "$share/ap/+/+/alarm",
        "$share/ap/+/+/repub_alarms",
        "/ap/test_alarm/crontab",
    ];
    tokio::select! {
        _ = signal::ctrl_c() => info!("Received Ctrl+C"),
        _ = term_signal.recv() => info!("Received SIGTERM"),
        result = client.subscribe(&topics, shutdown.clone()) => {
            if let Err(e) = result {
                error!("Alarm produce running failed: {e}");
                shutdown.notify_waiters();
            }
        }
    }

    shutdown.notify_waiters();

    let _ = tokio::join!(real_time_handle, cycle_handle, player_handle);

    info!("==================== Alarm player exited ====================");
}
