//! MQTT 验收使用独立存储。mqtt 模式提供本机测试 Broker 的连接配置。

use std::sync::Arc;

use ramag_app::MqttService;
use ramag_domain::{entities::MqttProfile, traits::Storage};
use ramag_infra_mqtt::{NativeMqttLocalServer, NativeMqttTransport};

pub(super) fn service(storage: Arc<dyn Storage>, seed: bool) -> anyhow::Result<Arc<MqttService>> {
    let service = Arc::new(
        MqttService::new(Arc::new(NativeMqttTransport::new()), storage)
            .with_local_server_driver(Arc::new(NativeMqttLocalServer::new())),
    );
    if seed {
        let profile = MqttProfile::new("本机 MQTT 测试 Broker", "127.0.0.1", 18883);
        futures::executor::block_on(service.save_profile(&profile))?;
    }
    Ok(service)
}
