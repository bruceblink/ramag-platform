use super::*;

impl KafkaView {
    pub(super) fn invalidate_schema_version_requests(&mut self) {
        self.invalidate_schema_versions_request();
        self.invalidate_schema_version_request();
    }

    fn invalidate_schema_versions_request(&mut self) {
        self.schema_versions_request_id = self.schema_versions_request_id.wrapping_add(1);
        self.loading_schema_versions = false;
        if let Some(cancelled) = self.schema_versions_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
    }

    fn invalidate_schema_version_request(&mut self) {
        self.schema_version_request_id = self.schema_version_request_id.wrapping_add(1);
        self.loading_schema_version = false;
        if let Some(cancelled) = self.schema_version_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
    }

    pub(super) fn clear_schema_version_snapshot(&mut self) {
        self.invalidate_schema_version_requests();
        self.schema_versions.clear();
        self.schema_selected_subject = None;
        self.schema_selected_version = None;
        self.schema_version_detail = None;
        self.schema_versions_error = None;
        self.schema_version_error = None;
        self.schema_versions_scroll
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.schema_version_detail_scroll
            .set_offset(gpui::point(px(0.0), px(0.0)));
    }

    /// Selects a Subject and starts a bounded version-list request; no Schema
    /// content is fetched until the selected version is known.
    pub(super) fn select_schema_subject(
        &mut self,
        subject: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_schema_version_requests();
        self.schema_versions.clear();
        self.schema_selected_version = None;
        self.schema_version_detail = None;
        self.schema_versions_error = None;
        self.schema_version_error = None;
        self.schema_versions_scroll
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.schema_version_detail_scroll
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.schema_selected_subject = Some(subject.clone());
        if let Some(config) = self.selected_config() {
            self.load_schema_versions(config, subject, window, cx);
        }
        cx.notify();
    }

    /// Reads the selected Subject's version numbers and automatically opens the
    /// highest returned version, while rejecting stale cluster or Subject work.
    pub(super) fn load_schema_versions(
        &mut self,
        config: KafkaClusterConfig,
        subject: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading_schema_versions
            || self.loading_runtime
            || self.selected_cluster_id.as_ref() != Some(&config.id)
            || self.schema_selected_subject.as_ref() != Some(&subject)
        {
            return;
        }
        self.invalidate_schema_versions_request();
        self.schema_versions_request_id = self.schema_versions_request_id.wrapping_add(1);
        let request_id = self.schema_versions_request_id;
        let cluster_id = config.id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let service = self.service.clone();
        self.schema_versions_cancelled = Some(cancelled.clone());
        self.loading_schema_versions = true;
        self.schema_versions_error = None;
        self.schema_version_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .list_schema_versions_with_cancel(&config, &subject, cancelled)
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.schema_versions_request_id != request_id
                    || this.selected_cluster_id.as_ref() != Some(&cluster_id)
                    || this.schema_selected_subject.as_ref() != Some(&subject)
                {
                    return;
                }
                this.loading_schema_versions = false;
                this.schema_versions_cancelled = None;
                match result {
                    Ok(versions) => {
                        this.schema_versions = versions;
                        this.schema_versions_error = None;
                        if let Some(version) = this.schema_versions.iter().copied().max() {
                            this.schema_selected_version = Some(version);
                            this.load_schema_version(
                                config.clone(),
                                subject.clone(),
                                version,
                                window,
                                cx,
                            );
                        }
                    }
                    Err(error) => {
                        this.schema_versions.clear();
                        this.schema_selected_version = None;
                        this.schema_version_detail = None;
                        this.schema_versions_error = Some(error.user_message());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn select_schema_version(
        &mut self,
        version: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(subject) = self.schema_selected_subject.clone() else {
            return;
        };
        let Some(config) = self.selected_config() else {
            return;
        };
        if !self.schema_versions.contains(&version) {
            self.schema_version_error = Some("Schema Version 不在当前 Subject 快照中".into());
            cx.notify();
            return;
        }
        self.schema_selected_version = Some(version);
        self.load_schema_version(config, subject, version, window, cx);
    }

    fn load_schema_version(
        &mut self,
        config: KafkaClusterConfig,
        subject: String,
        version: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_cluster_id.as_ref() != Some(&config.id)
            || self.schema_selected_subject.as_ref() != Some(&subject)
        {
            return;
        }
        self.invalidate_schema_version_request();
        self.schema_version_request_id = self.schema_version_request_id.wrapping_add(1);
        let request_id = self.schema_version_request_id;
        let cluster_id = config.id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let service = self.service.clone();
        self.schema_version_cancelled = Some(cancelled.clone());
        self.loading_schema_version = true;
        self.schema_version_error = None;
        self.schema_version_detail = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .get_schema_version_with_cancel(&config, &subject, version, cancelled)
                .await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.schema_version_request_id != request_id
                    || this.selected_cluster_id.as_ref() != Some(&cluster_id)
                    || this.schema_selected_subject.as_ref() != Some(&subject)
                    || this.schema_selected_version != Some(version)
                {
                    return;
                }
                this.loading_schema_version = false;
                this.schema_version_cancelled = None;
                match result {
                    Ok(detail) => {
                        this.schema_version_detail = Some(detail);
                        this.schema_version_error = None;
                    }
                    Err(error) => {
                        this.schema_version_detail = None;
                        this.schema_version_error = Some(error.user_message());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
