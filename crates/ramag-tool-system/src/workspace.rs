#[path = "presets.rs"]
pub(crate) mod presets;
#[path = "window_lifetime.rs"]
mod window_lifetime;
#[cfg(test)]
use crate::fixture;
use crate::{
    live::{self, LiveState},
    panel::{self, MonitorPanel, WorkspaceSkin},
    storage::{self, Storage},
};
use gpui_kit::base::{Button, ElementExt, Scrollbar, ScrollbarMode, dock::*};
use gpui_kit::component::{ActiveTheme, menu::ContextMenuExt};
use gpui_kit::{prelude::FluentBuilder, *};
use ramag_infra_system::{SamplingService, Snapshot};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};
use system_pulse_model::{
    ExpandedSize, HistoryStore, MonitorDescriptor as Monitor, Session, Workspace,
};

pub(crate) type Shared = Rc<RefCell<Data>>;
pub(crate) struct Data {
    pub(crate) session: Session,
    pub(crate) history: HistoryStore,
    pub(crate) catalog: Vec<Monitor>,
    pub(crate) owner: Option<WeakEntity<WorkspaceView>>,
    pub(crate) views: BTreeMap<String, WeakEntity<MonitorPanel>>,
    pub(crate) bounds: BTreeMap<String, Bounds<Pixels>>,
    pub(crate) scroll: ScrollHandle,
    pub(crate) processes: Vec<crate::live::ProcessView>,
    accepted_clock: Option<(std::time::Instant, u64)>,
    process_presentation_stale: Option<bool>,
    pub(crate) process_widths: [f32; 8],
    pub(crate) allow_process_actions: bool,
    pub(crate) process_action: crate::panel::ProcessActionState,
    pub(crate) snapshot: Option<std::sync::Arc<Snapshot>>,
    pub(crate) live: LiveState,
    pub(crate) presets: system_pulse_model::PresetLibrary,
    pub(crate) preset_error: Option<String>,
    pub(crate) preset_notice: String,
    pub(crate) preset_busy: bool,
}

impl Data {
    pub(crate) fn process_count(&self) -> usize {
        self.snapshot
            .as_ref()
            .map_or(self.processes.len(), |snapshot| snapshot.processes.len())
    }

    pub(crate) fn process_identities(&self) -> Vec<ramag_infra_system::ProcessIdentity> {
        if let Some(snapshot) = &self.snapshot {
            snapshot
                .processes
                .iter()
                .map(|row| row.identity.clone())
                .collect()
        } else {
            self.processes
                .iter()
                .map(|row| row.identity.clone())
                .collect()
        }
    }

    pub(crate) fn prepare_processes(&mut self) {
        let Some((accepted, collector_ms)) = self.accepted_clock else {
            return;
        };
        let now = collector_ms.saturating_add(accepted.elapsed().as_millis() as u64);
        self.prepare_processes_at(now);
    }

    fn prepare_processes_at(&mut self, now: u64) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let threshold = self.session.workspace.interval_ms * 2;
        let stale = now.saturating_sub(snapshot.capture_finished_ns / 1_000_000) > threshold;
        if self.process_presentation_stale != Some(stale) {
            let processes = live::process_views(snapshot, now, threshold);
            self.set_processes(processes);
            self.process_presentation_stale = Some(stale);
        }
    }

    fn set_processes(&mut self, processes: Vec<crate::live::ProcessView>) {
        // Keep inspected columns in place when long status text disappears.
        // Widths reset with the view and still grow to fit newly observed text.
        let measured_widths = live::process_widths(&processes);
        for (width, measured) in self.process_widths.iter_mut().zip(measured_widths) {
            *width = width.max(measured);
        }
        self.processes = processes;
    }
}

#[derive(Clone)]
pub(crate) enum Command {
    #[cfg(target_os = "windows")]
    EnableCpuTemperatures,
    #[cfg(target_os = "windows")]
    DisableCpuTemperatures,
    PanelCollapse(String),
    PanelVisible(String),
    RowCollapse(String, String),
    SensorVisible(String, String),
    SensorMove(String, String, system_pulse_model::SensorMove),
    SensorMeter(String, String, system_pulse_model::Meter),
    Meter(String, String),
    ShowSettings,
    AskResetLayout,
    CancelResetLayout,
    ResetLayout,
    Save,
    SavePreset,
    RecallPreset,
    Recover,
    Interval(u64),
    Appearance(system_pulse_model::Appearance),
    Screen(system_pulse_model::Screen),
    ScreenDevice(system_pulse_model::Screen, String),
    Preset(presets::PresetCommand),
    Scroll(f32, f32),
}

