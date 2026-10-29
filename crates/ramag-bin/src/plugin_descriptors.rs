//! First-party plugin descriptors kept outside the application composition root.

use ramag_tool_object_storage::ObjectStorageTool;

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
    use super::object_storage_plugin_descriptor;

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
