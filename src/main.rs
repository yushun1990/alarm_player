use std::{process, sync::Arc};

use alarm_player::{config::Alarm, producer::act_alarm};
use mimalloc::MiMalloc;
use tokio::{
    signal::{self, unix::signal},
    sync::{Notify, mpsc},
};
use tracing::{error, info};


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info") // Default log level: info
        .init();
    info!("Starting alarm service");
    let mut mqtt = alarm_player::config::Mqtt::default();
    mqtt.broker = Some("192.168.77.34".into());
    mqtt.clean_session = Some(true);

    let (alarm_tx, alarm_rx) = mpsc::channel::<Alarm>(100);

    let shutdown = Arc::new(Notify::new());
    let producer = act_alarm::Producer::new(mqtt, alarm_tx.clone(), shutdown.clone());

    #[cfg(unix)]
    let mut term_signal = signal::unix::signal(signal::unix::SignalKind::terminate())?;

    tokio::select! {
        _ = signal::ctrl_c() => info!("Received Ctrl+C"),
        _ = async { term_signal.recv().await } => info!("Received SIGTERM"),
        result = producer.run() => {
            if let Err(e) = result {
                error!("Alarm produce failed: {e}");
                shutdown.notify_waiters();
                process::exit(1);
            }
        }
    };

    shutdown.notify_waiters();

    info!("===END!");

    Ok(())
}
