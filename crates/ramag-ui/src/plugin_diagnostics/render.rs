use super::*;

use gpui_kit::component::v_flex;
use gpui_kit::{IntoElement, Render, Window, px};
use ramag_app::PluginState;

impl Render for PluginDiagnosticsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let diagnostics = self.host.diagnostics();
        let catalog = self.host.catalog();
        let available = self.host.registry().list();
        let failed_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.failure.is_some())
            .count();
        let ready_count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.state == PluginState::Ready)
            .count();

        v_flex()
            .id("plugin-diagnostics")
            .debug_selector(|| "plugin-diagnostics".into())
            .w_full()
            .min_w_0()
            .gap(px(12.0))
            .child(render_summary(
                diagnostics.len(),
                ready_count,
                failed_count,
                cx,
            ))
            .child(render_installations(
                &self.installations,
                self.installation_store.is_some(),
                self.package_installer.is_some(),
                InstallationRenderState {
                    pending_uninstall: self.pending_uninstall.as_ref(),
                    operation_error: self.installation_error.as_deref(),
                    installing_archive: self.installing_archive,
                    package_operation_busy: self.downloading_package.is_some()
                        || self.installing_package.is_some(),
                },
                cx,
            ))
            .when_some(self.catalog_source_url.as_ref(), |content, source_input| {
                content.child(render_catalog_source_controls(
                    source_input,
                    self.catalog_fetcher.is_some() && self.catalog_store.is_some(),
                    self.catalog_refreshing,
                    self.catalog_refresh_error.as_deref(),
                    cx,
                ))
            })
            .child(render_package_catalog(
                &self.catalog_entries,
                &self.installations,
                self.downloading_package.as_deref(),
                self.installing_package.as_deref(),
                &self.downloaded_packages,
                self.download_error.as_deref(),
                cx,
            ))
            .child(render_catalog(&catalog, cx))
            .child(render_available_entries(&available, cx))
            .child(render_diagnostics(&diagnostics, cx))
    }
}
