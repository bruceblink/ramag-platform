//! 本机 Docker Mosquitto 数据面集成测试。

#![cfg(feature = "native")]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{TrySendError, sync_channel},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ramag_domain::entities::{
    MqttMessageSink, MqttMessageSinkResult, MqttProfile, MqttProtocolVersion, MqttPublishRequest,
    MqttQos, MqttSubscribeRequest, MqttSubscription, MqttUserProperty,
};
use ramag_domain::traits::MqttDriver;
use ramag_infra_mqtt::NativeMqttTransport;

/// 只从专用本机 Docker 测试环境读取连接参数，避免误连到用户保存的 Broker。
fn docker_profile(protocol_version: MqttProtocolVersion) -> Option<MqttProfile> {
    let host = std::env::var("RAMAG_TEST_MQTT_HOST").ok()?;
    let port = std::env::var("RAMAG_TEST_MQTT_PORT").ok()?.parse().ok()?;
    let mut profile = MqttProfile::new("ramag Docker MQTT test", host, port);
    profile.protocol_version = protocol_version;
    profile.keep_alive_seconds = 5;
    Some(profile)
}

/// 生成唯一测试 Topic，避免本次运行读取其他测试留下的保留消息。
fn test_topic() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("ramag/test/{}/{}", std::process::id(), nanos)
}

/// 验证两个协议版本、QoS 1、MQTT 5 属性与空闲订阅取消均可通过本机 Mosquitto 完成。
#[test]
fn docker_mosquitto_delivers_messages_and_stops_idle_subscriptions() -> Result<(), String> {
    let Some(v5_profile) = docker_profile(MqttProtocolVersion::V5) else {
        eprintln!(
            "Skipping Docker MQTT integration test; set RAMAG_TEST_MQTT_HOST and RAMAG_TEST_MQTT_PORT or run scripts/mqtt-test/mqtt-test.ps1 test."
        );
        return Ok(());
    };
    let Some(v311_profile) = docker_profile(MqttProtocolVersion::V311) else {
        return Ok(());
    };

    let driver = NativeMqttTransport::new();
    smol::block_on(driver.test_connection(&v5_profile))
        .map_err(|error| format!("MQTT 5 应连接到本机 Mosquitto: {error}"))?;
    smol::block_on(driver.test_connection(&v311_profile))
        .map_err(|error| format!("MQTT 3.1.1 应连接到本机 Mosquitto: {error}"))?;

    let topic = test_topic();
    let cancelled = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = sync_channel(1);
    let sink: MqttMessageSink = Arc::new(move |message| match sender.try_send(message) {
        Ok(()) => MqttMessageSinkResult::Accepted,
        Err(TrySendError::Full(_)) => MqttMessageSinkResult::Backpressured,
        Err(TrySendError::Disconnected(_)) => MqttMessageSinkResult::Closed,
    });
    let subscription_profile = v5_profile.clone();
    let subscription_cancelled = cancelled.clone();
    let subscription_topic = topic.clone();
    let subscription = std::thread::spawn(move || {
        smol::block_on(NativeMqttTransport::new().subscribe(
            &subscription_profile,
            &MqttSubscribeRequest {
                subscriptions: vec![MqttSubscription {
                    filter: subscription_topic,
                    qos: MqttQos::AtLeastOnce,
                    no_local: false,
                }],
            },
            sink,
            Arc::new(|_| {}),
            subscription_cancelled,
        ))
    });

    std::thread::sleep(Duration::from_millis(200));
    if subscription.is_finished() {
        let result = subscription
            .join()
            .map_err(|_| "MQTT 订阅线程不应 panic".to_string())?;
        return Err(format!("MQTT 订阅在发布前意外结束: {result:?}"));
    }
    let payload = b"docker mqtt payload".to_vec();
    let user_properties = vec![MqttUserProperty {
        name: "source".into(),
        value: "ramag-integration".into(),
    }];
    let publish_result = smol::block_on(driver.publish(
        &v5_profile,
        &MqttPublishRequest {
            topic: topic.clone(),
            payload: payload.clone(),
            qos: MqttQos::AtLeastOnce,
            retain: true,
            user_properties: user_properties.clone(),
        },
    ))
    .map_err(|error| format!("MQTT 5 发布到本机 Mosquitto 应成功: {error}"))?;
    if publish_result.packet_id.is_none() || publish_result.qos != MqttQos::AtLeastOnce {
        return Err(format!("MQTT QoS 1 发布回执无效: {publish_result:?}"));
    }

    let received = match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(message) => message,
        Err(error) => {
            let result = subscription
                .join()
                .map_err(|_| "MQTT 订阅线程不应 panic".to_string())?;
            return Err(format!(
                "订阅应在 5 秒内收到本机 Mosquitto 消息: {error}; subscription={result:?}"
            ));
        }
    };
    if received.topic != topic
        || received.payload != payload
        || received.qos != MqttQos::AtLeastOnce
        || received.user_properties != user_properties
    {
        return Err(format!("收到的 MQTT 5 消息与发布请求不一致: {received:?}"));
    }

    cancelled.store(true, Ordering::Release);
    let cancellation_started = Instant::now();
    let subscription_result = subscription
        .join()
        .map_err(|_| "MQTT 订阅线程不应 panic".to_string())?;
    subscription_result.map_err(|error| format!("取消空闲 MQTT 订阅不应失败: {error}"))?;
    if cancellation_started.elapsed() > Duration::from_secs(2) {
        return Err("空闲 MQTT 订阅取消超过 2 秒，不能及时释放运行时".into());
    }

    smol::block_on(driver.publish(
        &v5_profile,
        &MqttPublishRequest {
            topic,
            payload: Vec::new(),
            qos: MqttQos::AtLeastOnce,
            retain: true,
            user_properties: Vec::new(),
        },
    ))
    .map_err(|error| format!("应清理本次 Docker MQTT 测试保留消息: {error}"))?;
    Ok(())
}
