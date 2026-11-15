//! 用有界通道控制驱动返回顺序，测试仍经过真实服务和 GPUI 回调。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, px, size,
};
use ramag_app::{ContainerRegistryService, ContainerService};
use ramag_domain::entities::{
    ContainerEndpointProfile, ContainerListQuery, ContainerLogQuery, ContainerPage,
    ContainerRegistryCredential, ContainerRegistryProfile, ContainerRegistryRepository,
    ContainerRegistryTag, DockerConnectionInfo, DockerContainerDetail, DockerContainerLogs,
    DockerContainerStats, DockerContainerSummary, DockerEngineVersion, DockerOverview,
    DockerResourceCounts,
};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::{
    ContainerDriver, ContainerOperationCancellation, ContainerRegistryDriver,
};

use super::super::ContainerView;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Operation {
    Overview,
    List,
    Detail,
    Logs,
    Stats,
    Repositories,
    Tags,
}

type PendingResult = (async_channel::Receiver<()>, bool);

#[derive(Default)]
pub(super) struct ControlledDriver {
    pub calls: Mutex<Vec<Operation>>,
    pending: Mutex<HashMap<Operation, PendingResult>>,
}

impl ControlledDriver {
    pub fn delay(&self, operation: Operation, fail: bool) -> async_channel::Sender<()> {
        let (sender, receiver) = async_channel::bounded(1);
        self.pending
            .lock()
            .expect("受控请求锁")
            .insert(operation, (receiver, fail));
        sender
    }

    async fn complete(&self, operation: Operation) -> Result<()> {
        self.calls.lock().expect("驱动调用锁").push(operation);
        let pending = self.pending.lock().expect("受控请求锁").remove(&operation);
        if let Some((receiver, fail)) = pending {
            receiver.recv().await.expect("释放受控请求");
            if fail {
                return Err(DomainError::Other("旧请求的受控失败".into()));
            }
        }
        Ok(())
    }
}

pub(super) fn detail(id: &str) -> DockerContainerDetail {
    DockerContainerDetail {
        summary: DockerContainerSummary {
            id: id.into(),
            names: vec![format!("/{id}")],
            image: Some("alpine:3.20".into()),
            image_id: None,
            command: None,
            created: None,
            state: Some("running".into()),
            status: Some("running".into()),
            health: None,
            ports: Vec::new(),
            networks: Vec::new(),
            labels: Vec::new(),
        },
        path: None,
        args: Vec::new(),
        platform: Some("linux".into()),
        working_directory: None,
        entrypoint: Vec::new(),
        command: Vec::new(),
        restart_policy: None,
        env_keys: Vec::new(),
        mounts: Vec::new(),
        networks: Vec::new(),
    }
}

#[async_trait]
impl ContainerDriver for ControlledDriver {
    async fn overview(&self, profile: &ContainerEndpointProfile) -> Result<DockerOverview> {
        self.complete(Operation::Overview).await?;
        Ok(DockerOverview {
            connection: DockerConnectionInfo {
                endpoint_id: profile.id.clone(),
                api_version: None,
                server_version: Some("controlled-engine".into()),
                server_name: None,
                operating_system: Some("linux".into()),
                architecture: None,
                read_only: true,
            },
            version: DockerEngineVersion {
                api_version: None,
                min_api_version: None,
                server_version: Some("controlled-engine".into()),
                build_time: None,
                git_commit: None,
                go_version: None,
                os: Some("linux".into()),
                architecture: None,
                kernel_version: None,
                experimental: None,
            },
            counts: DockerResourceCounts {
                containers: 1,
                running_containers: 1,
                paused_containers: 0,
                stopped_containers: 0,
                images: 1,
                networks: None,
                volumes: None,
            },
            cpu_count: Some(4),
            memory_bytes: None,
        })
    }

    async fn list_containers(
        &self,
        _profile: &ContainerEndpointProfile,
        query: &ContainerListQuery,
    ) -> Result<ContainerPage<DockerContainerSummary>> {
        self.complete(Operation::List).await?;
        Ok(ContainerPage {
            items: vec![detail("context-container").summary],
            page: query.page,
            page_size: query.page_size,
            total: 1,
            has_more: false,
        })
    }

