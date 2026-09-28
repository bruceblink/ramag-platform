//! 容器管理工具的 Docker 只读工作台。

use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_channel::{TrySendError, bounded};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Input, InputState},
    notification::Notification,
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, ScrollHandle,
    Styled, Subscription, Window, div, prelude::*, px,
};
use ramag_app::{ContainerRegistryService, ContainerService};
use ramag_domain::{
    entities::{
        ContainerEndpointProfile, ContainerImageOperationKind, ContainerImageOperationRequest,
        ContainerListQuery, ContainerLogQuery, ContainerPage, ContainerPlatform,
        ContainerRegistryProfile, ContainerRegistryRepository, ContainerRegistryTag,
        DockerConnectionInfo, DockerContainerDetail, DockerContainerLogLine, DockerContainerLogs,
        DockerContainerStats, DockerContainerSummary, DockerImageDetail, DockerImageSummary,
        DockerNetworkDetail, DockerNetworkSummary, DockerOverview, DockerVolumeDetail,
        DockerVolumeSummary, MAX_CONTAINER_LOG_BYTES, MAX_CONTAINER_LOG_LINES,
        MAX_CONTAINER_QUERY_BYTES,
    },
    error::Result,
    traits::{ContainerLogSink, ContainerLogSinkResult},
};

const RESOURCE_PAGE_SIZE: usize = 100;
const CONTAINER_LOG_CHANNEL_CAPACITY: usize = 128;
const MAX_CONTAINER_STATS_HISTORY: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerSection {
    Overview,
    Containers,
    Images,
    Networks,
    Volumes,
    Logs,
    Registry,
}

