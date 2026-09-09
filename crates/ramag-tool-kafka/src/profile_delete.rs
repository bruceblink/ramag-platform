use super::profile::request_matches;
use super::*;

impl KafkaView {
    pub(super) fn delete_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.deleting || self.saving || self.testing {
            return;
        }
        let Some(id) = self.selected_cluster_id.clone() else {
            return;
        };
        let Some(config) = self.cluster_by_id(&id) else {
            return;
        };
        let view = cx.entity();
        ramag_ui::open_confirm(
            "删除 Kafka 配置？",
            format!(
                "将从本机删除「{}」及其加密认证信息。不会修改 Kafka 集群。",
                config.name
            ),
            "删除",
            true,
            move |window, app| {
                view.update(app, |this, cx| this.confirm_delete(id, window, cx));
            },
            window,
            cx,
        );
    }

    pub(super) fn confirm_delete(
        &mut self,
        id: KafkaClusterId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.deleting
            || self.saving
            || self.testing
            || self.selected_cluster_id.as_ref() != Some(&id)
        {
            return;
        }
        self.profile_operation_id = self.profile_operation_id.wrapping_add(1);
        let operation_id = self.profile_operation_id;
        let context_cluster_id = self.selected_cluster_id.clone();
        self.deleting = true;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.delete_cluster(&id).await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if !request_matches(
                    this.profile_operation_id,
                    operation_id,
                    this.selected_cluster_id.as_ref(),
                    context_cluster_id.as_ref(),
                ) {
                    return;
                }
                this.deleting = false;
                match result {
                    Ok(()) => {
                        this.invalidate_runtime_request();
                        this.invalidate_message_request();
                        this.invalidate_consumer_group_request();
                        this.clear_schema_registry_snapshot();
                        this.clusters.retain(|cluster| cluster.id != id);
                        this.selected_cluster_id = None;
                        this.selected_topic = None;
                        this.metadata = None;
                        this.topics.clear();
                        this.reset_topic_paging();
                        this.consumer_groups.clear();
                        this.selected_consumer_group = None;
                        this.consumer_group_error = None;
                        this.message_page = None;
                        this.clear_message_tail(cx);
                        this.selected_message = None;
                        this.clear_acl_snapshot();
                        this.invalidate_acl_operation();
                        this.section = KafkaSection::Overview;
                        this.invalidate_topic_operation();
                        this.notice = Some(("本地 Kafka 配置已删除".into(), false));
                    }
                    Err(error) => {
                        this.notice = Some((format!("删除失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
