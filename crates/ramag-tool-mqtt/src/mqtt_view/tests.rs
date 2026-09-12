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