#[cfg(test)]
pub(crate) fn default_dock() -> DockAreaState {
    default_dock_for(&fixture::catalog())
}

fn default_dock_for(monitors: &[Monitor]) -> DockAreaState {
    let children = monitors
        .iter()
        .map(|monitor| {
            let mut leaf = PanelState::new("SystemPulseMonitor");
            leaf.info = PanelInfo::panel(serde_json::json!({"monitor_id": monitor.id}));
            PanelState {
                panel_name: "TabPanel".into(),
                children: vec![leaf],
                info: PanelInfo::tabs(0),
            }
        })
        .collect::<Vec<_>>();
    DockAreaState {
        version: Some(1),
        center: PanelState {
            panel_name: "StackPanel".into(),
            info: PanelInfo::stack(vec![px(280.); children.len()], Axis::Vertical),
            children,
        },
        left_dock: None,
        right_dock: None,
        bottom_dock: None,
    }
}

#[cfg(test)]
pub(crate) fn validate_dock(value: &serde_json::Value) -> Result<(), String> {
    validate_dock_mode(value, false)
}

pub(crate) fn validate_dock_mode(
    value: &serde_json::Value,
    allow_fixture: bool,
) -> Result<(), String> {
    let state: DockAreaState = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if state.left_dock.is_some() || state.right_dock.is_some() || state.bottom_dock.is_some() {
        return Err("The workspace supports center splits only".into());
    }
    fn leaves(
        node: &PanelState,
        seen: &mut BTreeSet<String>,
        allow_fixture: bool,
    ) -> Result<(), String> {
        let expected = match &node.info {
            PanelInfo::Stack { .. } => "StackPanel",
            PanelInfo::Tabs { .. } => "TabPanel",
            PanelInfo::Panel(_) => "SystemPulseMonitor",
        };
        if node.panel_name != expected {
            return Err(format!("Layout kind/name mismatch: {}", node.panel_name));
        }
        if let PanelInfo::Panel(value) = &node.info {
            let id = value["monitor_id"]
                .as_str()
                .ok_or("Missing monitor identity")?;
            if node.panel_name != "SystemPulseMonitor"
                || id.trim().is_empty()
                || (!allow_fixture && live::is_fixture_id(id))
            {
                return Err(format!("Unsupported simulated monitor identity: {id}"));
            }
            if !seen.insert(id.to_owned()) {
                return Err(format!("Duplicate monitor: {id}"));
            }
        }
        for child in &node.children {
            leaves(child, seen, allow_fixture)?;
        }
        Ok(())
    }
    leaves(&state.center, &mut BTreeSet::new(), allow_fixture)
}

fn ensure_enabled_regions(
    shared: &Shared,
    dock: &Entity<DockArea>,
    window: &mut Window,
    cx: &mut App,
) {
    fn collect(node: &PanelState, ids: &mut BTreeSet<String>) {
        if let PanelInfo::Panel(value) = &node.info
            && let Some(id) = value["monitor_id"].as_str()
        {
            ids.insert(id.to_owned());
        }
        for child in &node.children {
            collect(child, ids);
        }
    }
    let mut present = BTreeSet::new();
    collect(&dock.read(cx).dump(cx).center, &mut present);
    let missing: Vec<_> = {
        let data = shared.borrow();
        data.catalog
            .iter()
            .filter(|m| data.session.workspace.panels[&m.id].visible && !present.contains(&m.id))
            .cloned()
            .collect()
    };
    for monitor in missing {
        let entity = cx.new(|cx| MonitorPanel::new(monitor.clone(), shared.clone(), cx));
        shared
            .borrow_mut()
            .views
            .insert(monitor.id, entity.downgrade());
        dock.update(cx, |dock, cx| {
            dock.add_panel(entity, DockPlacement::Center, Some(px(280.)), window, cx)
        });
    }
}

