mod clipboard;
mod database;
mod monitor_presets;
mod pages;
mod ssh;
mod system;
mod update;

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::{
    h_flex,
    input::{InputEvent, InputState},
    notification::Notification,
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    Styled, Subscription, Task, Window, div, prelude::FluentBuilder as _,
};
use ramag_app::{
    AvailableUpdate, ClipboardService, ConnectionService, SshService, StaticPluginHost,
    UpdateService,
};
use ramag_domain::entities::{
    ClipboardSettings, IdConverterKind, MAX_CUSTOM_ID_ALPHABET_BYTES,
    MAX_ID_CONVERTER_PROGRAM_BYTES, SshModuleSettings,
};
use tracing::error;

use crate::MAX_SEARCH_INPUT_BYTES;
use crate::plugin_diagnostics::PluginDiagnosticsView;

const SETTINGS_COMPACT_BREAKPOINT: f32 = 900.0;
const SETTINGS_COMPACT_NAV_ITEM_WIDTH: f32 = 144.0;

fn settings_is_compact(window: &Window) -> bool {
    f32::from(window.viewport_size().width) < SETTINGS_COMPACT_BREAKPOINT
}

fn render_settings_layout<N, C>(compact: bool, navigation: N, content: C) -> impl IntoElement
where
    N: IntoElement,
    C: IntoElement,
{
    h_flex()
        .when(compact, |root| root.flex_col())
        .id("settings-root")
        .debug_selector(|| "settings-root".into())
        .size_full()
        .min_w_0()
        .min_h_0()
        // 页面内容高度会随设置项变化，导航列始终沿窗口高度拉伸并从顶部对齐。
        .items_stretch()
        .child(navigation)
        .child(
            div()
                .id("settings-content")
                .debug_selector(|| "settings-content".into())
                .flex_1()
                .when(compact, |content| content.w_full().min_h_0())
                .when(!compact, |content| content.h_full())
                .min_w_0()
                .child(content),
        )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SettingsPage {
    #[default]
    System,
    Database,
    VersionControl,
    Ssh,
    ObjectStorage,
    Clipboard,
    Plugins,
    Update,
}

impl SettingsPage {
    const ALL: [Self; 8] = [
        Self::System,
        Self::Database,
        Self::VersionControl,
        Self::Ssh,
        Self::ObjectStorage,
        Self::Clipboard,
        Self::Plugins,
        Self::Update,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Database => "database",
            Self::VersionControl => "version-control",
            Self::Ssh => "ssh",
            Self::ObjectStorage => "object-storage",
            Self::Plugins => "plugins",
            Self::Update => "update",
            Self::Clipboard => "clipboard",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::System => "系统设置",
            Self::Database => "数据库客户端",
            Self::VersionControl => "版本管理",
            Self::Ssh => "SSH 管理",
            Self::ObjectStorage => "云存储",
            Self::Plugins => "插件",
            Self::Update => "关于",
            Self::Clipboard => "剪贴板",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::System => "外观、滚动与窗口",
            Self::Database => "连接与搜索",
            Self::VersionControl => "Git 行为",
            Self::Ssh => "SSH 与 SFTP",
            Self::ObjectStorage => "账号与访问模式",
            Self::Plugins => "状态、入口和错误",
            Self::Update => "版本与更新",
            Self::Clipboard => "采集、热键与历史",
        }
    }

    fn clears_database_test_when_switching_to(self, next: Self) -> bool {
        self == Self::Database && next != self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum UpdateUiState {
    #[default]
    Idle,
    UpToDate,
    Available(AvailableUpdate),
    UnsupportedPlatform(AvailableUpdate),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum DatabaseConverterTestState {
    #[default]
    Idle,
    Testing(DatabaseConverterTestDirection),
    Success {
        direction: DatabaseConverterTestDirection,
        output: String,
    },
    Error {
        direction: DatabaseConverterTestDirection,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseConverterTestDirection {
    ToInteger,
    ToString,
}

pub struct SettingsView {
    selected_page: SettingsPage,
    navigation_scroll: gpui_kit::ScrollHandle,
    page_scroll: gpui_kit::ScrollHandle,
    clipboard_service: Option<Arc<ClipboardService>>,
    connection_service: Arc<ConnectionService>,
    ssh_service: Arc<SshService>,
    update_service: Option<Arc<UpdateService>>,
    update_state: UpdateUiState,
    clipboard: ClipboardSettings,
    loaded_revision: u64,
    saving_clipboard: bool,
    clearing_clipboard_history: bool,
    ssh_module_settings: SshModuleSettings,
    saving_ssh_module_settings: bool,
    database_enabled_draft: bool,
    redis_sink_same_name_keys: bool,
    show_database_result_horizontal_scrollbar: bool,
    display_database_binary_16_as_uuid: bool,
    database_converter_kind: IdConverterKind,
    database_custom_alphabet: Entity<InputState>,
    database_converter_program: Entity<InputState>,
    database_converter_test_input: Entity<InputState>,
    database_converter_test_state: DatabaseConverterTestState,
    database_converter_test_seq: u64,
    database_converter_test_task: Option<Task<()>>,
    saving_database: bool,
    database_save_pending: bool,
    database_save_debounce: Option<Task<()>>,
    picking_id_converter: bool,
    database_transferring: bool,
    pending_notification: Option<Notification>,
    plugin_diagnostics: Entity<PluginDiagnosticsView>,
    monitor_preset_manager: Entity<monitor_presets::MonitorPresetManager>,
    _update_indicator_subscription: Subscription,
}

impl SettingsView {
    pub fn new(
        plugin_host: Arc<StaticPluginHost>,
        clipboard_service: Option<Arc<ClipboardService>>,
        connection_service: Arc<ConnectionService>,
        ssh_service: Arc<SshService>,
        update_service: Option<Arc<UpdateService>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let update_indicator_subscription =
            cx.observe_global::<crate::activity_bar::UpdateIndicatorGlobal>(|_, cx| cx.notify());
        let plugin_diagnostics = cx.new(|_| PluginDiagnosticsView::new(plugin_host));
        let monitor_preset_manager =
            cx.new(|cx| monitor_presets::MonitorPresetManager::new(window, cx));
        let (clipboard, loaded_revision) = clipboard_service
            .as_ref()
            .map(|service| service.settings_snapshot_with_revision())
            .unwrap_or_default();
        let ssh_module_settings = ssh_service.module_settings_snapshot();
        let database = crate::database_search_settings(cx);
        let database_enabled_draft = database.id_conversion_enabled;
        let redis_sink_same_name_keys = crate::redis_tree_settings(cx).sink_same_name_keys;
        let database_result_settings = crate::database_result_settings(cx);
        let show_database_result_horizontal_scrollbar =
            database_result_settings.show_horizontal_scrollbar;
        let display_database_binary_16_as_uuid = database_result_settings.display_binary_16_as_uuid;
        let database_converter_kind = database.converter.kind;
        let custom_alphabet = database.converter.custom_alphabet.clone();
        let converter_program = database.converter.external_program.clone();
        let database_custom_alphabet = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|value, _| value.len() <= MAX_CUSTOM_ID_ALPHABET_BYTES)
                .placeholder("按数值从小到大输入字符，例如 0123456789abcdef")
                .default_value(custom_alphabet)
        });
        let database_converter_program = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|value, _| value.len() <= MAX_ID_CONVERTER_PROGRAM_BYTES)
                .placeholder("请选择外部程序的绝对路径")
                .default_value(converter_program)
        });
        let database_converter_test_input = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|value, _| value.len() <= MAX_SEARCH_INPUT_BYTES)
                .placeholder("输入字符串，或非负十进制整数")
        });
        cx.subscribe_in(
            &database_custom_alphabet,
            window,
            |this, _, event: &InputEvent, _, cx| {
                this.on_database_converter_input_event(event, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            &database_converter_program,
            window,
            |this, _, event: &InputEvent, _, cx| {
                this.on_database_converter_input_event(event, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            &database_converter_test_input,
            window,
            |this, _, event: &InputEvent, _, cx| {
                this.on_database_converter_test_input_event(event, cx);
            },
        )
        .detach();

        if let Some(service) = clipboard_service.clone() {
            cx.spawn(async move |this, cx| {
                service.load_settings().await;
                let (settings, revision) = service.settings_snapshot_with_revision();
                let _ = this.update(cx, |this, cx| {
                    if !this.saving_clipboard {
                        this.clipboard = settings;
                        this.loaded_revision = revision;
                        cx.notify();
                    }
                });
            })
            .detach();
        }

        let ssh_settings_service = ssh_service.clone();
        cx.spawn(async move |this, cx| {
            let result = ssh_settings_service.load_module_settings().await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(settings) if !this.saving_ssh_module_settings => {
                    this.ssh_module_settings = settings;
                    cx.notify();
                }
                Ok(_) => {}
                Err(error) => {
                    error!(
                        operation = "ssh_module_settings_load",
                        error = %error,
                        "load SSH module settings failed"
                    );
                    this.pending_notification = Some(Notification::error(format!(
                        "SSH 模块设置读取失败：{error}"
                    )));
                    cx.notify();
                }
            });
        })
        .detach();

        if clipboard_service.is_some() {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(600))
                        .await;
                    let alive = this
                        .update(cx, |this, cx| {
                            let Some(service) = this.clipboard_service.as_ref() else {
                                return;
                            };
                            let revision = service.settings_revision();
                            if !this.saving_clipboard && revision != this.loaded_revision {
                                let (settings, revision) =
                                    service.settings_snapshot_with_revision();
                                this.clipboard = settings;
                                this.loaded_revision = revision;
                                cx.notify();
                            }
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            })
            .detach();
        }

        Self {
            selected_page: SettingsPage::default(),
            navigation_scroll: gpui_kit::ScrollHandle::new(),
            page_scroll: gpui_kit::ScrollHandle::new(),
            clipboard_service,
            connection_service,
            ssh_service,
            update_service,
            update_state: UpdateUiState::Idle,
            clipboard,
            loaded_revision,
            saving_clipboard: false,
            clearing_clipboard_history: false,
            ssh_module_settings,
            saving_ssh_module_settings: false,
            database_enabled_draft,
            redis_sink_same_name_keys,
            show_database_result_horizontal_scrollbar,
            display_database_binary_16_as_uuid,
            database_converter_kind,
            database_custom_alphabet,
            database_converter_program,
            database_converter_test_input,
            database_converter_test_state: DatabaseConverterTestState::Idle,
            database_converter_test_seq: 0,
            database_converter_test_task: None,
            saving_database: false,
            database_save_pending: false,
            database_save_debounce: None,
            picking_id_converter: false,
            database_transferring: false,
            pending_notification: None,
            plugin_diagnostics,
            monitor_preset_manager,
            _update_indicator_subscription: update_indicator_subscription,
        }
    }

    fn save_clipboard(&mut self, next: ClipboardSettings, cx: &mut Context<Self>) {
        if self.saving_clipboard || self.clipboard == next {
            return;
        }
        let Some(service) = self.clipboard_service.clone() else {
            return;
        };
        let previous = std::mem::replace(&mut self.clipboard, next.clone());
        self.saving_clipboard = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = service.save_settings(&next).await;
            let _ = this.update(cx, |this, cx| {
                this.saving_clipboard = false;
                match result {
                    Ok(()) => {
                        let (settings, revision) = service.settings_snapshot_with_revision();
                        this.loaded_revision = revision;
                        this.clipboard = settings;
                    }
                    Err(error) => {
                        error!(
                            operation = "clipboard_settings_save",
                            error = %error,
                            "save clipboard settings failed"
                        );
                        this.clipboard = previous;
                        this.pending_notification = Some(Notification::error(format!(
                            "剪贴板设置保存失败（已还原）：{error}"
                        )));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_update_state();
        if let Some(notification) = self.pending_notification.take() {
            crate::push_responsive_notification(window, notification, cx);
        }
        let compact = settings_is_compact(window);
        let navigation = self.render_navigation(window, cx).into_any_element();
        let content = self.render_selected_page(window, cx);
        let keys: &[(&str, &str)] = match self.selected_page {
            SettingsPage::System => &[
                ("theme_mode", "主题"),
                (crate::SYSTEM_SETTINGS_PREF_KEY, "系统设置"),
                (crate::MONITOR_SETTINGS_PREF_KEY, "刷新频率"),
            ],
            _ => &[],
        };
        let content = v_flex()
            .size_full()
            .min_h_0()
            .when_some(
                crate::preferences::status::preference_status(keys, cx),
                |content, status| {
                    content.child(
                        div()
                            .debug_selector(|| "settings-save-area".into())
                            .w_full()
                            .h_auto()
                            .flex_none()
                            .px_3()
                            .py_2()
                            .max_h(gpui_kit::px(160.0))
                            .overflow_y_scrollbar()
                            .child(status),
                    )
                },
            )
            .child(div().flex_1().min_h_0().child(content));
        render_settings_layout(compact, navigation, content)
    }
}

impl SettingsView {
    /// 按工具 ID 选择已有设置页；切换时清理数据库临时测试，不复制连接配置。
    pub(crate) fn open_tool_page(
        &mut self,
        tool_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = match tool_id {
            "system" => SettingsPage::System,
            "dbclient" | "redis" => SettingsPage::Database,
            "ssh" => SettingsPage::Ssh,
            "clipboard" => SettingsPage::Clipboard,
            "vcs" => SettingsPage::VersionControl,
            "object_storage" | "object-storage" => SettingsPage::ObjectStorage,
            _ => SettingsPage::System,
        };
        if self
            .selected_page
            .clears_database_test_when_switching_to(next)
        {
            self.clear_database_converter_test(window, cx);
        }
        self.selected_page = next;
        self.page_scroll
            .set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0)));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::SettingsPage;

    #[test]
    fn system_is_the_default_page() {
        assert_eq!(SettingsPage::default(), SettingsPage::System);
        assert_eq!(SettingsPage::ALL.first(), Some(&SettingsPage::System));
    }

    #[test]
    fn each_large_module_has_a_distinct_page() {
        let ids: HashSet<_> = SettingsPage::ALL
            .into_iter()
            .map(SettingsPage::id)
            .collect();

        assert_eq!(ids.len(), SettingsPage::ALL.len());
        assert!(ids.contains("system"));
        assert!(ids.contains("database"));
        assert!(ids.contains("version-control"));
        assert!(ids.contains("clipboard"));
        assert!(ids.contains("ssh"));
        assert!(ids.contains("object-storage"));
        assert!(ids.contains("plugins"));
        assert!(ids.contains("update"));
    }

    #[test]
    fn update_page_is_always_last() {
        assert_eq!(SettingsPage::ALL.last(), Some(&SettingsPage::Update));
        assert_eq!(SettingsPage::Update.title(), "关于");
    }

    #[test]
    fn leaving_database_page_clears_temporary_converter_test() {
        assert!(SettingsPage::Database.clears_database_test_when_switching_to(SettingsPage::Ssh));
        assert!(
            !SettingsPage::Database.clears_database_test_when_switching_to(SettingsPage::Database)
        );
        assert!(!SettingsPage::Clipboard.clears_database_test_when_switching_to(SettingsPage::Ssh));
    }
}
