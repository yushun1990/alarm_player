use std::sync::Arc;

use alarm_player::{app, service::DefaultAlarmServiceImpl};

#[tokio::main]
async fn main() {
    app::run::<DefaultAlarmServiceImpl>(Arc::new(DefaultAlarmServiceImpl::new())).await;
}
