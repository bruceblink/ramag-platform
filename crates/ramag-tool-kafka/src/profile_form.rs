use super::*;

impl KafkaView {
    /// Restore a saved profile into editor fields without exposing stored passwords.
    pub(super) fn set_form_from_config(
        &mut self,
        config: &KafkaClusterConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_config_request();
        self.clear_schema_registry_snapshot();
        self.clear_connect_snapshot();
        self.reset_ksqldb_query();
        self.reset_acl_state(window, cx);
        self.read_only = config.read_only;
        set_value(&self.name, config.name.clone(), window, cx);
        set_value(
            &self.bootstrap_servers,
            config.bootstrap_servers.join(", "),
            window,
            cx,
        );
        set_value(
            &self.client_id,
            config.client_id.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.sasl_username,
            config.sasl_username.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.sasl_password, "", window, cx);
        self.sasl_password.update(cx, |state, cx| {
            state.set_placeholder(
                if config.sasl_password.is_some() {
                    "已保存密码，留空保持；输入新值可替换"
                } else {
                    "SASL 密码"
                },
                window,
                cx,
            );
        });
        set_value(
            &self.remark,
            config.remark.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.broker_metrics_endpoint,
            config.broker_metrics.endpoint.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.broker_metrics_username,
            config.broker_metrics.username.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.broker_metrics_password, "", window, cx);
        self.broker_metrics_password.update(cx, |state, cx| {
            state.set_placeholder(
                if config.broker_metrics.password.is_some() {
                    "已保存密码，留空保持；输入新值可替换"
                } else {
                    "指标端点密码"
                },
                window,
                cx,
            );
        });
        set_value(
            &self.schema_registry_endpoint,
            config.schema_registry.endpoint.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.schema_registry_username,
            config.schema_registry.username.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.schema_registry_password, "", window, cx);
        self.schema_registry_password.update(cx, |state, cx| {
            state.set_placeholder(
                if config.schema_registry.password.is_some() {
                    "已保存密码，留空保持；输入新值可替换"
                } else {
                    "Schema Registry 密码"
                },
                window,
                cx,
            );
        });
        set_value(
            &self.connect_endpoint,
            config.connect.endpoint.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.connect_username,
            config.connect.username.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.connect_password, "", window, cx);
        self.connect_password.update(cx, |state, cx| {
            state.set_placeholder(
                if config.connect.password.is_some() {
                    "已保存密码，留空保持；输入新值可替换"
                } else {
                    "Kafka Connect 密码"
                },
                window,
                cx,
            );
        });
        set_value(
            &self.ksqldb.endpoint,
            config.ksqldb.endpoint.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.ca_cert_path,
            config.tls.ca_cert_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_cert_path,
            config.tls.client_cert_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.client_key_path,
            config.tls.client_key_path.clone().unwrap_or_default(),
            window,
            cx,
        );
        self.security_protocol = config.security_protocol;
        self.sasl_mechanism = config.sasl_mechanism.unwrap_or(KafkaSaslMechanism::Plain);
        set_value(
            &self.topic_input,
            self.selected_topic.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(
            &self.produce_topic_input,
            self.selected_topic.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.produce_partition_input, "", window, cx);
        set_value(&self.produce_key_input, "", window, cx);
        set_value(&self.produce_value_input, "", window, cx);
        set_value(&self.topic_create_name, "", window, cx);
        set_value(&self.topic_create_partitions, "1", window, cx);
        set_value(&self.topic_create_replication_factor, "1", window, cx);
        set_value(&self.topic_target_partitions, "", window, cx);
        self.config_resource_type = KafkaConfigResourceType::Topic;
        set_value(
            &self.config_resource_name,
            self.selected_topic.clone().unwrap_or_default(),
            window,
            cx,
        );
        set_value(&self.config_value, "", window, cx);
    }
}
