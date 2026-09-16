//! 容器管理工具的 Docker 只读工作台。

use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use ramag_app::{ContainerRegistryService, ContainerService};
use ramag_domain::{
    entities::{
        ContainerEndpointProfile, ContainerImageOperationKind, ContainerImageOperationRequest,
        ContainerListQuery, ContainerPage, ContainerPlatform, ContainerRegistryProfile,
        ContainerRegistryRepository, ContainerRegistryTag, DockerConnectionInfo,
        DockerContainerDetail, DockerContainerSummary, DockerImageDetail, DockerImageSummary,
        DockerNetworkDetail, DockerNetworkSummary, DockerOverview, DockerVolumeDetail,
        DockerVolumeSummary,
    },
    error::Result,
};

const RESOURCE_PAGE_SIZE: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerSection {
    Overview,
    Containers,
    Images,
    Networks,
    Volumes,
    Registry,
}

impl ContainerSection {
    const ALL: [Self; 6] = [
        Self::Overview,
        Self::Containers,
        Self::Images,
        Self::Networks,
        Self::Volumes,
        Self::Registry,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Containers => "容器",
            Self::Images => "镜像",
            Self::Networks => "网络",
            Self::Volumes => "数据卷",
            Self::Registry => "镜像仓库",
        }
    }

    const fn icon(self) -> IconName {
        match self {
            Self::Overview | Self::Containers | Self::Volumes => IconName::HardDrive,
            Self::Images => IconName::File,
            Self::Networks => IconName::Network,
            Self::Registry => IconName::File,
        }
    }
}

enum LoadResult {
    Overview(Box<Result<DockerOverview>>),
    Containers(Result<ContainerPage<DockerContainerSummary>>),
    Images(Result<ContainerPage<DockerImageSummary>>),
    Networks(Result<ContainerPage<DockerNetworkSummary>>),
    Volumes(Result<ContainerPage<DockerVolumeSummary>>),
    Registry(Result<Vec<ContainerRegistryRepository>>),
}

enum DetailResult {
    Container(Result<DockerContainerDetail>),
    Image(Result<DockerImageDetail>),
    Network(Result<DockerNetworkDetail>),
    Volume(Result<DockerVolumeDetail>),
}

enum SelectedDetail {
    Container(DockerContainerDetail),
    Image(DockerImageDetail),
    Network(DockerNetworkDetail),
    Volume(DockerVolumeDetail),
}

/// Docker 连接和资源查询的主视图；没有服务时只渲染静态空状态，供 headless 布局测试使用。
pub struct ContainerView {
    service: Option<Arc<ContainerService>>,
    registry_service: Option<Arc<ContainerRegistryService>>,
    registry_endpoint_input: Option<Entity<InputState>>,
    registry_input_subscription: Option<Subscription>,
    registry_endpoint: String,
    registry_allow_insecure_http: bool,
    registry_repositories: Option<Vec<ContainerRegistryRepository>>,
    registry_tags: Option<Vec<ContainerRegistryTag>>,
    selected_registry_repository: Option<String>,
    registry_loading: bool,
    registry_error: Option<String>,
    profile: ContainerEndpointProfile,
    platform: ContainerPlatform,
    section: ContainerSection,
    connection: Option<DockerConnectionInfo>,
    overview: Option<DockerOverview>,
    containers: Option<ContainerPage<DockerContainerSummary>>,
    images: Option<ContainerPage<DockerImageSummary>>,
    networks: Option<ContainerPage<DockerNetworkSummary>>,
    volumes: Option<ContainerPage<DockerVolumeSummary>>,
    selected_detail: Option<SelectedDetail>,
    loading: bool,
    detail_loading: bool,
    error: Option<String>,
    request_id: u64,
}