// Divider events can precede prepaint. ResizableState's resolved split sizes are
// authoritative on those axes; cached bounds supply only unconstrained axes.
fn capture_preferences(shared: &Shared, dock: &DockAreaState) {
    fn collect(node: &PanelState, width: Option<f32>, height: Option<f32>, data: &mut Data) {
        if let PanelInfo::Panel(value) = &node.info {
            let Some(id) = value["monitor_id"].as_str() else {
                return;
            };
            if !data.catalog.iter().any(|monitor| monitor.id == id) {
                return;
            }
            let Some(panel) = data.session.workspace.panels.get_mut(id) else {
                return;
            };
            if panel.collapsed || !panel.visible {
                return;
            }
            let measured = data.bounds.get(id).map(|bounds| bounds.size);
            let width = width.or_else(|| measured.map(|size| size.width.as_f32()));
            let height = height.or_else(|| measured.map(|size| size.height.as_f32()));
            if let (Some(width), Some(height)) = (width, height)
                && width.is_finite()
                && height.is_finite()
                && width >= 320.
                && height >= 220.
            {
                panel.expanded_size = ExpandedSize {
                    width: width.min(system_pulse_model::MAX_EXPANDED_DIMENSION),
                    height: height.min(system_pulse_model::MAX_EXPANDED_DIMENSION),
                };
            }
        }
        for (ix, child) in node.children.iter().enumerate() {
            let (width, height) = match &node.info {
                PanelInfo::Stack { sizes, axis } => {
                    let allocation = sizes
                        .get(ix)
                        .map(|size| size.as_f32())
                        .filter(|size| *size > 0.);
                    if *axis == 0 {
                        (allocation.or(width), height)
                    } else {
                        (width, allocation.or(height))
                    }
                }
                _ => (width, height),
            };
            collect(child, width, height, data);
        }
    }
    collect(&dock.center, None, None, &mut shared.borrow_mut());
}

pub struct WorkspaceView {
    attached_window: Option<AnyWindowHandle>,
    pub(crate) shared: Shared,
    pub(crate) screen_view: Option<WeakEntity<crate::screens::ScreenView>>,
    pub(crate) dock: Entity<DockArea>,
    notice: String,
    directory: Option<PathBuf>,
    read_blocked: bool,
    storage: Storage,
    revision: u64,
    #[cfg(test)]
    tick: u64,
    fixture_mode: bool,
    initial_layout_pending: bool,
    service: Option<SamplingService>,
    accepted_unix_ns: u64,
    accepted_model_timing: Option<(u64, u64)>,
    diagnostic_revision: u64,
    diagnostics: Option<crate::diagnostics::Writer>,
    preset: Option<String>,
    timer: Option<Task<()>>,
    save_task: Option<Task<()>>,
    focus: FocusHandle,
    visibility_scroll: ScrollHandle,
    keyboard_repaint_pending: bool,
    visibility_controls: BTreeMap<String, crate::controls::FocusEntry>,
    confirm_layout_reset: bool,
    cancel_layout_reset_focus: FocusHandle,
}

impl Command {
    fn control_id(&self) -> String {
        match self {
            Self::PanelVisible(id) => format!("workspace:visible:{id}"),
            Self::ShowSettings => "workspace:settings".into(),
            Self::AskResetLayout => "workspace:reset-layout".into(),
            Self::CancelResetLayout => "workspace:cancel-reset-layout".into(),
            Self::ResetLayout => "workspace:confirm-reset-layout".into(),
            Self::Save => "workspace:save".into(),
            Self::SavePreset => "workspace:save-preset".into(),
            Self::RecallPreset => "workspace:recall-preset".into(),
            Self::Interval(ms) => format!("workspace:interval:{ms}"),
            Self::Recover => "workspace:recover".into(),
            _ => unreachable!("only workspace commands appear in the toolbar"),
        }
    }
}

#[path = "workspace/commands.rs"]
mod commands;
#[path = "workspace/construct.rs"]
mod construct;
#[path = "workspace/delivery.rs"]
mod delivery;
#[cfg(test)]
#[path = "workspace/tests.rs"]
mod tests;
#[path = "workspace/view.rs"]
mod view;
pub(crate) use view::{initial_catalog, restore_session, scroll_viewport};
