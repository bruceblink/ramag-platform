//! 第一方插件目录；只记录已由静态宿主注册和审查的入口，不负责执行代码。

use std::collections::HashSet;

use ramag_domain::{
    PluginApiVersion, PluginCapability, PluginDescriptor, PluginEntryDescriptor, PluginId,
};
use thiserror::Error;

/// 目录最多保留的第一方入口数量。
pub const MAX_PLUGIN_CATALOG_ENTRIES: usize = 256;

/// 入口读取和写入数据的处理范围；目录展示该边界，不代替运行时权限检查。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginDataHandling {
    LocalOnly,
}

impl std::fmt::Display for PluginDataHandling {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::LocalOnly => "本机处理，不自动同步",
        })
    }
}

/// 目录中的审核状态；只有已审核入口才允许进入第一方目录。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCatalogAcceptance {
    Verified,
}

impl std::fmt::Display for PluginCatalogAcceptance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Verified => "已审核",
        })
    }
}

/// 一个稳定工具入口的目录记录；平台字段用于区分桌面原生和 Web/WASM 适配。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCatalogEntry {
    pub plugin_id: PluginId,
    pub entry_id: String,
    pub name: String,
    pub api_version: PluginApiVersion,
    pub desktop: bool,
    pub web: bool,
    pub capabilities: Vec<PluginCapability>,
    pub data_handling: PluginDataHandling,
    pub acceptance: PluginCatalogAcceptance,
}

impl PluginCatalogEntry {
    /// 从已校验清单创建桌面原生目录项；默认不宣称 Web 支持。
    pub fn desktop_only(descriptor: &PluginDescriptor, entry: &PluginEntryDescriptor) -> Self {
        Self {
            plugin_id: descriptor.id.clone(),
            entry_id: entry.id.clone(),
            name: entry.name.clone(),
            api_version: descriptor.api_version,
            desktop: true,
            web: false,
            capabilities: descriptor.capabilities.clone(),
            data_handling: PluginDataHandling::LocalOnly,
            acceptance: PluginCatalogAcceptance::Verified,
        }
    }

    pub fn with_web_support(mut self, web: bool) -> Self {
        self.web = web;
        self
    }
}

#[derive(Debug, Default, Clone)]
pub struct PluginCatalog {
    entries: Vec<PluginCatalogEntry>,
}

impl PluginCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// 原子加入一组目录项；重复插件/入口或超量时整组不写入。
    pub fn register(&mut self, entries: Vec<PluginCatalogEntry>) -> Result<(), PluginCatalogError> {
        if self.entries.len().saturating_add(entries.len()) > MAX_PLUGIN_CATALOG_ENTRIES {
            return Err(PluginCatalogError::TooManyEntries {
                max: MAX_PLUGIN_CATALOG_ENTRIES,
            });
        }
        let mut keys = self
            .entries
            .iter()
            .map(|entry| (entry.plugin_id.clone(), entry.entry_id.clone()))
            .collect::<HashSet<_>>();
        for entry in &entries {
            if !keys.insert((entry.plugin_id.clone(), entry.entry_id.clone())) {
                return Err(PluginCatalogError::DuplicateEntry {
                    plugin_id: entry.plugin_id.clone(),
                    entry_id: entry.entry_id.clone(),
                });
            }
        }
        self.entries.extend(entries);
        Ok(())
    }

    pub fn remove_plugin(&mut self, plugin_id: &str) {
        self.entries
            .retain(|entry| entry.plugin_id.as_str() != plugin_id);
    }

    pub fn entries(&self) -> Vec<PluginCatalogEntry> {
        self.entries.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginCatalogError {
    #[error("第一方目录入口超过 {max} 项")]
    TooManyEntries { max: usize },
    #[error("第一方目录入口重复：插件 `{plugin_id}` / 入口 `{entry_id}`")]
    DuplicateEntry {
        plugin_id: PluginId,
        entry_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_domain::PluginId;

    fn descriptor(plugin_id: &str, entry_id: &str) -> PluginDescriptor {
        PluginDescriptor::new(
            PluginId::new(plugin_id).expect("valid plugin ID"),
            "Catalog test",
            entry_id,
        )
    }

    #[test]
    fn catalog_registers_reviewed_entries_atomically_and_removes_plugin() {
        let descriptor = descriptor("catalog.test", "catalog.entry");
        let entry =
            PluginCatalogEntry::desktop_only(&descriptor, &descriptor.entry_descriptors()[0]);
        let mut catalog = PluginCatalog::new();
        catalog
            .register(vec![entry.clone()])
            .expect("entry registers");
        assert_eq!(catalog.entries(), vec![entry.clone()]);
        assert!(matches!(
            catalog.register(vec![entry]),
            Err(PluginCatalogError::DuplicateEntry { .. })
        ));
        catalog.remove_plugin("catalog.test");
        assert!(catalog.entries().is_empty());
    }
}
