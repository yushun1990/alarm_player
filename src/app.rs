use std::sync::Arc;

use clap::{Parser, command};
use mimalloc::MiMalloc;
use tokio::sync::mpsc::channel;

use crate::{
    model::Alarm,
    player::Player,
    processor::{cycle::Cycle, real_time::RealTime},
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

async fn run<S>(service: Arc<S>) -> anyhow::Result<()>
where
    S: AlarmService + Send + Sync + 'static,
{
    let args = Args::parse();
    let config = crate::config::Config::new(args.config.as_str())?;

    match config.tracing.level {
        Some(level) => tracing_subscriber::fmt().with_env_filter(level).init(),
        None => tracing_subscriber::fmt().with_env_filter("info").init(),
    };

    let (real_time_tx, real_time_rx) = channel::<Alarm>(config.queue.real_time_size);
    let (cycle_tx, cycle_rx) = channel::<Alarm>(config.queue.real_time_size);
    let (player_tx, player_rx) = channel::<Alarm>(config.queue.real_time_size);

    let player = Player::new(player_rx, cycle_tx.clone(), service.clone());
    let cycle = Cycle::init(player_tx.clone(), cycle_rx, service.clone()).await;
    let real_time = RealTime::new(
        player_tx.clone(),
        real_time_rx,
        config.alarm.asc_interval_secs,
        service.clone(),
    );

    Ok(())
}