impl ContainerSection {
    const ALL: [Self; 7] = [
        Self::Overview,
        Self::Containers,
        Self::Images,
        Self::Networks,
        Self::Volumes,
        Self::Logs,
        Self::Registry,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Containers => "容器",
            Self::Images => "镜像",
            Self::Networks => "网络",
            Self::Volumes => "数据卷",
            Self::Logs => "日志",
            Self::Registry => "镜像仓库",
        }
    }

    const fn icon(self) -> IconName {
        match self {
            Self::Overview | Self::Containers | Self::Volumes => IconName::HardDrive,
            Self::Images => IconName::File,
            Self::Networks => IconName::Network,
            Self::Logs => IconName::File,
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
    docker_endpoint_input: Option<Entity<InputState>>,
    docker_input_subscription: Option<Subscription>,
    docker_endpoint: String,
    resource_search_input: Option<Entity<InputState>>,
    resource_search_subscription: Option<Subscription>,
    resource_search: String,
    logs_search_input: Option<Entity<InputState>>,
    logs_search_subscription: Option<Subscription>,
    logs_search: String,
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
    logs: Option<DockerContainerLogs>,
    logs_loading: bool,
    logs_exporting: bool,
    logs_scroll: ScrollHandle,
    log_cancellation: Option<Arc<AtomicBool>>,
    logs_following: bool,
    log_follow_cancellation: Option<Arc<AtomicBool>>,
    log_follow_paused: bool,
    pending_follow_lines: VecDeque<DockerContainerLogLine>,
    pending_follow_bytes: usize,
    logs_follow_evicted_lines: usize,
    selected_log_container: Option<String>,
    selected_detail: Option<SelectedDetail>,
    container_stats: Option<DockerContainerStats>,
    container_stats_history: VecDeque<DockerContainerStats>,
    stats_loading: bool,
    stats_cancellation: Option<Arc<AtomicBool>>,
    loading: bool,
    detail_loading: bool,
    error: Option<String>,
    request_id: u64,
}

impl ContainerView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self::without_service();
        view.attach_logs_search_input(window, cx);
        view.attach_resource_search_input(window, cx);
        view
    }

    pub fn with_service(
        service: Arc<ContainerService>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::without_service();
        view.service = Some(service);
        view.attach_logs_search_input(_window, cx);
        view.attach_docker_endpoint_input(_window, cx);
        view.attach_resource_search_input(_window, cx);
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
        view.attach_logs_search_input(window, cx);
        view.attach_docker_endpoint_input(window, cx);
        view.attach_resource_search_input(window, cx);
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
        let profile = initial_docker_profile();
        Self {
            service: None,
            registry_service: None,
            docker_endpoint_input: None,
            docker_input_subscription: None,
            docker_endpoint: profile.address.clone(),
            resource_search_input: None,
            resource_search_subscription: None,
            resource_search: String::new(),
            logs_search_input: None,
            logs_search_subscription: None,
            logs_search: String::new(),
            registry_endpoint_input: None,
            registry_input_subscription: None,
            registry_endpoint: "https://registry.example.com".into(),
            registry_allow_insecure_http: false,
            registry_repositories: None,
            registry_tags: None,
            selected_registry_repository: None,
            registry_loading: false,
            registry_error: None,
            profile,
            platform: ContainerPlatform::Docker,
            section: ContainerSection::Overview,
            connection: None,
            overview: None,
            containers: None,
            images: None,
            networks: None,
            volumes: None,
            logs: None,
            logs_loading: false,
            logs_exporting: false,
            logs_scroll: ScrollHandle::new(),
            log_cancellation: None,
            logs_following: false,
            log_follow_cancellation: None,
            log_follow_paused: false,
            pending_follow_lines: VecDeque::new(),
            pending_follow_bytes: 0,
            logs_follow_evicted_lines: 0,
            selected_log_container: None,
            selected_detail: None,
            container_stats: None,
            container_stats_history: VecDeque::new(),
            stats_loading: false,
            stats_cancellation: None,
            loading: false,
            detail_loading: false,
            error: None,
            request_id: 0,
        }
    }

    fn attach_docker_endpoint_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let endpoint = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("unix:///var/run/docker.sock 或 ssh://用户@主机")
                .default_value(self.docker_endpoint.clone())
        });
        let endpoint_for_observer = endpoint.clone();
        self.docker_input_subscription = Some(cx.observe(&endpoint, move |view, _, cx| {
            view.docker_endpoint = endpoint_for_observer.read(cx).value().to_string();
            view.profile.address = view.docker_endpoint.clone();
            view.request_id = view.request_id.wrapping_add(1);
            view.clear_resource_state();
            view.error = None;
            cx.notify();
        }));
        self.docker_endpoint_input = Some(endpoint);
    }

    fn attach_resource_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("筛选名称、镜像、标签或地址")
                .validate(|value, _| value.len() <= MAX_CONTAINER_QUERY_BYTES)
        });
        let search_for_observer = search.clone();
        self.resource_search_subscription = Some(cx.observe(&search, move |view, _, cx| {
            view.resource_search = search_for_observer.read(cx).value().to_string();
            cx.notify();
        }));
        self.resource_search_input = Some(search);
    }

    fn attach_logs_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("筛选当前日志窗口")
                .validate(|value, _| value.len() <= MAX_CONTAINER_QUERY_BYTES)
        });
        let search_for_observer = search.clone();
        self.logs_search_subscription = Some(cx.observe(&search, move |view, _, cx| {
            view.logs_search = search_for_observer.read(cx).value().to_string();
            view.logs_scroll = ScrollHandle::new();
            cx.notify();
        }));
        self.logs_search_input = Some(search);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.section == ContainerSection::Registry {
            self.refresh_registry(cx);
            return;
        }
        if self.section == ContainerSection::Logs {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.sync_docker_endpoint(cx);
        self.sync_resource_search(cx);
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        let profile = self.profile.clone();
        let section = self.section;
        let query = self.resource_query();
        self.loading = true;
        self.error = None;
        self.clear_resource_state();
        if let Err(error) = profile.validate() {
            self.loading = false;
            self.error = Some(error);
            cx.notify();
            return;
        }
        cx.notify();
        cx.spawn(async move |this, async_cx| {
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
                ContainerSection::Logs => unreachable!("日志页面不通过资源刷新读取"),
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
        self.clear_resource_state();
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
        if let Some(cancellation) = self.stats_cancellation.take() {
            cancellation.store(true, Ordering::Relaxed);
        }
        self.stats_loading = false;
        if section == ContainerSection::Containers
            && !matches!(
                &self.selected_detail,
                Some(SelectedDetail::Container(detail)) if detail.summary.id == id
            )
        {
            self.container_stats = None;
            self.container_stats_history.clear();
            if let Some(cancellation) = self.stats_cancellation.take() {
                cancellation.store(true, Ordering::Relaxed);
            }
            self.stats_loading = false;
        }
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
                ContainerSection::Logs => return,
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

    fn load_container_stats(&mut self, container_id: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        if let Some(cancellation) = self.stats_cancellation.take() {
            cancellation.store(true, Ordering::Relaxed);
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        let profile = self.profile.clone();
        self.stats_loading = true;
        self.stats_cancellation = Some(cancellation.clone());
        self.error = None;
        let request_id = self.request_id.wrapping_add(1);
        self.request_id = request_id;
        cx.notify();
        cx.spawn(async move |this, async_cx| {
            let result = service
                .container_stats(&profile, &container_id, cancellation.clone())
                .await;
            let _ = this.update(async_cx, |view, cx| {
                if view.request_id != request_id {
                    return;
                }
                view.apply_container_stats_result(result);
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_container_stats_result(&mut self, result: Result<DockerContainerStats>) {
        self.stats_loading = false;
        self.stats_cancellation = None;
        match result {
            Ok(stats) => {
                self.container_stats = Some(stats.clone());
                self.container_stats_history.push_back(stats);
                while self.container_stats_history.len() > MAX_CONTAINER_STATS_HISTORY {
                    self.container_stats_history.pop_front();
                }
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.user_message());
            }
        }
    }

    fn open_container_logs(
        &mut self,
        container_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let profile = self.profile.clone();
        if let Some(cancellation) = self.log_cancellation.take() {
            cancellation.store(true, Ordering::Relaxed);
        }
        if let Some(cancellation) = self.log_follow_cancellation.take() {
            cancellation.store(true, Ordering::Relaxed);
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        self.section = ContainerSection::Logs;
        self.selected_detail = None;
        self.selected_log_container = Some(container_id.clone());
        self.logs = None;
        self.logs_scroll = ScrollHandle::new();
        self.logs_search.clear();
        if let Some(search) = &self.logs_search_input {
            search.update(cx, |state, cx| state.set_value("", window, cx));
        }
        self.logs_loading = true;
        self.log_cancellation = Some(cancellation.clone());
        self.logs_following = false;
        self.log_follow_paused = false;
        self.pending_follow_lines.clear();
        self.pending_follow_bytes = 0;
        self.logs_follow_evicted_lines = 0;
        self.error = None;
        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        cx.notify();
        cx.spawn(async move |this, async_cx| {
            let query = ContainerLogQuery::default();
            let result = service
                .container_logs_with_cancel(&profile, &container_id, &query, cancellation)
                .await;
            let _ = this.update(async_cx, |view, cx| {
                if view.request_id != request_id {
                    return;
                }
                view.logs_loading = false;
                view.log_cancellation = None;
                match result {
                    Ok(logs) => {
                        view.logs = Some(logs);
                        view.logs_scroll.scroll_to_bottom();
                        view.error = None;
                    }
                    Err(error) => {
                        view.logs = None;
                        view.error = Some(error.user_message());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start_container_log_follow(&mut self, cx: &mut Context<Self>) {
        if self.logs_loading || self.logs_following {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(container_id) = self.selected_log_container.clone() else {
            return;
        };
        let profile = self.profile.clone();
        let query = ContainerLogQuery::follow_new_lines();
        let cancellation = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = bounded(CONTAINER_LOG_CHANNEL_CAPACITY);
        let sink: ContainerLogSink = Arc::new(move |line| match sender.try_send(line) {
            Ok(()) => ContainerLogSinkResult::Accepted,
            Err(TrySendError::Full(_)) => ContainerLogSinkResult::Backpressured,
            Err(TrySendError::Closed(_)) => ContainerLogSinkResult::Closed,
        });

        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        self.logs_following = true;
        self.log_follow_cancellation = Some(cancellation.clone());
        self.log_follow_paused = false;
        self.pending_follow_lines.clear();
        self.pending_follow_bytes = 0;
        self.logs_follow_evicted_lines = 0;
        self.error = None;
        cx.notify();

        let service_for_reader = service.clone();
        let cancellation_for_reader = cancellation.clone();
        cx.spawn(async move |this, async_cx| {
            let result = service_for_reader
                .follow_container_logs(
                    &profile,
                    &container_id,
                    &query,
                    sink,
                    cancellation_for_reader,
                )
                .await;
            let _ = this.update(async_cx, |view, cx| {
                if view.request_id != request_id {
                    return;
                }
                if view.log_follow_paused {
                    view.resume_container_log_follow();
                }
                view.logs_following = false;
                view.log_follow_cancellation = None;
                if let Err(error) = result {
                    view.error = Some(error.user_message());
                }
                cx.notify();
            });
        })
        .detach();

        cx.spawn(async move |this, async_cx| {
            while let Ok(line) = receiver.recv().await {
                let _ = this.update(async_cx, |view, cx| {
                    if view.request_id != request_id {
                        return;
                    }
                    if view.log_follow_paused {
                        view.queue_follow_log_line(line);
                    } else {
                        view.append_follow_log_line(line);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn stop_container_log_follow(&mut self, cx: &mut Context<Self>) {
        let Some(cancellation) = self.log_follow_cancellation.take() else {
            return;
        };
        cancellation.store(true, Ordering::Relaxed);
        self.request_id = self.request_id.wrapping_add(1);
        self.logs_following = false;
        self.log_follow_paused = false;
        self.pending_follow_lines.clear();
        self.pending_follow_bytes = 0;
        self.error = Some("容器日志持续读取已停止；当前窗口保留".into());
        cx.notify();
    }

    fn copy_container_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(logs) = self.logs.as_ref() else {
            return;
        };
        let text = container_logs_copy_text(logs);
        if text.is_empty() {
            return;
        }
        ramag_ui::copy_text_with_notification(text, window, cx);
    }

    fn export_container_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.logs_exporting {
            return;
        }
        let Some(logs) = self.logs.as_ref() else {
            return;
        };
        let text = container_logs_copy_text(logs);
        if text.is_empty() {
            return;
        }
        let logs = logs.clone();
        let container_id = self
            .selected_log_container
            .as_deref()
            .unwrap_or(&logs.container_id)
            .to_owned();
        let file_name = ramag_app::usecases::export::suggested_export_file_name(
            "container",
            &container_id,
            None,
            false,
            "log",
        );
        self.logs_exporting = true;
        cx.notify();
        cx.spawn_in(window, async move |this, async_cx| {
            let outcome: std::result::Result<Option<std::path::PathBuf>, String> = async {
                let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_file_name(&file_name)
                    .add_filter("日志文本", &["log", "txt"])
                    .save_file()
                    .await
                else {
                    return Ok(None);
                };
                let path = handle.path().to_path_buf();
                let write_path = path.clone();
                ramag_app::run_blocking(move || write_container_logs(&write_path, &logs))
                    .await
                    .map_err(|error| format!("写入日志导出失败：{error}"))?;
                Ok(Some(path))
            }
            .await;
            let _ = this.update_in(async_cx, |view, window, cx| {
                view.logs_exporting = false;
                match outcome {
                    Ok(None) => {}
                    Ok(Some(path)) => ramag_ui::push_responsive_notification(
                        window,
                        Notification::success(format!("日志已导出到 {}", path.display()))
                            .autohide(true),
                        cx,
                    ),
                    Err(error) => ramag_ui::push_responsive_notification(
                        window,
                        Notification::error(error).autohide(true),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_export_logs_button(&self, cx: &mut Context<Self>) -> AnyElement {
        ramag_ui::clickable_button("container-logs-export")
            .ghost()
            .small()
            .icon(IconName::File)
            .label(if self.logs_exporting {
                "导出中..."
            } else {
                "导出日志"
            })
            .debug_selector(|| "container-logs-export".into())
            .tooltip(if self.logs_exporting {
                "日志导出进行中"
            } else {
                "导出当前已保留日志"
            })
            .disabled(self.logs_exporting)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.export_container_logs(window, cx);
            }))
            .into_any_element()
    }

    fn render_copy_logs_button(&self, cx: &mut Context<Self>) -> AnyElement {
        ramag_ui::clickable_button("container-logs-copy")
            .ghost()
            .small()
            .icon(IconName::Copy)
            .label("复制日志")
            .debug_selector(|| "container-logs-copy".into())
            .tooltip("复制当前已保留日志")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.copy_container_logs(window, cx);
            }))
            .into_any_element()
    }

    fn toggle_container_log_follow_pause(&mut self, cx: &mut Context<Self>) {
        if !self.logs_following {
            return;
        }
        if self.log_follow_paused {
            self.resume_container_log_follow();
        } else {
            self.log_follow_paused = true;
        }
        self.error = None;
        cx.notify();
    }

    fn resume_container_log_follow(&mut self) {
        self.log_follow_paused = false;
        let pending = std::mem::take(&mut self.pending_follow_lines);
        self.pending_follow_bytes = 0;
        for line in pending {
            self.append_follow_log_line(line);
        }
        self.logs_scroll.scroll_to_bottom();
    }

    fn queue_follow_log_line(&mut self, line: DockerContainerLogLine) {
        let line_bytes = line.message.len();
        if line_bytes > MAX_CONTAINER_LOG_BYTES {
            self.logs_follow_evicted_lines = self.logs_follow_evicted_lines.saturating_add(1);
            return;
        }
        let mut evicted: usize = 0;
        while (!self.pending_follow_lines.is_empty()
            && self.pending_follow_lines.len() >= MAX_CONTAINER_LOG_LINES)
            || (!self.pending_follow_lines.is_empty()
                && self.pending_follow_bytes.saturating_add(line_bytes) > MAX_CONTAINER_LOG_BYTES)
        {
            if let Some(removed) = self.pending_follow_lines.pop_front() {
                self.pending_follow_bytes = self
                    .pending_follow_bytes
                    .saturating_sub(removed.message.len());
                evicted = evicted.saturating_add(1);
            }
        }
        self.pending_follow_lines.push_back(line);
        self.pending_follow_bytes = self.pending_follow_bytes.saturating_add(line_bytes);
        self.logs_follow_evicted_lines = self.logs_follow_evicted_lines.saturating_add(evicted);
    }

    fn append_follow_log_line(&mut self, line: DockerContainerLogLine) {
        let line_bytes = line.message.len();
        if line_bytes > MAX_CONTAINER_LOG_BYTES {
            self.logs_follow_evicted_lines = self.logs_follow_evicted_lines.saturating_add(1);
            return;
        }
        let mut evicted: usize = 0;
        {
            let Some(logs) = self.logs.as_mut() else {
                return;
            };
            while (!logs.lines.is_empty() && logs.lines.len() >= MAX_CONTAINER_LOG_LINES)
                || (!logs.lines.is_empty()
                    && logs.bytes.saturating_add(line_bytes) > MAX_CONTAINER_LOG_BYTES)
            {
                let removed = logs.lines.remove(0);
                logs.bytes = logs.bytes.saturating_sub(removed.message.len());
                evicted = evicted.saturating_add(1);
            }
            if line_bytes <= MAX_CONTAINER_LOG_BYTES {
                logs.lines.push(line);
                logs.bytes = logs.bytes.saturating_add(line_bytes);
            }
        }
        self.logs_follow_evicted_lines = self.logs_follow_evicted_lines.saturating_add(evicted);
        if !self.log_follow_paused {
            self.logs_scroll.scroll_to_bottom();
        }
    }

    fn cancel_container_logs(&mut self, cx: &mut Context<Self>) {
        let Some(cancellation) = self.log_cancellation.take() else {
            return;
        };
        cancellation.store(true, Ordering::Relaxed);
        self.request_id = self.request_id.wrapping_add(1);
        self.logs_loading = false;
        self.error = Some("容器日志读取已取消；迟到结果不会写入当前页面".into());
        cx.notify();
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

    fn sync_docker_endpoint(&mut self, cx: &mut Context<Self>) {
        let endpoint = self
            .docker_endpoint_input
            .as_ref()
            .map(|input| input.read(cx).value().trim().to_owned())
            .unwrap_or_else(|| self.docker_endpoint.trim().to_owned());
        self.docker_endpoint = endpoint.clone();
        self.profile.address = endpoint;
    }

    fn sync_resource_search(&mut self, cx: &mut Context<Self>) {
        if let Some(input) = &self.resource_search_input {
            self.resource_search = input.read(cx).value().to_string();
        }
    }

    fn resource_query(&self) -> ContainerListQuery {
        ContainerListQuery {
            page: 1,
            page_size: RESOURCE_PAGE_SIZE,
            search: (!self.resource_search.trim().is_empty()).then(|| self.resource_search.clone()),
        }
    }

    fn clear_resource_state(&mut self) {
        self.connection = None;
        self.overview = None;
        self.containers = None;
        self.images = None;
        self.networks = None;
        self.volumes = None;
        self.logs = None;
        self.logs_loading = false;
        self.logs_scroll = ScrollHandle::new();
        if let Some(cancellation) = self.log_cancellation.take() {
            cancellation.store(true, Ordering::Relaxed);
        }
        if let Some(cancellation) = self.log_follow_cancellation.take() {
            cancellation.store(true, Ordering::Relaxed);
        }
        self.logs_following = false;
        self.log_follow_paused = false;
        self.pending_follow_lines.clear();
        self.pending_follow_bytes = 0;
        self.logs_follow_evicted_lines = 0;
        self.selected_log_container = None;
        self.selected_detail = None;
        self.container_stats = None;
        self.container_stats_history.clear();
        self.stats_loading = false;
        if let Some(cancellation) = self.stats_cancellation.take() {
            cancellation.store(true, Ordering::Relaxed);
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
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child("容器管理"),
                            )
                            .child(
                                div()
                                    .id("container-subtitle")
                                    .debug_selector(|| "container-subtitle".into())
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
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
        let endpoint = self
            .docker_endpoint_input
            .as_ref()
            .map(|input| {
                Input::new(input)
                    .small()
                    .min_w(px(220.0))
                    .flex_1()
                    .disabled(self.platform != ContainerPlatform::Docker)
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.docker_endpoint.clone())
                    .into_any_element()
            });
        let endpoint_toolbar = ramag_ui::responsive_toolbar()
            .id("container-connection-config")
            .debug_selector(|| "container-connection-config".into())
            .items_center()
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Docker Engine 地址"),
            )
            .child(endpoint)
            .child(
                ramag_ui::clickable_button("container-connect")
                    .ghost()
                    .small()
                    .icon(ramag_ui::icons::refresh_cw())
                    .label("连接")
                    .disabled(
                        self.loading
                            || self.service.is_none()
                            || self.platform == ContainerPlatform::Kubernetes,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx))),
            );
        let header = header.child(endpoint_toolbar);

        let navigation = v_flex()
            .id("container-resource-nav")
            .debug_selector(|| "container-resource-nav".into())
            .bg(theme.sidebar)
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
                                .map(|item| compact_resource_button(item, section, cx))
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

fn initial_docker_profile() -> ContainerEndpointProfile {
    let mut profile = ContainerEndpointProfile::local_docker("本机 Docker");
    if let Ok(endpoint) = std::env::var("DOCKER_HOST")
        && !endpoint.trim().is_empty()
    {
        profile.address = endpoint.trim().to_owned();
    }
    profile
}

impl ContainerView {
    fn render_content(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut content = v_flex()
            .id("container-content")
            .debug_selector(|| "container-content".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scrollbar()
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
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
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
        if let Some(filter) = self.render_resource_filter(theme, cx) {
            content = content.child(filter);
        }
        if self.section == ContainerSection::Logs && self.selected_log_container.is_some() {
            content = content.child(self.render_log_filter(theme, cx));
            let has_copyable_logs = self
                .logs
                .as_ref()
                .is_some_and(|logs| !logs.lines.is_empty());
            let controls = if self.logs_loading {
                Some(
                    ramag_ui::responsive_toolbar()
                        .id("container-logs-controls")
                        .debug_selector(|| "container-logs-controls".into())
                        .child(
                            ramag_ui::clickable_button("container-logs-cancel")
                                .ghost()
                                .small()
                                .icon(IconName::CircleX)
                                .label("停止读取")
                                .debug_selector(|| "container-logs-cancel".into())
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.cancel_container_logs(cx);
                                })),
                        )
                        .into_any_element(),
                )
            } else if self.logs_following {
                Some(
                    ramag_ui::responsive_toolbar()
                        .id("container-logs-controls")
                        .debug_selector(|| "container-logs-controls".into())
                        .when(has_copyable_logs, |toolbar| {
                            toolbar
                                .child(self.render_export_logs_button(cx))
                                .child(self.render_copy_logs_button(cx))
                        })
                        .child(
                            ramag_ui::clickable_button("container-logs-follow-pause")
                                .ghost()
                                .small()
                                .icon(if self.log_follow_paused {
                                    IconName::Play
                                } else {
                                    IconName::Pause
                                })
                                .label(if self.log_follow_paused {
                                    "恢复展示"
                                } else {
                                    "暂停展示"
                                })
                                .debug_selector(|| "container-logs-follow-pause".into())
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.toggle_container_log_follow_pause(cx);
                                })),
                        )
                        .child(
                            ramag_ui::clickable_button("container-logs-follow-stop")
                                .ghost()
                                .small()
                                .icon(IconName::CircleX)
                                .label("停止持续读取")
                                .debug_selector(|| "container-logs-follow-stop".into())
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.stop_container_log_follow(cx);
                                })),
                        )
                        .into_any_element(),
                )
            } else if self.logs.is_some() {
                Some(
                    ramag_ui::responsive_toolbar()
                        .id("container-logs-controls")
                        .debug_selector(|| "container-logs-controls".into())
                        .when(has_copyable_logs, |toolbar| {
                            toolbar
                                .child(self.render_export_logs_button(cx))
                                .child(self.render_copy_logs_button(cx))
                        })
                        .child(
                            ramag_ui::clickable_button("container-logs-follow")
                                .ghost()
                                .small()
                                .icon(IconName::Play)
                                .label("持续读取")
                                .debug_selector(|| "container-logs-follow".into())
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.start_container_log_follow(cx);
                                })),
                        )
                        .into_any_element(),
                )
            } else {
                None
            };
            if let Some(controls) = controls {
                content = content.child(controls);
            }
        }
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
            ContainerSection::Logs => self.render_logs(theme),
            ContainerSection::Registry => self.render_registry(theme, cx),
        };
        content.child(section_content).into_any_element()
    }

    fn render_log_filter(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self
            .logs_search_input
            .as_ref()
            .map(|input| {
                ramag_ui::cleanable_input(input, "container-log-filter-clear", false, cx)
                    .small()
                    .prefix(
                        Icon::new(IconName::Search)
                            .small()
                            .text_color(theme.muted_foreground),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("筛选当前日志窗口")
                    .into_any_element()
            });
        ramag_ui::responsive_toolbar()
            .id("container-log-filter")
            .debug_selector(|| "container-log-filter".into())
            .items_center()
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("日志筛选"),
            )
            .child(
                div()
                    .id("container-log-filter-input")
                    .debug_selector(|| "container-log-filter-input".into())
                    .flex_1()
                    .min_w(px(180.0))
                    .child(input),
            )
            .into_any_element()
    }

    fn render_resource_filter(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !matches!(
            self.section,
            ContainerSection::Containers
                | ContainerSection::Images
                | ContainerSection::Networks
                | ContainerSection::Volumes
        ) {
            return None;
        }
        let disabled =
            self.loading || self.service.is_none() || self.platform != ContainerPlatform::Docker;
        let input = self
            .resource_search_input
            .as_ref()
            .map(|input| {
                div()
                    .id("container-resource-filter-input")
                    .debug_selector(|| "container-resource-filter-input".into())
                    .flex_1()
                    .min_w(px(180.0))
                    .child(Input::new(input).small().disabled(disabled))
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .id("container-resource-filter-input")
                    .debug_selector(|| "container-resource-filter-input".into())
                    .flex_1()
                    .min_w(px(180.0))
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("筛选名称、镜像、标签或地址")
                    .into_any_element()
            });
        Some(
            ramag_ui::responsive_toolbar()
                .id("container-resource-filter")
                .debug_selector(|| "container-resource-filter".into())
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("资源筛选"),
                )
                .child(input)
                .child(
                    ramag_ui::clickable_button("container-resource-filter-apply")
                        .ghost()
                        .small()
                        .icon(ramag_ui::icons::list_filter())
                        .label("筛选")
                        .disabled(disabled)
                        .debug_selector(|| "container-resource-filter-apply".into())
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.refresh(cx);
                        })),
                )
                .into_any_element(),
        )
    }

    fn render_overview(&self, theme: &gpui_kit::component::theme::Theme) -> AnyElement {
        let Some(overview) = &self.overview else {
            return empty_state("连接 Docker Engine 后显示概览", theme).into_any_element();
        };
        let cards = [
            (
                "containers",
                "容器",
                overview.counts.containers.to_string(),
                IconName::HardDrive,
            ),
            (
                "running",
                "运行中",
                overview.counts.running_containers.to_string(),
                IconName::CircleCheck,
            ),
            (
                "images",
                "镜像",
                overview.counts.images.to_string(),
                IconName::File,
            ),
            (
                "networks",
                "网络",
                optional_number(overview.counts.networks),
                IconName::Network,
            ),
            (
                "volumes",
                "数据卷",
                optional_number(overview.counts.volumes),
                IconName::HardDrive,
            ),
            (
                "cpu",
                "CPU 核数",
                optional_number(overview.cpu_count),
                IconName::HardDrive,
            ),
            (
                "memory",
                "内存",
                format_capacity_bytes(overview.memory_bytes),
                IconName::MemoryStick,
            ),
        ]
        .into_iter()
        .map(|(key, label, value, icon)| {
            let selector = format!("container-overview-card-{key}");
            v_flex()
                .id(selector.clone())
                .debug_selector(move || selector.clone())
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
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
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
        theme: &gpui_kit::component::theme::Theme,
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
        self.render_resource_list(rows, "暂无容器", theme, cx)
    }

    fn render_images(
        &self,
        theme: &gpui_kit::component::theme::Theme,
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
        self.render_resource_list(rows, "暂无镜像", theme, cx)
    }

    fn render_registry(
        &self,
        theme: &gpui_kit::component::theme::Theme,
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
        theme: &gpui_kit::component::theme::Theme,
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
        self.render_resource_list(rows, "暂无网络", theme, cx)
    }

    fn render_volumes(
        &self,
        theme: &gpui_kit::component::theme::Theme,
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
        self.render_resource_list(rows, "暂无数据卷", theme, cx)
    }

    fn render_logs(&self, theme: &gpui_kit::component::theme::Theme) -> AnyElement {
        let Some(logs) = &self.logs else {
            return empty_state(
                if self.logs_loading {
                    "正在读取容器日志..."
                } else {
                    "从容器详情打开日志"
                },
                theme,
            )
            .into_any_element();
        };
        let normalized_search = normalize_log_search(&self.logs_search);
        let matched_lines = filtered_log_line_count(logs, &normalized_search);
        let rows = logs
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| log_line_matches(line, &normalized_search))
            .map(|(index, line)| {
                h_flex()
                    .id(format!("container-log-line-{index}"))
                    .debug_selector(move || format!("container-log-line-{index}"))
                    .w_full()
                    .items_start()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(line.stream.label()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .whitespace_normal()
                            .child(line.message.clone()),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let container_id = self
            .selected_log_container
            .as_deref()
            .unwrap_or(&logs.container_id);
        let summary = format!(
            "容器 {container_id} · {matched_lines}/{} 行匹配 · {} bytes{}{}{}{}{}",
            logs.lines.len(),
            logs.bytes,
            if normalized_search.is_empty() {
                String::new()
            } else {
                format!(" · 筛选：{}", self.logs_search.trim())
            },
            if logs.truncated {
                format!(" · 已丢弃 {} 行", logs.dropped_lines)
            } else {
                String::new()
            },
            if self.logs_follow_evicted_lines > 0 {
                format!(" · 持续窗口已移除 {} 行", self.logs_follow_evicted_lines)
            } else {
                String::new()
            },
            if self.logs_following {
                " · 持续读取中"
            } else {
                ""
            },
            if self.log_follow_paused {
                format!(
                    " · 已暂停展示 · 待显示 {} 行",
                    self.pending_follow_lines.len()
                )
            } else {
                String::new()
            },
        );
        let output = if rows.is_empty() {
            div()
                .id("container-log-filter-empty")
                .debug_selector(|| "container-log-filter-empty".into())
                .w_full()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(if normalized_search.is_empty() {
                    "当前没有日志"
                } else {
                    "没有匹配当前筛选条件的日志"
                })
                .into_any_element()
        } else {
            v_flex().gap(px(6.0)).children(rows).into_any_element()
        };
        v_flex()
            .id("container-logs-panel")
            .debug_selector(|| "container-logs-panel".into())
            .w_full()
            .gap(px(12.0))
            .child(info_panel("日志摘要", summary, theme))
            .child(
                v_flex()
                    .id("container-logs-output")
                    .debug_selector(|| "container-logs-output".into())
                    .w_full()
                    .max_h(px(420.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.logs_scroll)
                    .vertical_scrollbar(&self.logs_scroll)
                    .child(output),
            )
            .into_any_element()
    }

    fn render_resource_list(
        &self,
        rows: Vec<AnyElement>,
        empty: &'static str,
        theme: &gpui_kit::component::theme::Theme,
        cx: &mut Context<Self>,
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
        let detail = self.render_detail(theme, cx);
        v_flex()
            .id("container-resource-panel")
            .debug_selector(|| "container-resource-panel".into())
            .w_full()
            .gap(px(12.0))
            .child(body)
            .when_some(detail, |panel, detail| panel.child(detail))
            .into_any_element()
    }

    fn render_detail(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let detail = match &self.selected_detail {
            None => return None,
            Some(SelectedDetail::Container(v)) => container_detail_text(v),
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
        let detail_actions = match &self.selected_detail {
            Some(SelectedDetail::Container(value)) => {
                let container_id_for_logs = value.summary.id.clone();
                let container_id_for_refresh = value.summary.id.clone();
                let container_id_for_stats = value.summary.id.clone();
                Some(
                    ramag_ui::responsive_toolbar()
                        .id("container-detail-actions")
                        .debug_selector(|| "container-detail-actions".into())
                        .child(
                            ramag_ui::clickable_button("container-detail-refresh")
                                .ghost()
                                .small()
                                .icon(ramag_ui::icons::refresh_cw())
                                .label("刷新状态")
                                .debug_selector(|| "container-detail-refresh".into())
                                .tooltip("重新读取容器状态和健康检查")
                                .disabled(self.service.is_none() || self.detail_loading)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.load_detail(container_id_for_refresh.clone(), cx);
                                })),
                        )
                        .child(
                            ramag_ui::clickable_button("container-detail-stats")
                                .ghost()
                                .small()
                                .icon(IconName::MemoryStick)
                                .label(if self.stats_loading {
                                    "读取指标中..."
                                } else {
                                    "刷新指标"
                                })
                                .debug_selector(|| "container-detail-stats".into())
                                .tooltip("读取一次容器 CPU、内存和网络指标")
                                .disabled(
                                    self.service.is_none()
                                        || self.detail_loading
                                        || self.stats_loading,
                                )
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.load_container_stats(container_id_for_stats.clone(), cx);
                                })),
                        )
                        .child(
                            ramag_ui::clickable_button("container-open-logs")
                                .ghost()
                                .small()
                                .icon(ramag_ui::icons::scroll_text())
                                .label("查看日志")
                                .debug_selector(|| "container-open-logs".into())
                                .disabled(self.service.is_none() || self.detail_loading)
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.open_container_logs(
                                        container_id_for_logs.clone(),
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .into_any_element(),
                )
            }
            _ => None,
        };
        let mut panel = v_flex()
            .id("container-detail-panel")
            .debug_selector(|| "container-detail-panel".into())
            .w_full()
            .gap(px(8.0))
            .child(info_panel("详情", detail, theme));
        if let Some(detail_actions) = detail_actions {
            panel = panel.child(detail_actions);
        }
        if let Some(stats) = &self.container_stats {
            panel = panel.child(
                div()
                    .id("container-detail-stats-panel")
                    .debug_selector(|| "container-detail-stats-panel".into())
                    .child(info_panel("资源指标", container_stats_text(stats), theme)),
            );
        }
        if !self.container_stats_history.is_empty() {
            panel = panel.child(
                div()
                    .id("container-detail-stats-history-panel")
                    .debug_selector(|| "container-detail-stats-history-panel".into())
                    .child(info_panel(
                        "最近指标",
                        container_stats_history_text(&self.container_stats_history),
                        theme,
                    )),
            );
        }
        Some(panel.into_any_element())
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
    resource_button_with_width(section, selected, true, cx)
}

fn compact_resource_button(
    section: ContainerSection,
    selected: ContainerSection,
    cx: &mut Context<ContainerView>,
) -> AnyElement {
    resource_button_with_width(section, selected, false, cx)
}

fn resource_button_with_width(
    section: ContainerSection,
    selected: ContainerSection,
    full_width: bool,
    cx: &mut Context<ContainerView>,
) -> AnyElement {
    let id = match section {
        ContainerSection::Overview => "container-resource-overview",
        ContainerSection::Containers => "container-resource-containers",
        ContainerSection::Images => "container-resource-images",
        ContainerSection::Networks => "container-resource-networks",
        ContainerSection::Volumes => "container-resource-volumes",
        ContainerSection::Logs => "container-resource-logs",
        ContainerSection::Registry => "container-resource-registry",
    };
    let mut button = ramag_ui::clickable_button(id)
        .ghost()
        .small()
        .flex_none()
        .justify_start()
        .debug_selector(move || id.into())
        .icon(section.icon())
        .label(section.label())
        .selected(section == selected);
    if full_width {
        button = button.w_full();
    }
    button
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
    let row_id = format!("container-resource-{kind}-{id}");
    let row_selector = row_id.clone();
    let title_selector = format!("{row_id}-title");
    let subtitle_selector = format!("{row_id}-subtitle");
    ramag_ui::clickable_button(row_id)
        .ghost()
        .w_full()
        .justify_start()
        .debug_selector(move || row_selector.clone())
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
                        .gap(px(2.0))
                        .child(
                            div()
                                .debug_selector(move || title_selector.clone())
                                .min_w_0()
                                .text_sm()
                                .truncate()
                                .child(title),
                        )
                        .child(
                            div()
                                .debug_selector(move || subtitle_selector.clone())
                                .min_w_0()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate()
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
    theme: &gpui_kit::component::theme::Theme,
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

fn empty_state(text: &'static str, theme: &gpui_kit::component::theme::Theme) -> impl IntoElement {
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

fn format_capacity_bytes(value: Option<u64>) -> String {
    let Some(value) = value else {
        return "未知".into();
    };
    if value >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", value as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if value >= 1024 * 1024 {
        format!("{:.1} MiB", value as f64 / (1024.0 * 1024.0))
    } else if value >= 1024 {
        format!("{:.1} KiB", value as f64 / 1024.0)
    } else {
        format!("{} B", value)
    }
}

fn format_percent(value: Option<f64>) -> String {
    value.map_or_else(|| "未知".into(), |value| format!("{value:.1}%"))
}

fn container_stats_text(stats: &DockerContainerStats) -> String {
    format!(
        "容器 {}\n采样时间：{}\nCPU 使用率：{}\n内存：{} / {}（{}）\n网络接收：{}\n网络发送：{}",
        stats.container_id,
        stats.read_at.as_deref().unwrap_or("未知"),
        format_percent(stats.cpu_percent),
        format_capacity_bytes(stats.memory_usage_bytes),
        format_capacity_bytes(stats.memory_limit_bytes),
        format_percent(stats.memory_percent),
        format_capacity_bytes(stats.network_rx_bytes),
        format_capacity_bytes(stats.network_tx_bytes)
    )
}

fn container_stats_history_text(history: &VecDeque<DockerContainerStats>) -> String {
    let mut text = format!("最近 {} 次成功刷新", history.len());
    for (index, stats) in history.iter().rev().enumerate() {
        text.push_str(&format!(
            "\n{} · {} · CPU {} · 内存 {} / {} · 网络接收 {} · 网络发送 {}",
            index + 1,
            stats.read_at.as_deref().unwrap_or("未知时间"),
            format_percent(stats.cpu_percent),
            format_capacity_bytes(stats.memory_usage_bytes),
            format_capacity_bytes(stats.memory_limit_bytes),
            format_capacity_bytes(stats.network_rx_bytes),
            format_capacity_bytes(stats.network_tx_bytes)
        ));
    }
    text
}

fn container_detail_text(detail: &DockerContainerDetail) -> String {
    let summary = &detail.summary;
    format!(
        "容器 {}\n状态：{}\n状态说明：{}\n健康检查：{}\n创建时间：{}\n路径：{}\n环境变量：{} 个\n挂载：{} 个\n网络：{} 个",
        summary.id,
        container_state_label(summary.state.as_deref()),
        summary.status.as_deref().unwrap_or("未知"),
        container_health_label(summary.health.as_deref()),
        format_container_created_at(summary.created),
        detail.path.as_deref().unwrap_or("未知"),
        detail.env_keys.len(),
        detail.mounts.len(),
        detail.networks.len()
    )
}

fn container_state_label(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "未知".into();
    };
    match value.to_ascii_lowercase().as_str() {
        "created" => "已创建".into(),
        "running" => "运行中".into(),
        "paused" => "已暂停".into(),
        "restarting" => "重启中".into(),
        "exited" => "已退出".into(),
        "dead" => "已失效".into(),
        _ => value.to_owned(),
    }
}

fn container_health_label(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "未配置健康检查".into();
    };
    match value.to_ascii_lowercase().as_str() {
        "starting" => "启动检查中".into(),
        "healthy" => "健康".into(),
        "unhealthy" => "不健康".into(),
        _ => value.to_owned(),
    }
}

fn format_container_created_at(value: Option<i64>) -> String {
    let Some(value) = value else {
        return "未知".into();
    };
    chrono::DateTime::from_timestamp(value, 0)
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| format!("Unix {value}"))
}

fn normalize_log_search(value: &str) -> String {
    value.trim().to_lowercase()
}

fn log_line_matches(line: &DockerContainerLogLine, normalized_search: &str) -> bool {
    normalized_search.is_empty()
        || line.message.to_lowercase().contains(normalized_search)
        || line.stream.label().contains(normalized_search)
}

fn filtered_log_line_count(logs: &DockerContainerLogs, normalized_search: &str) -> usize {
    logs.lines
        .iter()
        .filter(|line| log_line_matches(line, normalized_search))
        .count()
}

fn container_logs_copy_text(logs: &DockerContainerLogs) -> String {
    let mut text = String::new();
    for line in &logs.lines {
        if text.len() >= MAX_CONTAINER_LOG_BYTES {
            break;
        }
        let line_start = text.len();
        if !text.is_empty() {
            text.push('\n');
        }
        if !append_bounded_text(&mut text, line.stream.label(), MAX_CONTAINER_LOG_BYTES)
            || !append_bounded_text(&mut text, ": ", MAX_CONTAINER_LOG_BYTES)
            || !append_bounded_text(&mut text, &line.message, MAX_CONTAINER_LOG_BYTES)
        {
            text.truncate(line_start);
            break;
        }
    }
    text
}

fn write_container_logs(path: &std::path::Path, logs: &DockerContainerLogs) -> Result<()> {
    ramag_app::usecases::export::write_atomic(path, &container_logs_copy_text(logs))
}

fn append_bounded_text(target: &mut String, value: &str, max_bytes: usize) -> bool {
    for character in value.chars() {
        let next_bytes = target.len().saturating_add(character.len_utf8());
        if next_bytes > max_bytes {
            return false;
        }
        target.push(character);
    }
    true
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

#[cfg(test)]
#[path = "copy_tests.rs"]
mod copy_tests;

#[cfg(test)]
#[path = "export_tests.rs"]
mod export_tests;

#[cfg(test)]
#[path = "detail_tests.rs"]
mod detail_tests;

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod scroll_tests;

#[cfg(test)]
#[path = "filter_tests.rs"]
mod filter_tests;

#[cfg(test)]
#[path = "overview_tests.rs"]
mod overview_tests;

#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
