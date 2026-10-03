use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    #[default]
    Summary,
    Cpu,
    Memory,
    Gpu,
    Disks,
    Network,
    Energy,
    Thermals,
    Processes,
    /// Retained only to read old workspace files; it is no longer navigable.
    Settings,
}

impl Screen {
    /// Screens exposed by the current monitor UI. `Settings` is a legacy
    /// compatibility value and must not be added back to this navigation list.
    pub const ALL: [Self; 9] = [
        Self::Summary,
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::Disks,
        Self::Network,
        Self::Energy,
        Self::Thermals,
        Self::Processes,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Gpu => "gpu",
            Self::Disks => "disks",
            Self::Network => "network",
            Self::Energy => "energy",
            Self::Thermals => "thermals",
            Self::Processes => "processes",
            Self::Settings => "settings",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Gpu => "GPU",
            Self::Disks => "Disks",
            Self::Network => "Network",
            Self::Energy => "Energy",
            Self::Thermals => "Thermals",
            Self::Processes => "Processes",
            Self::Settings => "Settings",
        }
    }

    pub fn has_device_selection(self) -> bool {
        matches!(
            self,
            Self::Gpu | Self::Disks | Self::Network | Self::Energy | Self::Thermals
        )
    }

    pub fn adjacent(self, forward: bool) -> Self {
        if self == Self::Settings {
            return Self::Summary;
        }
        let index = Self::ALL
            .iter()
            .position(|screen| *screen == self)
            .unwrap_or(0);
        Self::ALL[(index + if forward { 1 } else { Self::ALL.len() - 1 }) % Self::ALL.len()]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenState {
    #[serde(deserialize_with = "deserialize_active_screen")]
    pub active: Screen,
    pub devices: BTreeMap<Screen, String>,
}

/// Map the removed monitor Settings tab to Summary while reading old state.
fn deserialize_active_screen<'de, D>(deserializer: D) -> Result<Screen, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let screen = Screen::deserialize(deserializer)?;
    Ok(match screen {
        Screen::Settings => Screen::Summary,
        screen => screen,
    })
}

impl ScreenState {
    pub(crate) fn validate(&self) -> Result<(), String> {
        for (screen, id) in &self.devices {
            if !screen.has_device_selection() || id.trim().is_empty() {
                return Err(format!("Invalid device selection for {}", screen.title()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Screen, ScreenState};

    #[test]
    fn legacy_settings_screen_is_migrated_to_summary() -> Result<(), Box<dyn std::error::Error>> {
        let state: ScreenState = serde_json::from_str(r#"{"active":"settings","devices":{}}"#)?;
        assert_eq!(state.active, Screen::Summary);
        assert!(!Screen::ALL.contains(&Screen::Settings));
        assert_eq!(
            serde_json::to_value(state)?["active"],
            serde_json::Value::String("summary".into())
        );
        Ok(())
    }
}
