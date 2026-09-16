//! 容器管理工具的 CMT-001 空工作台。

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, button::ButtonVariants as _,
    h_flex, v_flex,
};
use ramag_domain::entities::ContainerPlatform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerSection {
    Overview,
    Containers,
    Images,
    Networks,
    Volumes,
}

impl ContainerSection {
    const ALL: [Self; 5] = [
        Self::Overview,
        Self::Containers,
        Self::Images,
        Self::Networks,
        Self::Volumes,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Containers => "容器",
            Self::Images => "镜像",
            Self::Networks => "网络",
            Self::Volumes => "数据卷",
        }
    }

    const fn icon(self) -> IconName {
        match self {
            Self::Overview => IconName::HardDrive,
            Self::Containers => IconName::HardDrive,
            Self::Images => IconName::File,
            Self::Networks => IconName::Network,
            Self::Volumes => IconName::HardDrive,
        }
    }
}

/// CMT-001 的容器工具视图；它不持有连接客户端，避免空工作台伪造远端资源。
pub struct ContainerView {
    platform: ContainerPlatform,
    section: ContainerSection,
}

impl ContainerView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            platform: ContainerPlatform::Docker,
            section: ContainerSection::Overview,
        }
    }

    fn select_platform(&mut self, platform: ContainerPlatform, cx: &mut Context<Self>) {
        if self.platform != platform {
            self.platform = platform;
            self.section = ContainerSection::Overview;
            cx.notify();
        }
    }

    fn select_section(&mut self, section: ContainerSection, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            cx.notify();
        }
    }
}

impl Render for ContainerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (background, border, foreground, muted_foreground) = {
            let theme = cx.theme();
            (
                theme.background,
                theme.border,
                theme.foreground,
                theme.muted_foreground,
            )
        };
        let width = f32::from(window.viewport_size().width);
        let compact = width < 720.0;
        let platform = self.platform;
        let section = self.section;

        let platform_buttons = ramag_ui::responsive_toolbar()
            .id("container-platform-picker")
            .debug_selector(|| "container-platform-picker".into())
            .child(platform_button(ContainerPlatform::Docker, platform, cx))
            .child(platform_button(ContainerPlatform::Kubernetes, platform, cx));

        let header = v_flex()
            .id("container-header")
            .debug_selector(|| "container-header".into())
            .w_full()
            .flex_none()
            .gap(px(10.0))
            .px(px(20.0))
            .py(px(14.0))
            .border_b_1()
            .border_color(border)
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
                                    .text_color(muted_foreground)
                                    .child("Docker 与 Kubernetes"),
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
                            .border_color(border)
                            .rounded(px(6.0))
                            .child(
                                Icon::new(IconName::CircleX)
                                    .small()
                                    .text_color(muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_foreground)
                                    .child("暂无连接"),
                            ),
                    ),
            )
            .child(platform_buttons);

        let navigation_items = ContainerSection::ALL
            .into_iter()
            .map(|item| resource_button(item, section, cx))
            .collect::<Vec<_>>();
        let navigation = v_flex()
            .id("container-resource-nav")
            .debug_selector(|| "container-resource-nav".into())
            .w(px(176.0))
            .flex_none()
            .gap(px(4.0))
            .p(px(10.0))
            .border_r_1()
            .border_color(border)
            .children(navigation_items);

        let content = v_flex()
            .id("container-content")
            .debug_selector(|| "container-content".into())
            .size_full()
            .items_center()
            .justify_center()
            .p(px(24.0))
            .child(
                v_flex()
                    .id("container-empty-state")
                    .debug_selector(|| "container-empty-state".into())
                    .w_full()
                    .max_w(px(520.0))
                    .items_center()
                    .gap(px(10.0))
                    .text_center()
                    .child(
                        Icon::new(platform_icon(platform))
                            .large()
                            .text_color(muted_foreground),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!("暂无{}连接配置", platform.label())),
                    )
                    .child(div().text_sm().text_color(muted_foreground).child(format!(
                        "当前区域：{}。添加连接配置后，这里会显示资源列表。",
                        section.label()
                    ))),
            );

        let body = if compact {
            let compact_navigation_items = ContainerSection::ALL
                .into_iter()
                .map(|item| resource_button(item, section, cx))
                .collect::<Vec<_>>();
            let compact_navigation = ramag_ui::responsive_toolbar()
                .id("container-compact-resource-nav")
                .debug_selector(|| "container-compact-resource-nav".into())
                .px(px(12.0))
                .py(px(8.0))
                .border_b_1()
                .border_color(border)
                .children(compact_navigation_items);
            v_flex()
                .size_full()
                .child(compact_navigation)
                .child(content)
        } else {
            h_flex().size_full().child(navigation).child(content)
        };

        v_flex()
            .id("container-view")
            .debug_selector(|| "container-view".into())
            .size_full()
            .bg(background)
            .text_color(foreground)
            .child(header)
            .child(body)
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
    let icon = platform_icon(platform);
    ramag_ui::clickable_button(id)
        .ghost()
        .small()
        .icon(icon)
        .label(platform.label())
        .selected(platform == selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.select_platform(platform, cx);
        }))
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
    };
    ramag_ui::clickable_button(id)
        .ghost()
        .small()
        .w_full()
        .justify_start()
        .icon(section.icon())
        .label(section.label())
        .selected(section == selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.select_section(section, cx);
        }))
        .into_any_element()
}

const fn platform_icon(platform: ContainerPlatform) -> IconName {
    match platform {
        ContainerPlatform::Docker => IconName::HardDrive,
        ContainerPlatform::Kubernetes => IconName::Network,
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
