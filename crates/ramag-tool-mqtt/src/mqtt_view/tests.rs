    use super::*;
    use ramag_domain::entities::MqttTransportBackend;

    #[test]
    fn tool_metadata_exposes_mqtt_entry() {
        let tool = MqttTool::new();
        assert_eq!(tool.meta().id, MqttTool::ID);
        assert_eq!(tool.meta().name, "MQTT");
        assert_eq!(tool.meta().icon.as_deref(), Some("mqtt"));
    }

    #[test]
    fn client_permission_rows_expand_direct_and_group_roles() {
        let client = MosquittoClient {
            username: "operator".into(),
            client_id: None,
            password_configured: true,
            password: None,
            disabled: false,
            text_name: None,
            text_description: None,
            groups: vec![MosquittoGroupBinding {
                group_name: "operators".into(),
                priority: 20,
            }],
            roles: vec![MosquittoRoleBinding {
                role_name: "direct-reader".into(),
                priority: 10,
            }],
        };
        let snapshot = MosquittoDynamicSecuritySnapshot {
            clients: vec![client.clone()],
            groups: vec![MosquittoGroup {
                group_name: "operators".into(),
                text_name: None,
                text_description: None,
                roles: vec![MosquittoRoleBinding {
                    role_name: "group-writer".into(),
                    priority: 30,
                }],
            }],
            roles: vec![
                MosquittoRole {
                    role_name: "direct-reader".into(),
                    text_name: None,
                    text_description: None,
                    allow_wildcards_subscriptions: false,
                    acls: vec![MosquittoAcl {
                        acl_type: MosquittoAclType::SubscribeLiteral,
                        topic: "devices/operator/state".into(),
                        decision: MosquittoAclDecision::Allow,
                        priority: 1,
                    }],
                },
                MosquittoRole {
                    role_name: "group-writer".into(),
                    text_name: None,
                    text_description: None,
                    allow_wildcards_subscriptions: false,
                    acls: vec![MosquittoAcl {
                        acl_type: MosquittoAclType::PublishClientSend,
                        topic: "devices/operator/command".into(),
                        decision: MosquittoAclDecision::Deny,
                        priority: 2,
                    }],
                },
            ],
        };

        let (rows, missing_roles) = client_permission_rows(&snapshot, &client);
        assert_eq!(rows.len(), 2);
        assert!(missing_roles.is_empty());
        assert_eq!(rows[0].source, "直接 Role");
        assert_eq!(rows[0].role_name, "direct-reader");
        assert_eq!(rows[1].source, "Group operators");
        assert_eq!(rows[1].role_name, "group-writer");
    }

    #[test]
    fn capability_items_do_not_claim_management_or_topic_discovery() {
        let capabilities = MqttTransportCapabilities {
            backend: MqttTransportBackend::Native,
            build_available: true,
            mqtt311: true,
            mqtt5: true,
            tcp: true,
            tls: true,
            subscribe: true,
            publish: true,
            dynamic_security: false,
            static_config: false,
            metrics: false,
            retained_topics: false,
            online_clients: false,
        };
        let labels = capability_items(capabilities);
        assert!(
            !labels
                .iter()
                .any(|(label, enabled)| *enabled && label.contains("完整在线"))
        );
    }

    #[test]
    fn topic_source_labels_are_readable_instead_of_debug_names() {
        assert_eq!(mqtt_topic_source_label(MqttTopicSource::Observed), "观测");
        assert_eq!(mqtt_topic_source_label(MqttTopicSource::Retained), "保留");
        assert_eq!(mqtt_topic_source_label(MqttTopicSource::Acl), "ACL");
        assert_eq!(mqtt_topic_source_label(MqttTopicSource::Sys), "系统");
    }

    #[test]
    fn payload_formats_match_reference_client_wire_and_display_behavior() {
        assert_eq!(
            encode_publish_payload(MqttPayloadFormat::Plaintext, "hello").unwrap(),
            b"hello"
        );
        assert_eq!(
            encode_publish_payload(MqttPayloadFormat::Hex, "A1 02ff").unwrap(),
            vec![0xa1, 0x02, 0xff]
        );
        assert_eq!(
            encode_publish_payload(MqttPayloadFormat::Base64, "aGVsbG8=").unwrap(),
            b"hello"
        );
        assert_eq!(
            encode_publish_payload(MqttPayloadFormat::Base64Utf8, "hello").unwrap(),
            b"aGVsbG8="
        );
        assert_eq!(
            encode_publish_payload(MqttPayloadFormat::Base64Base64, "hello").unwrap(),
            b"hello"
        );
        assert_eq!(
            format_received_payload(MqttPayloadFormat::Hex, &[0xa1, 0x02, 0xff]),
            "A102 FF"
        );
        assert_eq!(
            format_received_payload(MqttPayloadFormat::Base64, b"hello"),
            "aGVsbG8="
        );
        assert_eq!(
            format_received_payload(MqttPayloadFormat::Json, br#"{"ok":true}"#),
            "{\n  \"ok\": true\n}"
        );
    }

    #[test]
    fn paused_timeline_does_not_append_and_clear_only_removes_local_messages() {
        use chrono::Utc;
        use std::collections::VecDeque;

        let mut messages = VecDeque::from([MqttMessage {
            topic: "devices/state".into(),
            payload: b"before".to_vec(),
            qos: MqttQos::AtMostOnce,
            retain: false,
            duplicate: false,
            received_at: Utc::now(),
            user_properties: Vec::new(),
        }]);
        let message = MqttMessage {
            topic: "devices/state".into(),
            payload: b"during-pause".to_vec(),
            qos: MqttQos::AtMostOnce,
            retain: false,
            duplicate: false,
            received_at: Utc::now(),
            user_properties: Vec::new(),
        };

        assert!(!append_timeline_message(&mut messages, true, message));
        assert_eq!(messages.len(), 1);
        messages.clear();
        assert!(messages.is_empty());

        let resumed_message = MqttMessage {
            topic: "devices/state".into(),
            payload: b"resumed".to_vec(),
            qos: MqttQos::AtMostOnce,
            retain: false,
            duplicate: false,
            received_at: Utc::now(),
            user_properties: Vec::new(),
        };
        assert!(append_timeline_message(
            &mut messages,
            false,
            resumed_message
        ));
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn message_viewer_formats_payload_and_keeps_truncated_text_bounded() {
        let (json, truncated) =
            bounded_message_view_text(MqttPayloadFormat::Json, br#"{"ok":true}"#);
        assert!(!truncated);
        assert_eq!(json, "{\n  \"ok\": true\n}");

        let payload = vec![b'x'; MAX_MESSAGE_VIEW_BYTES + 64];
        let (text, truncated) =
            bounded_message_view_text(MqttPayloadFormat::Plaintext, &payload);
        assert!(truncated);
        assert!(text.len() <= MAX_MESSAGE_VIEW_BYTES);
        assert!(text.contains("消息内容已截断"));
    }

    #[test]
    fn json_tree_keeps_structure_bounded_and_reports_invalid_payloads() {
        let (tree, error) = JsonTree::from_payload(br#"{"online":true,"items":[1,2]}"#);
        assert!(tree.is_some());
        assert!(error.is_none());

        let (tree, error) = JsonTree::from_payload(b"not-json");
        assert!(tree.is_none());
        assert!(error.expect("非法 JSON 应有原因").starts_with("JSON 解析失败："));

        let payload = serde_json::to_vec(&vec![0; MAX_JSON_TREE_NODES]).expect("数组应可序列化");
        let (tree, error) = JsonTree::from_payload(&payload);
        assert!(tree.is_none());
        assert!(error
            .expect("节点超限应有原因")
            .contains("JSON 树节点超过上限"));

        let mut nested = serde_json::Value::Null;
        for _ in 0..=MAX_JSON_TREE_DEPTH {
            nested = serde_json::Value::Array(vec![nested]);
        }
        let payload = serde_json::to_vec(&nested).expect("嵌套 JSON 应可序列化");
        let (tree, error) = JsonTree::from_payload(&payload);
        assert!(tree.is_none());
        assert!(error
            .expect("深度超限应有原因")
            .contains("JSON 树嵌套超过"));
    }
