//! 工具注册表，支持按用户布局顺序查询和动态隐藏工具入口。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;
use ramag_domain::{PluginCapability, PluginDescriptor, PluginId, PluginRegistrationError, Tool};

/// 工具入口顺序在 Storage 中使用的偏好键。
pub const TOOL_ORDER_PREF_KEY: &str = "tool_order";
/// 首页与侧边栏入口显示状态在 Storage 中使用的偏好键。
pub const TOOL_VISIBILITY_PREF_KEY: &str = "tool_visibility";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolVisibility {
    pub home: bool,
    pub sidebar: bool,
}

struct ToolEntry {
    tool: Arc<dyn Tool>,
    enabled: bool,
    home_visible: bool,
    sidebar_visible: bool,
    plugin: Option<PluginDescriptor>,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: RwLock<Vec<ToolEntry>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        let mut tools = self.tools.write();
        if tools.iter().any(|t| t.tool.meta().id == tool.meta().id) {
            tracing::warn!(
                operation = "tool_register",
                tool_id = %tool.meta().id,
                reason = "duplicate",
                "duplicate tool registration ignored"
            );
            return;
        }
        tracing::info!(
            operation = "tool_register",
            tool_id = %tool.meta().id,
            name = %tool.meta().name,
            "tool registered"
        );
        tools.push(ToolEntry {
            tool,
            enabled: true,
            home_visible: true,
            sidebar_visible: true,
            plugin: None,
        });
    }

    /// 将现有工具包装成内置插件并执行完整的描述校验。
    pub fn register_builtin(&self, tool: Arc<dyn Tool>) -> Result<(), PluginRegistrationError> {
        let meta = tool.meta();
        let plugin_id = PluginId::new(meta.id.clone())?;
        let descriptor = PluginDescriptor::new(plugin_id, meta.name.clone(), meta.id.clone())
            .with_description(meta.description.clone())
            .with_capabilities([PluginCapability::new("ui.entry")]);
        self.register_plugin(descriptor, tool)
    }

    /// 注册一个单入口静态插件；失败只返回诊断，不修改现有工具列表。
    pub fn register_plugin(
        &self,
        descriptor: PluginDescriptor,
        tool: Arc<dyn Tool>,
    ) -> Result<(), PluginRegistrationError> {
        self.register_plugin_entries(descriptor, vec![tool])
    }

    /// 原子注册一个插件的全部工具入口；任一入口不匹配时整组保持未注册。
    pub fn register_plugin_entries(
        &self,
        descriptor: PluginDescriptor,
        plugin_tools: Vec<Arc<dyn Tool>>,
    ) -> Result<(), PluginRegistrationError> {
        descriptor.validate()?;
        let entries = descriptor.entry_descriptors();
        if entries.len() != plugin_tools.len() {
            return Err(PluginRegistrationError::EntryCountMismatch {
                plugin_id: descriptor.id.clone(),
                expected: entries.len(),
                actual: plugin_tools.len(),
            });
        }
        let entry_ids = entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<HashSet<_>>();
        let mut tool_ids = HashSet::with_capacity(plugin_tools.len());
        for tool in &plugin_tools {
            let tool_id = tool.meta().id.as_str();
            if !entry_ids.contains(tool_id) {
                return Err(PluginRegistrationError::EntryIdMismatch {
                    plugin_id: descriptor.id.clone(),
                    entry_id: descriptor.entry_id.clone(),
                    tool_id: tool_id.to_owned(),
                });
            }
            if !tool_ids.insert(tool_id) {
                return Err(PluginRegistrationError::DuplicateEntryId {
                    entry_id: tool_id.to_owned(),
                });
            }
        }
        for entry in &entries {
            if !tool_ids.contains(entry.id.as_str()) {
                return Err(PluginRegistrationError::MissingToolEntry {
                    plugin_id: descriptor.id.clone(),
                    entry_id: entry.id.clone(),
                });
            }
        }
        let ordered_tools = entries
            .iter()
            .filter_map(|entry| {
                plugin_tools
                    .iter()
                    .find(|tool| tool.meta().id == entry.id)
                    .cloned()
            })
            .collect::<Vec<_>>();

        let mut tools = self.tools.write();
        if tools.iter().any(|entry| {
            entry
                .plugin
                .as_ref()
                .is_some_and(|plugin| plugin.id == descriptor.id)
        }) {
            return Err(PluginRegistrationError::DuplicatePluginId {
                id: descriptor.id.clone(),
            });
        }
        if let Some(duplicate) = tools
            .iter()
            .find(|entry| entry_ids.contains(entry.tool.meta().id.as_str()))
        {
            return Err(PluginRegistrationError::DuplicateEntryId {
                entry_id: duplicate.tool.meta().id.clone(),
            });
        }
        tracing::info!(
            operation = "plugin_register",
            plugin_id = %descriptor.id,
            entry_count = entries.len(),
            "static plugin registered"
        );
        for tool in ordered_tools {
            tools.push(ToolEntry {
                tool,
                enabled: true,
                home_visible: true,
                sidebar_visible: true,
                plugin: Some(descriptor.clone()),
            });
        }
        Ok(())
    }

    /// 返回已通过校验并注册的静态插件描述，不暴露工具运行时状态。
    pub fn plugin_descriptors(&self) -> Vec<PluginDescriptor> {
        let mut seen = HashSet::new();
        self.tools
            .read()
            .iter()
            .filter_map(|entry| {
                let plugin = entry.plugin.clone()?;
                seen.insert(plugin.id.clone()).then_some(plugin)
            })
            .collect()
    }

    /// 移除一个已注册的静态插件；普通工具和其他插件不受影响。
    pub fn unregister_plugin(&self, plugin_id: &str) -> bool {
        let mut tools = self.tools.write();
        let before = tools.len();
        tools.retain(|entry| {
            !entry
                .plugin
                .as_ref()
                .is_some_and(|plugin| plugin.id.as_str() == plugin_id)
        });
        if before == tools.len() {
            return false;
        }
        tracing::info!(
            operation = "plugin_unregister",
            plugin_id,
            "static plugin removed from tool registry"
        );
        true
    }

    /// 设置工具入口可见性，返回状态是否变化；未注册时返回 `false`。
    pub fn set_enabled(&self, id: &str, enabled: bool) -> bool {
        let mut tools = self.tools.write();
        let Some(entry) = tools.iter_mut().find(|t| t.tool.meta().id == id) else {
            return false;
        };
        if entry.enabled == enabled {
            return false;
        }
        entry.enabled = enabled;
        tracing::info!(
            operation = "tool_visibility_update",
            tool_id = %id,
            enabled,
            "tool visibility changed"
        );
        true
    }

    /// 返回所有已注册工具，包括被全局停用或在两个表面隐藏的工具。
    pub fn list_all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .read()
            .iter()
            .map(|entry| entry.tool.clone())
            .collect()
    }

    /// 返回当前可显示在首页的工具，顺序与注册表布局一致。
    pub fn home_list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .read()
            .iter()
            .filter(|entry| entry.enabled && entry.home_visible)
            .map(|entry| entry.tool.clone())
            .collect()
    }

    /// 返回当前可显示在侧边栏的工具，顺序与注册表布局一致。
    pub fn sidebar_list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .read()
            .iter()
            .filter(|entry| entry.enabled && entry.sidebar_visible)
            .map(|entry| entry.tool.clone())
            .collect()
    }

    /// 查询一个工具在首页和侧边栏的独立显示状态。
    pub fn visibility(&self, id: &str) -> Option<ToolVisibility> {
        self.tools
            .read()
            .iter()
            .find(|entry| entry.tool.meta().id == id)
            .map(|entry| ToolVisibility {
                home: entry.home_visible,
                sidebar: entry.sidebar_visible,
            })
    }

    /// 修改首页入口显示状态；工具或状态不存在变化时返回 `false`。
    pub fn set_home_visible(&self, id: &str, visible: bool) -> bool {
        self.set_surface_visibility(id, visible, |entry| &mut entry.home_visible, "home")
    }

    /// 修改侧边栏入口显示状态；工具或状态不存在变化时返回 `false`。
    pub fn set_sidebar_visible(&self, id: &str, visible: bool) -> bool {
        self.set_surface_visibility(id, visible, |entry| &mut entry.sidebar_visible, "sidebar")
    }

    /// 判断工具是否至少在一个表面可访问，用于启动时恢复最近工作区。
    pub fn is_visible(&self, id: &str) -> bool {
        self.tools.read().iter().any(|entry| {
            entry.enabled
                && entry.tool.meta().id == id
                && (entry.home_visible || entry.sidebar_visible)
        })
    }

    /// 按当前布局顺序返回已启用的工具。
    pub fn list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .read()
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.tool.clone())
            .collect()
    }

    /// 返回全部工具的当前顺序，包含暂时隐藏的工具以兼容平台差异。
    pub fn order(&self) -> Vec<String> {
        self.tools
            .read()
            .iter()
            .map(|entry| entry.tool.meta().id.clone())
            .collect()
    }

    /// 将启动时读取的 JSON 顺序应用到注册表；未知 ID 会被忽略。
    pub fn apply_order_json(&self, json: &str) -> Result<bool, serde_json::Error> {
        let order = serde_json::from_str::<Vec<String>>(json)?;
        Ok(self.apply_order(&order))
    }

    /// 按偏好中的 ID 排序，同时保留未出现在偏好中的新工具及其注册顺序。
    pub fn apply_order(&self, order: &[String]) -> bool {
        if order.is_empty() {
            return false;
        }

        let ranks: HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect();
        let mut tools = self.tools.write();
        let previous = tools
            .iter()
            .map(|entry| entry.tool.meta().id.clone())
            .collect::<Vec<_>>();
        tools.sort_by_key(|entry| {
            ranks
                .get(entry.tool.meta().id.as_str())
                .copied()
                .unwrap_or(order.len())
        });
        let changed = previous
            != tools
                .iter()
                .map(|entry| entry.tool.meta().id.clone())
                .collect::<Vec<_>>();
        if changed {
            tracing::info!(operation = "tool_order_load", "tool layout restored");
        }
        changed
    }

    /// 将一个可见工具插入另一个可见工具之前或之后，并返回顺序是否改变。
    pub fn reorder(&self, dragged_id: &str, target_id: &str, before: bool) -> bool {
        if dragged_id == target_id {
            return false;
        }

        let mut tools = self.tools.write();
        let Some(dragged_index) = tools
            .iter()
            .position(|entry| entry.enabled && entry.tool.meta().id == dragged_id)
        else {
            return false;
        };
        if !tools
            .iter()
            .any(|entry| entry.enabled && entry.tool.meta().id == target_id)
        {
            return false;
        }

        let previous_order = tools
            .iter()
            .map(|entry| entry.tool.meta().id.clone())
            .collect::<Vec<_>>();
        let dragged = tools.remove(dragged_index);
        let Some(target_index) = tools
            .iter()
            .position(|entry| entry.enabled && entry.tool.meta().id == target_id)
        else {
            let restore_index = dragged_index.min(tools.len());
            tools.insert(restore_index, dragged);
            return false;
        };
        let insert_index = if before {
            target_index
        } else {
            target_index + 1
        };
        let insert_index = insert_index.min(tools.len());
        tools.insert(insert_index, dragged);
        let changed = previous_order
            != tools
                .iter()
                .map(|entry| entry.tool.meta().id.clone())
                .collect::<Vec<_>>();
        if !changed {
            return false;
        }
        tracing::info!(
            operation = "tool_order_update",
            dragged_id,
            target_id,
            before,
            "tool layout changed"
        );
        true
    }

    /// 将工具移动到目标工具当前所在的位置，适合整项拖拽而不是按半区插入。
    pub fn reorder_to_target(&self, dragged_id: &str, target_id: &str) -> bool {
        if dragged_id == target_id {
            return false;
        }

        let (dragged_index, target_index) = {
            let tools = self.tools.read();
            let Some(dragged_index) = tools
                .iter()
                .position(|entry| entry.enabled && entry.tool.meta().id == dragged_id)
            else {
                return false;
            };
            let Some(target_index) = tools
                .iter()
                .position(|entry| entry.enabled && entry.tool.meta().id == target_id)
            else {
                return false;
            };
            (dragged_index, target_index)
        };

        // Keep the dragged item in the target's original slot: insert after the
        // target when it started before it, and before it when it started after it.
        self.reorder(dragged_id, target_id, dragged_index > target_index)
    }

    /// 按可见工具列表中的最终槽位移动工具，隐藏工具仍保留在注册表中。
    /// `visible_index` 可以等于可见工具数量，表示移动到列表末尾。
    pub fn reorder_to_index(&self, dragged_id: &str, visible_index: usize) -> bool {
        self.reorder_to_index_where(dragged_id, visible_index, |entry| entry.enabled)
    }

    /// 按首页当前可见槽位移动工具，侧边栏专属隐藏项不占首页的目标索引。
    pub fn reorder_home_to_index(&self, dragged_id: &str, visible_index: usize) -> bool {
        self.reorder_to_index_where(dragged_id, visible_index, |entry| {
            entry.enabled && entry.home_visible
        })
    }

    /// 按侧边栏当前可见槽位移动工具，首页专属隐藏项不占侧边栏的目标索引。
    pub fn reorder_sidebar_to_index(&self, dragged_id: &str, visible_index: usize) -> bool {
        self.reorder_to_index_where(dragged_id, visible_index, |entry| {
            entry.enabled && entry.sidebar_visible
        })
    }

    fn reorder_to_index_where(
        &self,
        dragged_id: &str,
        visible_index: usize,
        is_visible: impl Fn(&ToolEntry) -> bool,
    ) -> bool {
        let mut tools = self.tools.write();
        let Some(dragged_index) = tools
            .iter()
            .position(|entry| is_visible(entry) && entry.tool.meta().id == dragged_id)
        else {
            return false;
        };

        let previous_order = tools
            .iter()
            .map(|entry| entry.tool.meta().id.clone())
            .collect::<Vec<_>>();
        let dragged = tools.remove(dragged_index);
        let remaining_visible_count = tools.iter().filter(|entry| is_visible(entry)).count();
        let target_index = visible_index.min(remaining_visible_count);
        let insert_index = if target_index == remaining_visible_count {
            tools.len()
        } else {
            tools
                .iter()
                .enumerate()
                .filter(|(_, entry)| is_visible(entry))
                .nth(target_index)
                .map_or(tools.len(), |(index, _)| index)
        };
        tools.insert(insert_index, dragged);

        let changed = previous_order
            != tools
                .iter()
                .map(|entry| entry.tool.meta().id.clone())
                .collect::<Vec<_>>();
        if changed {
            tracing::info!(
                operation = "tool_order_update",
                dragged_id,
                visible_index,
                "tool layout changed"
            );
        }
        changed
    }

    /// 将可见工具移动到当前所有可见工具的末尾，供末尾整项落点使用。
    pub fn move_to_end(&self, dragged_id: &str) -> bool {
        let target_id = {
            let tools = self.tools.read();
            tools
                .iter()
                .rev()
                .find(|entry| entry.enabled)
                .map(|entry| entry.tool.meta().id.clone())
        };
        target_id.is_some_and(|target_id| self.reorder(dragged_id, &target_id, false))
    }

    pub fn find(&self, id: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .read()
            .iter()
            .find(|t| t.enabled && t.tool.meta().id == id)
            .map(|t| t.tool.clone())
    }

    pub fn count(&self) -> usize {
        self.tools.read().iter().filter(|t| t.enabled).count()
    }

    /// Update one registered entry under a single write lock and log only state changes.
    fn set_surface_visibility(
        &self,
        id: &str,
        visible: bool,
        field: impl Fn(&mut ToolEntry) -> &mut bool,
        surface: &'static str,
    ) -> bool {
        let mut tools = self.tools.write();
        let Some(entry) = tools.iter_mut().find(|entry| entry.tool.meta().id == id) else {
            return false;
        };
        let current = field(entry);
        if *current == visible {
            return false;
        }
        *current = visible;
        tracing::info!(
            operation = "tool_surface_visibility_update",
            tool_id = %id,
            surface,
            visible,
            "tool surface visibility changed"
        );
        true
    }
}

#[cfg(test)]
#[path = "tool_registry_tests.rs"]
mod tests;
