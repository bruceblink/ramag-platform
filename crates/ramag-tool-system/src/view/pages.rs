//! Focused page renderers keep the monitor UI readable and below source limits.

mod devices;
mod processes;
mod settings;
mod summary;

use gpui_kit::AnyElement;
use gpui_kit::{Context, Window};

use super::{SystemSection, SystemView};

impl SystemView {
    /// Routes each stable tab to its domain-focused page renderer.
    pub(super) fn render_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let snapshot = self.monitor.snapshot();
        match self.section {
            SystemSection::Summary => self.render_summary(&snapshot, window, cx),
            SystemSection::Cpu => {
                self.render_device_page(SystemSection::Cpu, &snapshot, window, cx)
            }
            SystemSection::Memory => {
                self.render_device_page(SystemSection::Memory, &snapshot, window, cx)
            }
            SystemSection::Gpu => {
                self.render_device_page(SystemSection::Gpu, &snapshot, window, cx)
            }
            SystemSection::Disks => {
                self.render_device_page(SystemSection::Disks, &snapshot, window, cx)
            }
            SystemSection::Network => {
                self.render_device_page(SystemSection::Network, &snapshot, window, cx)
            }
            SystemSection::Energy => {
                self.render_sensor_kind_page(SystemSection::Energy, &snapshot, cx)
            }
            SystemSection::Thermals => {
                self.render_sensor_kind_page(SystemSection::Thermals, &snapshot, cx)
            }
            SystemSection::Processes => self.render_processes(&snapshot, window, cx),
            SystemSection::Settings => self.render_settings(&snapshot, cx),
        }
    }
}