impl ContainerView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self::without_service()
    }

    pub fn with_service(
        service: Arc<ContainerService>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::without_service();
        view.service = Some(service);
        view.refresh(cx);
        view
    }

    pub fn with_services(
        service: Arc<ContainerService>,
        registry_service: Arc<ContainerRegistryService>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::without_service();
        view.service = Some(service);
        view.registry_service = Some(registry_service);
        let endpoint = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("https://registry.example.com")
                .default_value(view.registry_endpoint.clone())
        });
        let endpoint_for_observer = endpoint.clone();
        view.registry_input_subscription = Some(cx.observe(&endpoint, move |view, _, cx| {
            view.registry_endpoint = endpoint_for_observer.read(cx).value().to_string();
            cx.notify();
        }));
        view.registry_endpoint_input = Some(endpoint);
        view.refresh(cx);
        view
    }

    fn without_service() -> Self {
        Self {
            service: None,
            registry_service: None,
            registry_endpoint_input: None,
            registry_input_subscription: None,
            registry_endpoint: "https://registry.example.com".into(),
            registry_allow_insecure_http: false,
            registry_repositories: None,
            registry_tags: None,
            selected_registry_repository: None,
            registry_loading: false,
            registry_error: None,
            profile: ContainerEndpointProfile::local_docker("本机 Docker"),
            platform: ContainerPlatform::Docker,
            section: ContainerSection::Overview,
            connection: None,
            overview: None,
            containers: None,
            images: None,
            networks: None,
            volumes: None,
            selected_detail: None,
            loading: false,
            detail_loading: false,
            error: None,
            request_id: 0,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.section == ContainerSection::Registry {
            self.refresh_registry(cx);
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        let profile = self.profile.clone();
        let section = self.section;
        self.loading = true;
        self.error = None;
        self.selected_detail = None;
        cx.notify();
        cx.spawn(async move |this, async_cx| {
            let query = ContainerListQuery {
                page: 1,
                page_size: RESOURCE_PAGE_SIZE,
                search: None,
            };
            let result = match section {
                ContainerSection::Overview => {
                    LoadResult::Overview(Box::new(service.overview(&profile).await))
                }
                ContainerSection::Containers => {
                    LoadResult::Containers(service.list_containers(&profile, &query).await)
                }
                ContainerSection::Images => {
                    LoadResult::Images(service.list_images(&profile, &query).await)
                }
                ContainerSection::Networks => {
                    LoadResult::Networks(service.list_networks(&profile, &query).await)
                }
                ContainerSection::Volumes => {
                    LoadResult::Volumes(service.list_volumes(&profile, &query).await)
                }
                ContainerSection::Registry => unreachable!("Registry 已在 refresh_registry 处理"),
            };
            let _ = this.update(async_cx, |view, cx| {
                if view.request_id != request_id {
                    return;
                }
                view.apply_load_result(result);
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_load_result(&mut self, result: LoadResult) {
        self.loading = false;
        match result {
            LoadResult::Overview(result) => match *result {
                Ok(value) => {
                    self.connection = Some(value.connection.clone());
                    self.overview = Some(value);
                    self.error = None;
                }
                Err(error) => self.error = Some(error.user_message()),
            },
            LoadResult::Containers(result) => {
                self.store_page(result, |view, page| view.containers = Some(page))
            }
            LoadResult::Images(result) => {
                self.store_page(result, |view, page| view.images = Some(page))
            }
            LoadResult::Networks(result) => {
                self.store_page(result, |view, page| view.networks = Some(page))
            }
            LoadResult::Volumes(result) => {
                self.store_page(result, |view, page| view.volumes = Some(page))
            }
            LoadResult::Registry(result) => match result {
                Ok(repositories) => {
                    self.registry_repositories = Some(repositories);
                    self.registry_error = None;
                }
                Err(error) => self.registry_error = Some(error.user_message()),
            },
        }
    }

    fn refresh_registry(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.registry_service.clone() else {
            return;
        };
        let endpoint = self
            .registry_endpoint_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_else(|| self.registry_endpoint.clone());
        self.registry_endpoint = endpoint.clone();
        let profile = ContainerRegistryProfile::new("当前 Registry", endpoint)
            .with_insecure_http(self.registry_allow_insecure_http);
        if let Err(error) = profile.validate() {
            self.registry_error = Some(error);
            self.registry_repositories = None;
            self.registry_tags = None;
            cx.notify();
            return;
        }
        self.registry_loading = true;
        self.registry_error = None;
        self.registry_tags = None;
        let request_id = self.request_id.wrapping_add(1);
        self.request_id = request_id;
        cx.notify();
        cx.spawn(async move |this, async_cx| {
            let result = service.list_repositories(&profile, None).await;
            let _ = this.update(async_cx, |view, cx| {
                if view.request_id != request_id {
                    return;
                }
                view.registry_loading = false;
                view.apply_load_result(LoadResult::Registry(result));
                cx.notify();
            });
        })
        .detach();
    }

    fn load_registry_tags(&mut self, repository: String, cx: &mut Context<Self>) {
        let Some(service) = self.registry_service.clone() else {
            return;
        };
        let endpoint = self.registry_endpoint.clone();
        let profile = ContainerRegistryProfile::new("当前 Registry", endpoint)
            .with_insecure_http(self.registry_allow_insecure_http);
        self.selected_registry_repository = Some(repository.clone());
        self.registry_loading = true;
        self.registry_error = None;
        let request_id = self.request_id.wrapping_add(1);
        self.request_id = request_id;
        cx.notify();
        cx.spawn(async move |this, async_cx| {
            let result = service.list_tags(&profile, None, &repository).await;
            let _ = this.update(async_cx, |view, cx| {
                if view.request_id != request_id {
                    return;
                }
                view.registry_loading = false;
                match result {
                    Ok(tags) => {
                        view.registry_tags = Some(tags);
                        view.registry_error = None;
                    }
                    Err(error) => view.registry_error = Some(error.user_message()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn store_page<T>(
        &mut self,
        result: Result<ContainerPage<T>>,
        store: impl FnOnce(&mut Self, ContainerPage<T>),
    ) {
        match result {
            Ok(page) => {
                store(self, page);
                self.error = None;
            }
            Err(error) => self.error = Some(error.user_message()),
        }
    }

    fn select_platform(&mut self, platform: ContainerPlatform, cx: &mut Context<Self>) {
        if self.platform == platform {
            return;
        }
        self.platform = platform;
        self.section = ContainerSection::Overview;
        self.connection = None;
        self.overview = None;
        self.selected_detail = None;
        self.error = (platform == ContainerPlatform::Kubernetes)
            .then(|| "Kubernetes 只读查询将在 CMT-007 接入".into());
        cx.notify();
        if platform == ContainerPlatform::Docker {
            self.refresh(cx);
        }
    }

    fn select_section(&mut self, section: ContainerSection, cx: &mut Context<Self>) {
        if self.section == section {
            return;
        }
        self.section = section;
        self.selected_detail = None;
        self.refresh(cx);
    }

    fn load_detail(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let profile = self.profile.clone();
        let section = self.section;
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        self.detail_loading = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, async_cx| {
            let result = match section {
                ContainerSection::Containers => {
                    DetailResult::Container(service.get_container(&profile, &id).await)
                }
                ContainerSection::Images => {
                    DetailResult::Image(service.get_image(&profile, &id).await)
                }
                ContainerSection::Networks => {
                    DetailResult::Network(service.get_network(&profile, &id).await)
                }
                ContainerSection::Volumes => {
                    DetailResult::Volume(service.get_volume(&profile, &id).await)
                }
                ContainerSection::Overview => return,
                ContainerSection::Registry => return,
            };
            let _ = this.update(async_cx, |view, cx| {
                if view.request_id != request_id {
                    return;
                }
                view.detail_loading = false;
                match result {
                    DetailResult::Container(result) => {
                        view.set_detail(result.map(SelectedDetail::Container))
                    }
                    DetailResult::Image(result) => {
                        view.set_detail(result.map(SelectedDetail::Image))
                    }
                    DetailResult::Network(result) => {
                        view.set_detail(result.map(SelectedDetail::Network))
                    }
                    DetailResult::Volume(result) => {
                        view.set_detail(result.map(SelectedDetail::Volume))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn set_detail<T>(&mut self, result: Result<T>)
    where
        T: Into<SelectedDetail>,
    {
        match result {
            Ok(detail) => {
                self.selected_detail = Some(detail.into());
                self.error = None;
            }
            Err(error) => {
                self.selected_detail = None;
                self.error = Some(error.user_message());
            }
        }
    }
}

impl Render for ContainerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = f32::from(window.viewport_size().width) < 720.0;
        let section = self.section;
        let status = self
            .connection
            .as_ref()
            .and_then(|v| v.server_version.clone())
            .unwrap_or_else(|| "未连接".into());
        let status_icon = if self.connection.is_some() {
            IconName::CircleCheck
        } else {
            IconName::CircleX
        };

        let header = v_flex()
            .id("container-header")
            .debug_selector(|| "container-header".into())
            .w_full()
            .flex_none()
            .gap(px(10.0))
            .px(px(20.0))
            .py(px(14.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                ramag_ui::responsive_toolbar()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("容器管理"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Docker Engine 只读查询"),
                            ),
                    )
                    .child(
                        h_flex()
                            .id("container-connection-status")
                            .debug_selector(|| "container-connection-status".into())
                            .flex_none()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(10.0))
                            .py(px(6.0))
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(6.0))
                            .child(Icon::new(status_icon).small())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(status),
                            ),
                    )
                    .child(
                        ramag_ui::clickable_button("container-refresh")
                            .ghost()
                            .small()
                            .icon(ramag_ui::icons::refresh_cw())
                            .label("刷新")
                            .disabled(
                                self.loading || self.platform == ContainerPlatform::Kubernetes,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx))),
                    ),
            )
            .child(
                ramag_ui::responsive_toolbar()
                    .id("container-platform-picker")
                    .debug_selector(|| "container-platform-picker".into())
                    .child(platform_button(
                        ContainerPlatform::Docker,
                        self.platform,
                        cx,
                    ))
                    .child(platform_button(
                        ContainerPlatform::Kubernetes,
                        self.platform,
                        cx,
                    )),
            );

        let navigation = v_flex()
            .id("container-resource-nav")
            .debug_selector(|| "container-resource-nav".into())
            .w(px(176.0))
            .flex_none()
            .gap(px(4.0))
            .p(px(10.0))
            .border_r_1()
            .border_color(theme.border)
            .children(
                ContainerSection::ALL
                    .into_iter()
                    .map(|item| resource_button(item, section, cx))
                    .collect::<Vec<_>>(),
            );
        let content = self.render_content(&theme, cx);
        let body = if compact {
            v_flex()
                .size_full()
                .child(
                    ramag_ui::responsive_toolbar()
                        .id("container-compact-resource-nav")
                        .debug_selector(|| "container-compact-resource-nav".into())
                        .px(px(12.0))
                        .py(px(8.0))
                        .border_b_1()
                        .border_color(theme.border)
                        .children(
                            ContainerSection::ALL
                                .into_iter()
                                .map(|item| resource_button(item, section, cx))
                                .collect::<Vec<_>>(),
                        ),
                )
                .child(content)
        } else {
            h_flex().size_full().child(navigation).child(content)
        };
        v_flex()
            .id("container-view")
            .debug_selector(|| "container-view".into())
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(header)
            .child(body)
    }
}

impl ContainerView {
    fn render_content(
        &self,
        theme: &gpui_component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut content = v_flex()
            .id("container-content")
            .debug_selector(|| "container-content".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(20.0))
            .gap(px(12.0));
        let loading = self.loading || self.registry_loading;
        content = content.child(
            ramag_ui::responsive_toolbar()
                .items_center()
                .child(
                    div().flex_1().min_w_0().child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.section.label()),
                    ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(if loading { "读取中..." } else { "" }),
                ),
        );
        if let Some(error) = &self.error {
            let mut background = theme.danger;
            background.a = 0.12;
            content = content.child(
                h_flex()
                    .id("container-error")
                    .debug_selector(|| "container-error".into())
                    .w_full()
                    .gap(px(8.0))
                    .p(px(10.0))
                    .bg(background)
                    .text_color(theme.danger)
                    .child(Icon::new(IconName::CircleX))
                    .child(div().flex_1().min_w_0().child(error.clone())),
            );
        }
        let section_content = match self.section {
            ContainerSection::Overview => self.render_overview(theme),
            ContainerSection::Containers => self.render_containers(theme, cx),
            ContainerSection::Images => self.render_images(theme, cx),
            ContainerSection::Networks => self.render_networks(theme, cx),
            ContainerSection::Volumes => self.render_volumes(theme, cx),
            ContainerSection::Registry => self.render_registry(theme, cx),
        };
        content.child(section_content).into_any_element()
    }

    fn render_overview(&self, theme: &gpui_component::theme::Theme) -> AnyElement {
        let Some(overview) = &self.overview else {
            return empty_state("连接 Docker Engine 后显示概览", theme).into_any_element();
        };
        let cards = [
            (
                "容器",
                overview.counts.containers.to_string(),
                IconName::HardDrive,
            ),
            (
                "运行中",
                overview.counts.running_containers.to_string(),
                IconName::CircleCheck,
            ),
            ("镜像", overview.counts.images.to_string(), IconName::File),
            (
                "网络",
                optional_number(overview.counts.networks),
                IconName::Network,
            ),
            (
                "数据卷",
                optional_number(overview.counts.volumes),
                IconName::HardDrive,
            ),
        ]
        .into_iter()
        .map(|(label, value, icon)| {
            v_flex()
                .flex_1()
                .min_w(px(108.0))
                .gap(px(6.0))
                .p(px(14.0))
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0))
                .child(Icon::new(icon).small().text_color(theme.muted_foreground))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(value),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();
        v_flex()
            .id("container-overview-panel")
            .debug_selector(|| "container-overview-panel".into())
            .w_full()
            .gap(px(14.0))
            .child(h_flex().w_full().flex_wrap().gap(px(10.0)).children(cards))
            .child(info_panel(
                "Engine",
                format!(
                    "{} · API {} · {}",
                    overview
                        .version
                        .server_version
                        .as_deref()
                        .unwrap_or("未知版本"),
                    overview.version.api_version.as_deref().unwrap_or("未知"),
                    overview.version.os.as_deref().unwrap_or("未知系统")
                ),
                theme,
            ))
            .into_any_element()
    }

    fn render_containers(
        &self,
        theme: &gpui_component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self
            .containers
            .as_ref()
            .map(|page| {
                page.items
                    .iter()
                    .map(|item| {
                        resource_row(
                            "container",
                            item.id.clone(),
                            item.names
                                .first()
                                .cloned()
                                .unwrap_or_else(|| item.id.clone()),
                            format!(
                                "{} · {}",
                                item.image.as_deref().unwrap_or("未知镜像"),
                                item.status
                                    .as_deref()
                                    .or(item.state.as_deref())
                                    .unwrap_or("未知状态")
                            ),
                            self,
                            cx,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.render_resource_list(rows, "暂无容器", theme)
    }

    fn render_images(
        &self,
        theme: &gpui_component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self
            .images
            .as_ref()
            .map(|page| {
                page.items
                    .iter()
                    .map(|item| {
                        resource_row(
                            "image",
                            item.id.clone(),
                            item.repository_tags
                                .first()
                                .cloned()
                                .unwrap_or_else(|| item.id.clone()),
                            format!(
                                "{} · {}",
                                format_bytes(item.size_bytes),
                                item.operating_system.as_deref().unwrap_or("未知系统")
                            ),
                            self,
                            cx,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.render_resource_list(rows, "暂无镜像", theme)
    }

    fn render_registry(
        &self,
        theme: &gpui_component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let endpoint = self
            .registry_endpoint_input
            .as_ref()
            .map(|input| {
                Input::new(input)
                    .small()
                    .min_w(px(180.0))
                    .flex_1()
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.registry_endpoint.clone())
                    .into_any_element()
            });
        let mut content = v_flex()
            .id("container-registry-panel")
            .debug_selector(|| "container-registry-panel".into())
            .w_full()
            .gap(px(12.0))
            .child(
                ramag_ui::responsive_toolbar()
                    .id("container-registry-config")
                    .debug_selector(|| "container-registry-config".into())
                    .items_center()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(180.0))
                            .min_w_0()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Registry 端点"),
                            )
                            .child(endpoint),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("允许 HTTP"),
                            )
                            .child(
                                ramag_ui::clickable_switch("container-registry-insecure-http")
                                    .checked(self.registry_allow_insecure_http)
                                    .on_click(cx.listener(|this, _: &bool, _, cx| {
                                        this.registry_allow_insecure_http =
                                            !this.registry_allow_insecure_http;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        ramag_ui::clickable_button("container-registry-refresh")
                            .ghost()
                            .small()
                            .icon(ramag_ui::icons::refresh_cw())
                            .label("查询")
                            .disabled(self.registry_loading || self.registry_service.is_none())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_registry(cx);
                            })),
                    ),
            );
        if let Some(error) = &self.registry_error {
            let mut background = theme.danger;
            background.a = 0.12;
            content = content.child(
                h_flex()
                    .id("container-registry-error")
                    .debug_selector(|| "container-registry-error".into())
                    .w_full()
                    .gap(px(8.0))
                    .p(px(10.0))
                    .bg(background)
                    .text_color(theme.danger)
                    .child(Icon::new(IconName::CircleX))
                    .child(div().flex_1().min_w_0().child(error.clone())),
            );
        }

        let repository_rows = self
            .registry_repositories
            .as_ref()
            .map(|repositories| {
                repositories
                    .iter()
                    .map(|repository| registry_repository_row(repository, self, cx))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let repositories = if repository_rows.is_empty() {
            empty_state(
                if self.registry_loading {
                    "正在读取镜像仓库..."
                } else if self.registry_service.is_none() {
                    "镜像仓库模块不可用"
                } else {
                    "输入 Registry 端点后查询"
                },
                theme,
            )
            .into_any_element()
        } else {
            v_flex()
                .w_full()
                .gap(px(6.0))
                .children(repository_rows)
                .into_any_element()
        };
        content = content.child(info_panel(
            "仓库",
            self.registry_repositories.as_ref().map_or_else(
                || "尚未查询".into(),
                |repositories| format!("{} 个仓库", repositories.len()),
            ),
            theme,
        ));
        content = content.child(repositories);
        if let Some(repository) = &self.selected_registry_repository {
            let tags = self.registry_tags.as_ref().map_or_else(
                || "尚未读取 Tag".into(),
                |tags| {
                    if tags.is_empty() {
                        "没有 Tag".into()
                    } else {
                        tags.iter()
                            .map(|tag| tag.name.as_str())
                            .collect::<Vec<_>>()
                            .join("、")
                    }
                },
            );
            content = content.child(info_panel("Tag", format!("{repository}：{tags}"), theme));
        }
        content.into_any_element()
    }

    fn render_networks(
        &self,
        theme: &gpui_component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self
            .networks
            .as_ref()
            .map(|page| {
                page.items
                    .iter()
                    .map(|item| {
                        resource_row(
                            "network",
                            item.id.clone(),
                            item.name.clone().unwrap_or_else(|| item.id.clone()),
                            format!(
                                "{} · {} 个容器",
                                item.driver.as_deref().unwrap_or("未知驱动"),
                                item.container_count
                            ),
                            self,
                            cx,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.render_resource_list(rows, "暂无网络", theme)
    }

    fn render_volumes(
        &self,
        theme: &gpui_component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self
            .volumes
            .as_ref()
            .map(|page| {
                page.items
                    .iter()
                    .map(|item| {
                        resource_row(
                            "volume",
                            item.name.clone(),
                            item.name.clone(),
                            format!(
                                "{} · {}",
                                item.driver.as_deref().unwrap_or("未知驱动"),
                                item.mountpoint.as_deref().unwrap_or("未知挂载点")
                            ),
                            self,
                            cx,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.render_resource_list(rows, "暂无数据卷", theme)
    }

    fn render_resource_list(
        &self,
        rows: Vec<AnyElement>,
        empty: &'static str,
        theme: &gpui_component::theme::Theme,
    ) -> AnyElement {
        let body = if rows.is_empty() {
            empty_state(
                if self.loading {
                    "正在读取资源..."
                } else {
                    empty
                },
                theme,
            )
            .into_any_element()
        } else {
            v_flex()
                .w_full()
                .gap(px(6.0))
                .children(rows)
                .into_any_element()
        };
        let detail = self.render_detail(theme);
        v_flex()
            .id("container-resource-panel")
            .debug_selector(|| "container-resource-panel".into())
            .w_full()
            .gap(px(12.0))
            .child(body)
            .when_some(detail, |panel, detail| panel.child(detail))
            .into_any_element()
    }

    fn render_detail(&self, theme: &gpui_component::theme::Theme) -> Option<AnyElement> {
        let detail = match &self.selected_detail {
            None => return None,
            Some(SelectedDetail::Container(v)) => format!(
                "容器 {}\n路径：{}\n环境变量：{} 个\n挂载：{} 个\n网络：{} 个",
                v.summary.id,
                v.path.as_deref().unwrap_or("未知"),
                v.env_keys.len(),
                v.mounts.len(),
                v.networks.len()
            ),
            Some(SelectedDetail::Image(v)) => {
                let operation_preview = self
                    .service
                    .as_ref()
                    .and_then(|service| {
                        let request = ContainerImageOperationRequest::new(
                            ContainerImageOperationKind::Delete,
                            v.summary.id.clone(),
                        );
                        service
                            .preview_image_operation(&self.profile, &request)
                            .ok()
                    })
                    .map_or_else(
                        || "不可用".into(),
                        |preview| {
                            preview
                                .blocked_reason
                                .unwrap_or_else(|| "可执行，但仍需二次确认".into())
                        },
                    );
                format!(
                    "镜像 {}\n作者：{}\nDocker 版本：{}\n删除预览：{}",
                    v.summary.id,
                    v.author.as_deref().unwrap_or("未知"),
                    v.docker_version.as_deref().unwrap_or("未知"),
                    operation_preview
                )
            }
            Some(SelectedDetail::Network(v)) => format!(
                "网络 {}\n驱动：{}\n子网：{} 个\n关联容器：{} 个",
                v.summary.name.as_deref().unwrap_or(&v.summary.id),
                v.summary.driver.as_deref().unwrap_or("未知"),
                v.summary.subnets.len(),
                v.containers.len()
            ),
            Some(SelectedDetail::Volume(v)) => format!(
                "数据卷 {}\n挂载点：{}\n引用：{}",
                v.summary.name,
                v.summary.mountpoint.as_deref().unwrap_or("未知"),
                v.summary
                    .usage_reference_count
                    .map_or_else(|| "未知".into(), |count| count.to_string())
            ),
        };
        Some(info_panel("详情", detail, theme).into_any_element())
    }
}

fn platform_button(
    platform: ContainerPlatform,
    selected: ContainerPlatform,
    cx: &mut Context<ContainerView>,
) -> AnyElement {
    let id = match platform {
        ContainerPlatform::Docker => "container-platform-docker",
        ContainerPlatform::Kubernetes => "container-platform-kubernetes",
    };
    ramag_ui::clickable_button(id)
        .ghost()
        .small()
        .icon(if platform == ContainerPlatform::Docker {
            IconName::HardDrive
        } else {
            IconName::Network
        })
        .label(platform.label())
        .selected(platform == selected)
        .on_click(
            cx.listener(move |this, _: &ClickEvent, _, cx| this.select_platform(platform, cx)),
        )
        .into_any_element()
}

fn resource_button(
    section: ContainerSection,
    selected: ContainerSection,
    cx: &mut Context<ContainerView>,
) -> AnyElement {
    let id = match section {
        ContainerSection::Overview => "container-resource-overview",
        ContainerSection::Containers => "container-resource-containers",
        ContainerSection::Images => "container-resource-images",
        ContainerSection::Networks => "container-resource-networks",
        ContainerSection::Volumes => "container-resource-volumes",
        ContainerSection::Registry => "container-resource-registry",
    };
    ramag_ui::clickable_button(id)
        .ghost()
        .small()
        .w_full()
        .justify_start()
        .debug_selector(move || id.into())
        .icon(section.icon())
        .label(section.label())
        .selected(section == selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select_section(section, cx)))
        .into_any_element()
}

fn resource_row(
    kind: &'static str,
    id: String,
    title: String,
    subtitle: String,
    view: &ContainerView,
    cx: &mut Context<ContainerView>,
) -> AnyElement {
    let theme = cx.theme();
    let detail_id = id.clone();
    ramag_ui::clickable_button(format!("container-resource-{kind}-{id}"))
        .ghost()
        .w_full()
        .justify_start()
        .on_click(
            cx.listener(move |this, _: &ClickEvent, _, cx| this.load_detail(detail_id.clone(), cx)),
        )
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap(px(10.0))
                .child(
                    Icon::new(IconName::HardDrive)
                        .small()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().child(title))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(subtitle),
                        ),
                )
                .when(view.detail_loading, |row| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("读取详情..."),
                    )
                }),
        )
        .into_any_element()
}

fn registry_repository_row(
    repository: &ContainerRegistryRepository,
    view: &ContainerView,
    cx: &mut Context<ContainerView>,
) -> AnyElement {
    let theme = cx.theme();
    let name = repository.name.clone();
    let selected = view.selected_registry_repository.as_deref() == Some(name.as_str());
    ramag_ui::clickable_button(format!("container-registry-repository-{name}"))
        .ghost()
        .w_full()
        .selected(selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.load_registry_tags(name.clone(), cx);
        }))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap(px(10.0))
                .child(
                    Icon::new(IconName::File)
                        .small()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .child(repository.name.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("读取 Tag"),
                ),
        )
        .into_any_element()
}

fn info_panel(
    title: &'static str,
    text: String,
    theme: &gpui_component::theme::Theme,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap(px(8.0))
        .p(px(14.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(6.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(title),
        )
        .child(div().text_sm().child(text))
}

fn empty_state(text: &'static str, theme: &gpui_component::theme::Theme) -> impl IntoElement {
    v_flex()
        .id("container-empty-state")
        .debug_selector(|| "container-empty-state".into())
        .w_full()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .p(px(28.0))
        .text_center()
        .child(
            Icon::new(IconName::HardDrive)
                .large()
                .text_color(theme.muted_foreground),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text),
        )
}

fn optional_number(value: Option<usize>) -> String {
    value.map_or_else(|| "未知".into(), |value| value.to_string())
}

fn format_bytes(value: Option<i64>) -> String {
    let Some(value) = value else {
        return "大小未知".into();
    };
    if value >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", value as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if value >= 1024 * 1024 {
        format!("{:.1} MiB", value as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} B", value.max(0))
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
