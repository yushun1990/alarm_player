use std::sync::Arc;

use clap::{Parser, command};
use mimalloc::MiMalloc;

use crate::{
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

    let mut player = Player::new(config.queue.player_size, service.clone());
    let cycle = Cycle::init(player.sender.clone(), player.cycle_rx, service.clone()).await;

    let real_time = RealTime::new(
        config.queue.real_time_size,
        config.alarm.asc_interval_secs,
        player.sender(),
        service.clone(),
    );
    Ok(())
}
