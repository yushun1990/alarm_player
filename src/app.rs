use std::sync::Arc;

use tokio::{
    signal::{
        self,
        unix::{SignalKind, signal},
    },
    sync::{Notify, RwLock, mpsc::channel},
};
use tracing::{error, info};

use crate::{
    handler::{
        ActAlarmHandler, AlarmConfirmHandler, DefaultHandler, FarmConfigHandler, HouseSetHandler,
        SoundpostsHandler, TestAlarm, TestAlarmHandler,
    },
    model::{Alarm, TestAlarmConfig},
    mqtt_client::MqttClient,
    player::Soundpost,
    recorder::Recorder,
    service::AlarmService,
    task::{Cycle, Play, RealTime, WsClient},
};

pub async fn run(service: Arc<RwLock<AlarmService>>, config: crate::config::Config) {
    tracing_subscriber::fmt()
        .with_env_filter(config.tracing.level())
        .init();

    let (client, eventloop) = MqttClient::new(config.mqtt);
    {
        let mut service = service.write().await;
        service
            .init(config.alarm.localization_path())
            .await
            .unwrap();
        service.set_mqtt_client(client.clone());
    }

    let (real_time_tx, real_time_rx) = channel::<Alarm>(config.queue.real_time_size());
    let (cycle_tx, cycle_rx) = channel::<Alarm>(config.queue.cycle_size());
    let (player_tx, player_rx) = channel::<Alarm>(config.queue.player_size());
    let (ct_tx, ct_rx) = channel::<TestAlarmConfig>(10);

    let alarm_media_path = config.soundbox.alarm_media_path();
    let test_media_path = config.soundbox.test_media_path();
    let alarm_media_url = config.soundpost.alarm_media_url();
    let test_media_url = config.soundpost.test_media_url();
    let alarm_min_duration = config.alarm.alarm_min_duration();
    let test_min_duration = config.alarm.test_min_duration();
    let speech_min_duration = config.alarm.speech_min_duration();
    let play_mode = config.soundpost.play_mode();
    let soundpost = Soundpost::new(
        config.soundpost.api_host(),
        config.soundpost.api_login_token(),
    );

    let recorder = Recorder::new(
        config.recorder.record_storage_path(),
        config.recorder.record_link_path(),
    );
    let play_serivce = service.clone();

    let play = Play::new(
        alarm_media_path,
        test_media_path,
        alarm_media_url,
        test_media_url,
        alarm_min_duration,
        test_min_duration,
        speech_min_duration,
        play_mode,
        soundpost,
        recorder,
        play_serivce,
    );
    let play_clone = play.clone();
    let play_handle = tokio::spawn(async move {
        play_clone.run(cycle_tx, player_rx).await;
    });

    let shutdown = Arc::new(Notify::new());
    let sd = shutdown.clone();
    let alarm_tx = player_tx.clone();
    let real_time_service = service.clone();
    let asc_interval_secs = config.alarm.asc_interval_secs();
    let mut realtime = RealTime::new(asc_interval_secs, real_time_service);
    let real_time_handle = tokio::spawn(async move {
        realtime.run(player_tx, real_time_rx).await;
    });

    // ============================= MQTT 消息处理规则链 ===================================
    let handler = DefaultHandler::default();
    // 鸡场更新消息
    type FH = FarmConfigHandler<DefaultHandler>;
    let play_clone = play.clone();
    let service_clone = service.clone();
    let handler = FH::new(play_clone, service_clone).handler(handler);

    // 鸡舍更新消息
    type HSH = HouseSetHandler<FH>;
    let service_clone = service.clone();
    let handler = HSH::new(service_clone).handler(handler);

    // 音柱配置更新
    type SPH = SoundpostsHandler<HSH>;
    let service_clone = service.clone();
    let handler = SPH::new(service_clone).handler(handler);

    // 报警确认更新
    type ACH = AlarmConfirmHandler<SPH>;
    let service_clone = service.clone();
    let handler = ACH::new(service_clone).handler(handler);

    // 测试报警配置
    type TAH = TestAlarmHandler<ACH>;
    let handler = TAH::new(ct_tx).handler(handler);

    // 真实报警消息
    type AAH = ActAlarmHandler<TAH>;
    let play_clone = play.clone();
    let handler = AAH::new(real_time_tx.clone(), play_clone).handler(handler);
    // =========================================================================

    let test_alarm_service = service.clone();
    let mut test_alarm = TestAlarm::new(test_alarm_service);
    let test_alarm_handle = tokio::spawn(async move {
        test_alarm.run(real_time_tx, ct_rx).await;
    });

    let topics: Vec<String> = vec![
        crate::TOPIC_ALARM.to_string(),
        crate::TOPIC_REPUB_ALARM.to_string(),
        crate::TOPIC_CRONTAB.to_string(),
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

    {
        // 初始化报警表
        let mut service = service.write().await;
        if let Err(e) = service.init_alarm_set().await {
            error!("Latest alarms init failed: {e}");
        }
    }

    let cycle_service = service.clone();
    let cycle_interval_secs = config.alarm.cycle_interval_secs();
    let cycle_handle = tokio::spawn(async move {
        Cycle::init(cycle_interval_secs, cycle_service)
            .await
            .run(alarm_tx, cycle_rx, sd)
            .await;
    });

    let ws = WsClient::new(
        config.soundpost.api_host(),
        config.soundpost.ws_username(),
        config.soundpost.ws_password(),
        service,
    )
    .await
    .unwrap();
    let st = shutdown.clone();
    let ws_handle = tokio::spawn(async move {
        ws.subscribe(st).await;
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
        test_alarm_handle,
        ws_handle
    );

    info!("Notify player to cancel playing...");
    play.terminate_play().await;
    info!("Wait for player to end...");
    let _ = play_handle.await;

    info!("==================== Alarm player exited ====================");
}

#[cfg(test)]
mod app_tests {
    #[ctor::ctor]
    fn init() {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    }
}
