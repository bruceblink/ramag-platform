    use super::*;
    use chrono::Utc;
    use ramag_domain::entities::{
        MqttMessage, MqttProtocolVersion, MqttQos, MqttTlsConfig, MqttTransport, MqttUserProperty,
        TlsVerify,
    };
    use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, Outgoing, QoS, Transport};
    use std::fs;
    use std::time::Duration;

    const MAX_TLS_FILE_BYTES: u64 = 4 * 1024 * 1024;
    const DYNSEC_REQUEST_TOPIC: &str = "$CONTROL/dynamic-security/v1";
    const DYNSEC_RESPONSE_TOPIC: &str = "$CONTROL/dynamic-security/v1/response";

    type TlsMaterial = (Option<Vec<u8>>, Option<(Vec<u8>, Vec<u8>)>);

    pub(super) async fn test_connection_native(profile: MqttProfile) -> Result<()> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => {
                let (_, eventloop) = create_v311_client(&profile)?;
                wait_v311_connection(eventloop).await
            }
            MqttProtocolVersion::V5 => {
                let (_, eventloop) = create_v5_client(&profile)?;
                wait_v5_connection(eventloop).await
            }
        }
    }

    pub(super) async fn publish_native(
        profile: MqttProfile,
        request: MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => publish_v311(&profile, &request).await,
            MqttProtocolVersion::V5 => publish_v5(&profile, &request).await,
        }
    }

    pub(super) async fn subscribe_native(
        profile: MqttProfile,
        request: MqttSubscribeRequest,
        sink: MqttMessageSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => subscribe_v311(&profile, &request, sink, cancelled).await,
            MqttProtocolVersion::V5 => subscribe_v5(&profile, &request, sink, cancelled).await,
        }
    }

    pub(super) async fn dynamic_security_snapshot_native(
        profile: MqttProfile,
    ) -> Result<MosquittoDynamicSecuritySnapshot> {
        let clients = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "listClients", "verbose": true}),
        )
        .await?;
        let groups = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "listGroups", "verbose": true}),
        )
        .await?;
        let roles = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "listRoles", "verbose": true}),
        )
        .await?;
        Ok(MosquittoDynamicSecuritySnapshot {
            clients: parse_clients(&clients)?,
            groups: parse_groups(&groups)?,
            roles: parse_roles(&roles)?,
        })
    }

    pub(super) async fn save_client_native(
        profile: MqttProfile,
        client: MosquittoClient,
    ) -> Result<()> {
        let existing = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "getClient", "username": client.username}),
        )
        .await;
        let existed = match existing {
            Ok(_) => true,
            Err(error) if is_not_found(&error) => false,
            Err(error) => return Err(error),
        };
        dynamic_security_request(&profile, client_command(&client, existed)).await?;

        if client.disabled {
            dynamic_security_request(
                &profile,
                serde_json::json!({
                    "command": "disableClient",
                    "username": client.username,
                }),
            )
            .await?;
        } else if existed {
            dynamic_security_request(
                &profile,
                serde_json::json!({
                    "command": "enableClient",
                    "username": client.username,
                }),
            )
            .await?;
        }
        Ok(())
    }

    pub(super) async fn save_group_native(
        profile: MqttProfile,
        group: MosquittoGroup,
    ) -> Result<()> {
        let existing = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "getGroup", "groupname": group.group_name}),
        )
        .await;
        let existed = match existing {
            Ok(_) => true,
            Err(error) if is_not_found(&error) => false,
            Err(error) => return Err(error),
        };
        dynamic_security_request(&profile, group_command(&group, existed))
            .await
            .map(|_| ())
    }

    pub(super) async fn save_role_native(profile: MqttProfile, role: MosquittoRole) -> Result<()> {
        let existing = dynamic_security_request(
            &profile,
            serde_json::json!({"command": "getRole", "rolename": role.role_name}),
        )
        .await;
        let existed = match existing {
            Ok(_) => true,
            Err(error) if is_not_found(&error) => false,
            Err(error) => return Err(error),
        };
        dynamic_security_request(&profile, role_command(&role, existed))
            .await
            .map(|_| ())
    }

    pub(super) async fn dynamic_security_mutation_native(
        profile: MqttProfile,
        command: &'static str,
        fields: serde_json::Value,
    ) -> Result<()> {
        let mut object = fields.as_object().cloned().ok_or_else(|| {
            DomainError::InvalidConfig("Mosquitto 管理命令字段必须是 JSON 对象".into())
        })?;
        object.insert("command".into(), serde_json::Value::String(command.into()));
        dynamic_security_request(&profile, serde_json::Value::Object(object))
            .await
            .map(|_| ())
    }
