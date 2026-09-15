    fn client_command(client: &MosquittoClient, existing: bool) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert("username".into(), client.username.clone().into());
        if let Some(client_id) = &client.client_id {
            object.insert("clientid".into(), client_id.clone().into());
        }
        if let Some(text_name) = &client.text_name {
            object.insert("textname".into(), text_name.clone().into());
        }
        if let Some(text_description) = &client.text_description {
            object.insert("textdescription".into(), text_description.clone().into());
        }
        object.insert(
            "roles".into(),
            serde_json::Value::Array(
                client
                    .roles
                    .iter()
                    .map(|binding| {
                        serde_json::json!({
                            "rolename": binding.role_name,
                            "priority": binding.priority,
                        })
                    })
                    .collect(),
            ),
        );
        object.insert(
            "groups".into(),
            serde_json::Value::Array(
                client
                    .groups
                    .iter()
                    .map(|binding| {
                        serde_json::json!({
                            "groupname": binding.group_name,
                            "priority": binding.priority,
                        })
                    })
                    .collect(),
            ),
        );
        if let Some(password) = &client.password {
            object.insert("password".into(), password.clone().into());
        }
        object.insert(
            "command".into(),
            serde_json::Value::String(
                if existing {
                    "modifyClient"
                } else {
                    "createClient"
                }
                .into(),
            ),
        );
        serde_json::Value::Object(object)
    }

    fn group_command(group: &MosquittoGroup, existing: bool) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert(
            "command".into(),
            serde_json::Value::String(
                if existing {
                    "modifyGroup"
                } else {
                    "createGroup"
                }
                .into(),
            ),
        );
        object.insert("groupname".into(), group.group_name.clone().into());
        if let Some(text_name) = &group.text_name {
            object.insert("textname".into(), text_name.clone().into());
        }
        if let Some(text_description) = &group.text_description {
            object.insert("textdescription".into(), text_description.clone().into());
        }
        object.insert(
            "roles".into(),
            group
                .roles
                .iter()
                .map(|binding| {
                    serde_json::json!({
                        "rolename": binding.role_name,
                        "priority": binding.priority,
                    })
                })
                .collect::<Vec<_>>()
                .into(),
        );
        serde_json::Value::Object(object)
    }

    fn role_command(role: &MosquittoRole, existing: bool) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert(
            "command".into(),
            serde_json::Value::String(if existing { "modifyRole" } else { "createRole" }.into()),
        );
        object.insert("rolename".into(), role.role_name.clone().into());
        if let Some(text_name) = &role.text_name {
            object.insert("textname".into(), text_name.clone().into());
        }
        if let Some(text_description) = &role.text_description {
            object.insert("textdescription".into(), text_description.clone().into());
        }
        object.insert(
            "allowwildcardsubs".into(),
            role.allow_wildcards_subscriptions.into(),
        );
        object.insert(
            "acls".into(),
            role.acls
                .iter()
                .map(|acl| {
                    serde_json::json!({
                        "acltype": acl.acl_type.as_str(),
                        "topic": acl.topic,
                        "allow": matches!(acl.decision, ramag_domain::entities::MosquittoAclDecision::Allow),
                        "priority": acl.priority,
                    })
                })
                .collect::<Vec<_>>()
                .into(),
        );
        serde_json::Value::Object(object)
    }

    async fn dynamic_security_request(
        profile: &MqttProfile,
        command: serde_json::Value,
    ) -> Result<serde_json::Value> {
        match profile.protocol_version {
            MqttProtocolVersion::V311 => dynamic_security_request_v311(profile, command).await,
            MqttProtocolVersion::V5 => dynamic_security_request_v5(profile, command).await,
        }
    }

    async fn dynamic_security_request_v311(
        profile: &MqttProfile,
        command: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let payload =
            serde_json::to_vec(&serde_json::json!({"commands": [command]})).map_err(|error| {
                DomainError::InvalidConfig(format!("编码 Mosquitto 管理命令失败：{error}"))
            })?;
        let (client, mut eventloop) = create_v311_client(profile)?;
        let mut subscribe_sent = false;
        let mut publish_sent = false;
        loop {
            match eventloop
                .poll()
                .await
                .map_err(|error| mqtt_connection_error(error.to_string()))?
            {
                Event::Incoming(Incoming::ConnAck(_)) if !subscribe_sent => {
                    client
                        .subscribe(DYNSEC_RESPONSE_TOPIC, QoS::AtLeastOnce)
                        .await
                        .map_err(|error| {
                            mqtt_client_error("订阅 Mosquitto 管理响应", error.to_string())
                        })?;
                    subscribe_sent = true;
                }
                Event::Incoming(Incoming::SubAck(_)) if !publish_sent => {
                    client
                        .publish(
                            DYNSEC_REQUEST_TOPIC,
                            QoS::AtLeastOnce,
                            false,
                            payload.clone(),
                        )
                        .await
                        .map_err(|error| {
                            mqtt_client_error("发布 Mosquitto 管理命令", error.to_string())
                        })?;
                    publish_sent = true;
                }
                Event::Incoming(Incoming::Publish(publish))
                    if publish.topic == DYNSEC_RESPONSE_TOPIC =>
                {
                    return decode_dynamic_security_response(&publish.payload);
                }
                Event::Incoming(_) | Event::Outgoing(_) => {}
            }
        }
    }

    async fn dynamic_security_request_v5(
        profile: &MqttProfile,
        command: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let payload =
            serde_json::to_vec(&serde_json::json!({"commands": [command]})).map_err(|error| {
                DomainError::InvalidConfig(format!("编码 Mosquitto 管理命令失败：{error}"))
            })?;
        let (client, mut eventloop) = create_v5_client(profile)?;
        let mut subscribe_sent = false;
        let mut publish_sent = false;
        loop {
            match eventloop
                .poll()
                .await
                .map_err(|error| mqtt_connection_error(error.to_string()))?
            {
                rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::ConnAck(_))
                    if !subscribe_sent =>
                {
                    client
                        .subscribe(
                            DYNSEC_RESPONSE_TOPIC,
                            rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
                        )
                        .await
                        .map_err(|error| {
                            mqtt_client_error("订阅 Mosquitto 管理响应", error.to_string())
                        })?;
                    subscribe_sent = true;
                }
                rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::SubAck(_)) if !publish_sent => {
                    client
                        .publish(
                            DYNSEC_REQUEST_TOPIC,
                            rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
                            false,
                            bytes::Bytes::from(payload.clone()),
                        )
                        .await
                        .map_err(|error| {
                            mqtt_client_error("发布 Mosquitto 管理命令", error.to_string())
                        })?;
                    publish_sent = true;
                }
                rumqttc::v5::Event::Incoming(rumqttc::v5::Incoming::Publish(publish))
                    if publish.topic.as_ref() == DYNSEC_RESPONSE_TOPIC.as_bytes() =>
                {
                    return decode_dynamic_security_response(&publish.payload);
                }
                rumqttc::v5::Event::Incoming(_) | rumqttc::v5::Event::Outgoing(_) => {}
            }
        }
    }

    fn decode_dynamic_security_response(payload: &[u8]) -> Result<serde_json::Value> {
        let tree: serde_json::Value = serde_json::from_slice(payload).map_err(|error| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Protocol,
                "解析 Mosquitto 管理响应",
                format!("Mosquitto 管理响应不是有效 JSON：{error}"),
            ))
        })?;
        let response = tree
            .get("responses")
            .and_then(serde_json::Value::as_array)
            .and_then(|responses| responses.first())
            .ok_or_else(|| {
                DomainError::Mqtt(MqttError::new(
                    MqttErrorCategory::Protocol,
                    "解析 Mosquitto 管理响应",
                    "Mosquitto 管理响应缺少 responses 数组",
                ))
            })?;
        if let Some(error) = response.get("error").and_then(serde_json::Value::as_str) {
            let category = broker_error_category(error);
            return Err(DomainError::Mqtt(MqttError::new(
                category,
                "执行 Mosquitto 管理命令",
                format!("Mosquitto 管理命令被 Broker 拒绝：{error}"),
            )));
        }
        Ok(response.clone())
    }

    fn response_data(response: &serde_json::Value) -> Result<&serde_json::Value> {
        response.get("data").ok_or_else(|| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Protocol,
                "解析 Mosquitto 管理响应",
                "Mosquitto 管理响应缺少 data 对象",
            ))
        })
    }

    fn parse_clients(response: &serde_json::Value) -> Result<Vec<MosquittoClient>> {
        let values = response_data(response)?
            .get("clients")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("clients"))?;
        values.iter().map(parse_client).collect()
    }

    fn parse_client(value: &serde_json::Value) -> Result<MosquittoClient> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_management_response("client"))?;
        Ok(MosquittoClient {
            username: required_string(object, "username")?,
            client_id: optional_string(object, "clientid"),
            password_configured: object
                .get("password_configured")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            password: None,
            disabled: object
                .get("disabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            text_name: optional_string(object, "textname"),
            text_description: optional_string(object, "textdescription"),
            groups: parse_group_bindings(object.get("groups"))?,
            roles: parse_role_bindings(object.get("roles"))?,
        })
    }

    fn parse_groups(response: &serde_json::Value) -> Result<Vec<MosquittoGroup>> {
        let values = response_data(response)?
            .get("groups")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("groups"))?;
        values.iter().map(parse_group).collect()
    }

    fn parse_group(value: &serde_json::Value) -> Result<MosquittoGroup> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_management_response("group"))?;
        Ok(MosquittoGroup {
            group_name: required_string(object, "groupname")?,
            text_name: optional_string(object, "textname"),
            text_description: optional_string(object, "textdescription"),
            roles: parse_role_bindings(object.get("roles"))?,
        })
    }

    fn parse_roles(response: &serde_json::Value) -> Result<Vec<MosquittoRole>> {
        let values = response_data(response)?
            .get("roles")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("roles"))?;
        values.iter().map(parse_role).collect()
    }

    fn parse_role(value: &serde_json::Value) -> Result<MosquittoRole> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_management_response("role"))?;
        let acls = object
            .get("acls")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid_management_response("role.acls"))?
            .iter()
            .map(|value| {
                let acl = value
                    .as_object()
                    .ok_or_else(|| invalid_management_response("role.acl"))?;
                let acl_type = match required_string(acl, "acltype")?.as_str() {
                    "publishClientSend" => {
                        ramag_domain::entities::MosquittoAclType::PublishClientSend
                    }
                    "publishClientReceive" => {
                        ramag_domain::entities::MosquittoAclType::PublishClientReceive
                    }
                    "subscribeLiteral" => {
                        ramag_domain::entities::MosquittoAclType::SubscribeLiteral
                    }
                    "subscribePattern" => {
                        ramag_domain::entities::MosquittoAclType::SubscribePattern
                    }
                    "unsubscribeLiteral" => {
                        ramag_domain::entities::MosquittoAclType::UnsubscribeLiteral
                    }
                    "unsubscribePattern" => {
                        ramag_domain::entities::MosquittoAclType::UnsubscribePattern
                    }
                    _ => return Err(invalid_management_response("role.acl.acltype")),
                };
                Ok(ramag_domain::entities::MosquittoAcl {
                    acl_type,
                    topic: required_string(acl, "topic")?,
                    decision: if acl
                        .get("allow")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                    {
                        ramag_domain::entities::MosquittoAclDecision::Allow
                    } else {
                        ramag_domain::entities::MosquittoAclDecision::Deny
                    },
                    priority: acl
                        .get("priority")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(-1) as i32,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(MosquittoRole {
            role_name: required_string(object, "rolename")?,
            text_name: optional_string(object, "textname"),
            text_description: optional_string(object, "textdescription"),
            allow_wildcards_subscriptions: object
                .get("allowwildcardsubs")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            acls,
        })
    }

    fn parse_role_bindings(
        value: Option<&serde_json::Value>,
    ) -> Result<Vec<ramag_domain::entities::MosquittoRoleBinding>> {
        let Some(values) = value.and_then(serde_json::Value::as_array) else {
            return Ok(Vec::new());
        };
        values
            .iter()
            .map(|value| {
                let object = value
                    .as_object()
                    .ok_or_else(|| invalid_management_response("roles"))?;
                let priority = object
                    .get("priority")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(-1) as i32;
                Ok(ramag_domain::entities::MosquittoRoleBinding {
                    role_name: required_string(object, "rolename")?,
                    priority,
                })
            })
            .collect()
    }

    fn parse_group_bindings(
        value: Option<&serde_json::Value>,
    ) -> Result<Vec<ramag_domain::entities::MosquittoGroupBinding>> {
        let Some(values) = value.and_then(serde_json::Value::as_array) else {
            return Ok(Vec::new());
        };
        values
            .iter()
            .map(|value| {
                let object = value
                    .as_object()
                    .ok_or_else(|| invalid_management_response("groups"))?;
                Ok(ramag_domain::entities::MosquittoGroupBinding {
                    group_name: required_string(object, "groupname")?,
                    priority: object
                        .get("priority")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(-1) as i32,
                })
            })
            .collect()
    }

    fn required_string(
        object: &serde_json::Map<String, serde_json::Value>,
        key: &str,
    ) -> Result<String> {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| invalid_management_response(key))
    }

    fn optional_string(
        object: &serde_json::Map<String, serde_json::Value>,
        key: &str,
    ) -> Option<String> {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn invalid_management_response(field: &str) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Protocol,
            "解析 Mosquitto 管理响应",
            format!("Mosquitto 管理响应缺少有效字段：{field}"),
        ))
    }

    fn is_not_found(error: &DomainError) -> bool {
        matches!(
            error,
            DomainError::Mqtt(mqtt_error) if mqtt_error.category == MqttErrorCategory::NotFound
        ) || error.user_message().contains("not found")
            || error.user_message().contains("不存在")
    }

    fn broker_error_category(error: &str) -> MqttErrorCategory {
        let normalized = error.to_ascii_lowercase();
        if normalized.contains("not found") || error.contains("不存在") {
            MqttErrorCategory::NotFound
        } else if normalized.contains("not authorised")
            || normalized.contains("not authorized")
            || normalized.contains("permission")
            || normalized.contains("access denied")
        {
            MqttErrorCategory::PermissionDenied
        } else if normalized.contains("unknown command")
            || normalized.contains("unsupported")
            || normalized.contains("dynamic security") && normalized.contains("not enabled")
        {
            MqttErrorCategory::Unsupported
        } else {
            MqttErrorCategory::Protocol
        }
    }