    async fn get_container(
        &self,
        _profile: &ContainerEndpointProfile,
        id: &str,
    ) -> Result<DockerContainerDetail> {
        self.complete(Operation::Detail).await?;
        Ok(detail(id))
    }

    async fn container_logs(
        &self,
        _profile: &ContainerEndpointProfile,
        id: &str,
        _query: &ContainerLogQuery,
        _cancellation: ContainerOperationCancellation,
    ) -> Result<DockerContainerLogs> {
        // Deliberately return after cancellation: the real UI guard must reject it.
        self.complete(Operation::Logs).await?;
        Ok(DockerContainerLogs {
            container_id: id.into(),
            lines: Vec::new(),
            bytes: 0,
            dropped_lines: 0,
            truncated: false,
        })
    }

    async fn container_stats(
        &self,
        _profile: &ContainerEndpointProfile,
        id: &str,
        _cancellation: ContainerOperationCancellation,
    ) -> Result<DockerContainerStats> {
        self.complete(Operation::Stats).await?;
        Ok(DockerContainerStats {
            container_id: id.into(),
            name: None,
            read_at: None,
            cpu_percent: Some(1.0),
            memory_usage_bytes: None,
            memory_limit_bytes: None,
            memory_percent: None,
            network_rx_bytes: None,
            network_tx_bytes: None,
        })
    }
}

#[async_trait]
impl ContainerRegistryDriver for ControlledDriver {
    async fn list_repositories(
        &self,
        _profile: &ContainerRegistryProfile,
        _credential: Option<&ContainerRegistryCredential>,
    ) -> Result<Vec<ContainerRegistryRepository>> {
        self.complete(Operation::Repositories).await?;
        Ok(vec![ContainerRegistryRepository {
            name: "test/app".into(),
        }])
    }

    async fn list_tags(
        &self,
        _profile: &ContainerRegistryProfile,
        _credential: Option<&ContainerRegistryCredential>,
        repository: &str,
    ) -> Result<Vec<ContainerRegistryTag>> {
        self.complete(Operation::Tags).await?;
        Ok(vec![ContainerRegistryTag {
            repository: repository.into(),
            name: "latest".into(),
        }])
    }
}

pub(super) struct Harness<'a> {
    pub view: Entity<ContainerView>,
    pub visual: &'a mut VisualTestContext,
    pub driver: Arc<ControlledDriver>,
    pub completions: async_channel::Receiver<(u64, bool)>,
}

pub(super) fn setup(cx: &mut TestAppContext) -> Harness<'_> {
    cx.update(gpui_kit::component::init);
    cx.executor().allow_parking();
    let driver = Arc::new(ControlledDriver::default());
    let service = Arc::new(ContainerService::new(driver.clone()));
    let registry = Arc::new(ContainerRegistryService::new(driver.clone()));
    let (sender, completions) = async_channel::unbounded();
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = ContainerView::with_services(service, registry, window, cx);
            view.request_completions = Some(sender);
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    Harness {
        view: view_entity.expect("容器测试视图"),
        visual,
        driver,
        completions,
    }
}

pub(super) fn click(visual: &mut VisualTestContext, selector: &'static str) {
    visual.run_until_parked();
    let point = visual
        .debug_bounds(selector)
        .expect("容器操作入口")
        .center();
    visual.simulate_mouse_down(point, MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_up(point, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
}

impl Harness<'_> {
    pub fn generation(&mut self) -> u64 {
        self.visual.update(|_, app| self.view.read(app).request_id)
    }

    pub fn release_and_wait(&mut self, release: async_channel::Sender<()>, id: u64) {
        release.try_send(()).expect("释放旧请求");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            self.visual.run_until_parked();
            while let Ok((completed, accepted)) = self.completions.try_recv() {
                if completed == id {
                    assert!(!accepted, "旧请求不应更新当前页面");
                    return;
                }
            }
            assert!(Instant::now() < deadline, "未收到旧请求的实际 UI 回调");
            std::thread::yield_now();
        }
    }
}
