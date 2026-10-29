//! First-party plugin descriptors kept outside the application composition root.

use std::sync::Arc;

use super::{StaticPluginHost, register_first_party_tool};
use ramag_tool_container::ContainerTool;
use ramag_tool_dbclient::DbClientTool;
use ramag_tool_kafka::KafkaTool;
use ramag_tool_object_storage::ObjectStorageTool;

/// 注册 Kafka 第一方适配；保留集群、Topic、消息和管理工作台路径。
pub(super) fn register_kafka_tool(host: &StaticPluginHost) {
    register_first_party_tool(
        host,
        Arc::new(KafkaTool::new()),
        kafka_plugin_descriptor(),
        "kafka_plugin_descriptor",
        "kafka_plugin_register",
    );
}

/// Kafka 工具的稳定身份和消息管理边界；连接和管理操作仍由现有服务控制。
pub(super) fn kafka_plugin_descriptor()
-> Result<ramag_domain::PluginDescriptor, ramag_domain::PluginRegistrationError> {
    let id = ramag_domain::PluginId::new(KafkaTool::ID)?;
    Ok(
        ramag_domain::PluginDescriptor::new(id, "Kafka", KafkaTool::ID)
            .with_description(
                "浏览和管理 Kafka 集群、Topic、Partition 与消息；请求由用户显式触发。",
            )
            .with_entries(vec![
                ramag_domain::PluginEntryDescriptor::new(KafkaTool::ID, "Kafka")
                    .with_description("Kafka 集群、消息和管理工作台。"),
            ]),
    )
}

/// 注册数据库客户端第一方适配；保留多数据库连接、查询和同步工作台路径。
pub(super) fn register_dbclient_tool(host: &StaticPluginHost) {
    register_first_party_tool(
        host,
        Arc::new(DbClientTool::new()),
        dbclient_plugin_descriptor(),
        "dbclient_plugin_descriptor",
        "dbclient_plugin_register",
    );
}

/// 数据库客户端工具的稳定身份和连接边界；连接和查询仍由现有服务控制。
pub(super) fn dbclient_plugin_descriptor()
-> Result<ramag_domain::PluginDescriptor, ramag_domain::PluginRegistrationError> {
    let id = ramag_domain::PluginId::new(DbClientTool::ID)?;
    Ok(
        ramag_domain::PluginDescriptor::new(id, "数据库客户端", DbClientTool::ID)
            .with_description(
                "MySQL、PostgreSQL、Redis 和 MongoDB 统一工作台；连接和查询由用户显式触发。",
            )
            .with_entries(vec![
                ramag_domain::PluginEntryDescriptor::new(DbClientTool::ID, "数据库客户端")
                    .with_description("连接、查询和数据同步工作台。"),
            ]),
    )
}

/// 容器管理工具的稳定身份和资源操作边界；连接和写入操作仍由现有服务控制。
pub(super) fn container_plugin_descriptor()
-> Result<ramag_domain::PluginDescriptor, ramag_domain::PluginRegistrationError> {
    let id = ramag_domain::PluginId::new(ContainerTool::ID)?;
    Ok(
        ramag_domain::PluginDescriptor::new(id, "容器管理", ContainerTool::ID)
            .with_description("查看 Docker 与 Kubernetes 资源；连接和资源操作由用户显式触发。")
            .with_entries(vec![
                ramag_domain::PluginEntryDescriptor::new(ContainerTool::ID, "容器管理")
                    .with_description("容器、镜像、网络和存储工作台。"),
            ]),
    )
}

/// 对象存储工具的稳定身份和云存储边界；连接和对象操作仍由现有服务控制。
pub(super) fn object_storage_plugin_descriptor()
-> Result<ramag_domain::PluginDescriptor, ramag_domain::PluginRegistrationError> {
    let id = ramag_domain::PluginId::new(ObjectStorageTool::ID)?;
    Ok(
        ramag_domain::PluginDescriptor::new(id, "云存储", ObjectStorageTool::ID)
            .with_description("管理 COS、OSS 与 S3 兼容存储；连接和对象操作由用户显式触发。")
            .with_entries(vec![
                ramag_domain::PluginEntryDescriptor::new(ObjectStorageTool::ID, "云存储")
                    .with_description("账号、对象列表和上传下载工作台。"),
            ]),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        container_plugin_descriptor, dbclient_plugin_descriptor, kafka_plugin_descriptor,
        object_storage_plugin_descriptor,
    };

    #[test]
    fn kafka_plugin_descriptor_preserves_stable_entry_and_minimal_capability() {
        let result = kafka_plugin_descriptor();
        assert!(
            result.is_ok(),
            "Kafka 插件描述应始终有效：{:?}",
            result.err()
        );
        let Some(descriptor) = result.ok() else {
            return;
        };

        assert_eq!(descriptor.id.as_str(), "kafka");
        assert_eq!(descriptor.entry_id, "kafka");
        assert_eq!(descriptor.entry_descriptors().len(), 1);
        assert_eq!(descriptor.capabilities.len(), 1);
        assert_eq!(descriptor.capabilities[0].as_str(), "ui.entry");
        assert!(descriptor.settings.is_empty());
    }

    #[test]
    fn dbclient_plugin_descriptor_preserves_stable_entry_and_minimal_capability() {
        let result = dbclient_plugin_descriptor();
        assert!(
            result.is_ok(),
            "数据库客户端插件描述应始终有效：{:?}",
            result.err()
        );
        let Some(descriptor) = result.ok() else {
            return;
        };

        assert_eq!(descriptor.id.as_str(), "dbclient");
        assert_eq!(descriptor.entry_id, "dbclient");
        assert_eq!(descriptor.entry_descriptors().len(), 1);
        assert_eq!(descriptor.capabilities.len(), 1);
        assert_eq!(descriptor.capabilities[0].as_str(), "ui.entry");
        assert!(descriptor.settings.is_empty());
    }

    #[test]
    fn container_plugin_descriptor_preserves_stable_entry_and_minimal_capability() {
        let result = container_plugin_descriptor();
        assert!(result.is_ok(), "容器插件描述应始终有效：{:?}", result.err());
        let Some(descriptor) = result.ok() else {
            return;
        };

        assert_eq!(descriptor.id.as_str(), "container");
        assert_eq!(descriptor.entry_id, "container");
        assert_eq!(descriptor.entry_descriptors().len(), 1);
        assert_eq!(descriptor.capabilities.len(), 1);
        assert_eq!(descriptor.capabilities[0].as_str(), "ui.entry");
        assert!(descriptor.settings.is_empty());
    }

    #[test]
    fn object_storage_plugin_descriptor_preserves_stable_entry_and_minimal_capability() {
        let result = object_storage_plugin_descriptor();
        assert!(
            result.is_ok(),
            "对象存储插件描述应始终有效：{:?}",
            result.err()
        );
        let Some(descriptor) = result.ok() else {
            return;
        };

        assert_eq!(descriptor.id.as_str(), "object_storage");
        assert_eq!(descriptor.entry_id, "object_storage");
        assert_eq!(descriptor.entry_descriptors().len(), 1);
        assert_eq!(descriptor.capabilities.len(), 1);
        assert_eq!(descriptor.capabilities[0].as_str(), "ui.entry");
        assert!(descriptor.settings.is_empty());
    }
}
