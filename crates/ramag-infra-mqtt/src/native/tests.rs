        use super::*;

        #[test]
        fn dynamic_security_commands_keep_secrets_out_of_debug_and_preserve_optional_fields() {
            let client = MosquittoClient {
                username: "operator".into(),
                client_id: None,
                password_configured: false,
                password: Some("secret-password".into()),
                disabled: false,
                text_name: Some("Operator".into()),
                text_description: None,
                groups: Vec::new(),
                roles: Vec::new(),
            };

            let command = client_command(&client, true);
            assert_eq!(command["command"], "modifyClient");
            assert_eq!(command["username"], "operator");
            assert_eq!(command["password"], "secret-password");
            assert!(command.get("clientid").is_none());
            assert!(command.get("textdescription").is_none());
            assert!(!format!("{client:?}").contains("secret-password"));
        }

        #[test]
        fn role_commands_send_the_broker_wildcard_flag_and_supported_acl_types() {
            let role = MosquittoRole {
                role_name: "operator".into(),
                text_name: None,
                text_description: None,
                allow_wildcards_subscriptions: true,
                acls: vec![ramag_domain::entities::MosquittoAcl {
                    acl_type: ramag_domain::entities::MosquittoAclType::SubscribePattern,
                    topic: "devices/%u/#".into(),
                    decision: ramag_domain::entities::MosquittoAclDecision::Allow,
                    priority: 10,
                }],
            };

            let command = role_command(&role, false);
            assert_eq!(command["command"], "createRole");
            assert_eq!(command["allowwildcardsubs"], true);
            assert_eq!(command["acls"][0]["acltype"], "subscribePattern");
        }

        #[test]
        fn dynamic_security_response_errors_are_classified_by_broker_reason() {
            let not_found = match decode_dynamic_security_response(
                br#"{"responses":[{"error":"Client not found"}]}"#,
            ) {
                Err(DomainError::Mqtt(error)) => Some(error.category),
                _ => None,
            };
            assert_eq!(not_found, Some(MqttErrorCategory::NotFound));

            let permission = match decode_dynamic_security_response(
                br#"{"responses":[{"error":"Not authorised"}]}"#,
            ) {
                Err(DomainError::Mqtt(error)) => Some(error.category),
                _ => None,
            };
            assert_eq!(permission, Some(MqttErrorCategory::PermissionDenied));
        }

        #[test]
        fn role_parser_rejects_unsupported_acl_aliases_and_reads_wildcard_flag() {
            let response = serde_json::json!({
                "data": {
                    "roles": [{
                        "rolename": "operator",
                        "allowwildcardsubs": true,
                        "acls": [{
                            "acltype": "subscribePattern",
                            "topic": "devices/%u/#",
                            "allow": true,
                            "priority": 1
                        }]
                    }]
                }
            });
            let allows_wildcards = parse_roles(&response)
                .ok()
                .and_then(|roles| roles.first().map(|role| role.allow_wildcards_subscriptions));
            assert_eq!(allows_wildcards, Some(true));

            let unsupported = serde_json::json!({
                "data": {
                    "roles": [{
                        "rolename": "operator",
                        "acls": [{
                            "acltype": "subscribe",
                            "topic": "devices/#",
                            "allow": true
                        }]
                    }]
                }
            });
            assert!(parse_roles(&unsupported).is_err());
        }
