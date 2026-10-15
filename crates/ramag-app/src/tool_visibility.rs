use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::tool_registry::ToolRegistry;

#[derive(Debug, Default, Deserialize, Serialize)]
struct ToolVisibilityPreference {
    #[serde(default)]
    home_hidden: Vec<String>,
    #[serde(default)]
    sidebar_hidden: Vec<String>,
}

impl ToolRegistry {
    /// Apply saved hidden-entry lists; unknown IDs are ignored and new tools stay visible.
    pub fn apply_visibility_json(&self, json: &str) -> Result<bool, serde_json::Error> {
        let preference = serde_json::from_str::<ToolVisibilityPreference>(json)?;
        let home_hidden = preference.home_hidden.into_iter().collect::<HashSet<_>>();
        let sidebar_hidden = preference
            .sidebar_hidden
            .into_iter()
            .collect::<HashSet<_>>();
        let mut changed = false;
        for id in self.order() {
            changed |= self.set_home_visible(&id, !home_hidden.contains(&id));
            changed |= self.set_sidebar_visible(&id, !sidebar_hidden.contains(&id));
        }
        if changed {
            tracing::info!(
                operation = "tool_visibility_load",
                "tool visibility restored"
            );
        }
        Ok(changed)
    }

    /// Serialize hidden entries in registry order for stable local preference writes.
    pub fn visibility_json(&self) -> Result<String, serde_json::Error> {
        let mut preference = ToolVisibilityPreference::default();
        for id in self.order() {
            let Some(visibility) = self.visibility(&id) else {
                continue;
            };
            if !visibility.home {
                preference.home_hidden.push(id.clone());
            }
            if !visibility.sidebar {
                preference.sidebar_hidden.push(id);
            }
        }
        serde_json::to_string(&preference)
    }
}
