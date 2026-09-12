    fn qos_v311(qos: MqttQos) -> QoS {
        match qos {
            MqttQos::AtMostOnce => QoS::AtMostOnce,
            MqttQos::AtLeastOnce => QoS::AtLeastOnce,
            MqttQos::ExactlyOnce => QoS::ExactlyOnce,
        }
    }

    fn qos_v5(qos: MqttQos) -> rumqttc::v5::mqttbytes::QoS {
        match qos {
            MqttQos::AtMostOnce => rumqttc::v5::mqttbytes::QoS::AtMostOnce,
            MqttQos::AtLeastOnce => rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
            MqttQos::ExactlyOnce => rumqttc::v5::mqttbytes::QoS::ExactlyOnce,
        }
    }

    fn qos_from_v311(qos: QoS) -> MqttQos {
        match qos {
            QoS::AtMostOnce => MqttQos::AtMostOnce,
            QoS::AtLeastOnce => MqttQos::AtLeastOnce,
            QoS::ExactlyOnce => MqttQos::ExactlyOnce,
        }
    }

    fn qos_from_v5(qos: rumqttc::v5::mqttbytes::QoS) -> MqttQos {
        match qos {
            rumqttc::v5::mqttbytes::QoS::AtMostOnce => MqttQos::AtMostOnce,
            rumqttc::v5::mqttbytes::QoS::AtLeastOnce => MqttQos::AtLeastOnce,
            rumqttc::v5::mqttbytes::QoS::ExactlyOnce => MqttQos::ExactlyOnce,
        }
    }

    fn nonzero_packet_id(packet_id: u16) -> Option<u16> {
        (packet_id != 0).then_some(packet_id)
    }

    fn user_properties(request: &MqttPublishRequest) -> Vec<(String, String)> {
        request
            .user_properties
            .iter()
            .map(|property| (property.name.clone(), property.value.clone()))
            .collect()
    }

    fn build_v311_transport(tls: &MqttTlsConfig) -> Result<Transport> {
        let (ca, client_auth) = load_tls_material(tls)?;
        if tls.verify == TlsVerify::None {
            return Err(DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Unsupported,
                "创建 MQTT TLS 连接",
                "MQTT native 适配器不允许关闭服务器证书校验",
            )));
        }
        if tls.verify == TlsVerify::Ca {
            return Err(DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Unsupported,
                "创建 MQTT TLS 连接",
                "当前 MQTT native 适配器暂不支持仅校验证书链而跳过主机名校验",
            )));
        }
        Ok(match (ca, client_auth) {
            (None, None) => Transport::tls_with_default_config(),
            (Some(ca), auth) => Transport::tls(ca, auth, None),
            (None, Some(_)) => {
                return Err(DomainError::InvalidConfig(
                    "配置客户端证书时必须同时配置 CA 证书".into(),
                ));
            }
        })
    }

    fn build_v5_transport(tls: &MqttTlsConfig) -> Result<Transport> {
        build_v311_transport(tls)
    }

    fn load_tls_material(tls: &MqttTlsConfig) -> Result<TlsMaterial> {
        let ca = read_tls_file(tls.ca_cert_path.as_deref())?;
        let client_cert = read_tls_file(tls.client_cert_path.as_deref())?;
        let client_key = read_tls_file(tls.client_key_path.as_deref())?;
        let client_auth = match (client_cert, client_key) {
            (None, None) => None,
            (Some(cert), Some(key)) => Some((cert, key)),
            _ => {
                return Err(DomainError::InvalidConfig(
                    "客户端证书和客户端密钥必须同时配置".into(),
                ));
            }
        };
        Ok((ca, client_auth))
    }

    fn read_tls_file(path: Option<&str>) -> Result<Option<Vec<u8>>> {
        let Some(path) = path else {
            return Ok(None);
        };
        let metadata = fs::metadata(path).map_err(|_| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Tls,
                "读取 MQTT TLS 文件",
                "无法读取 MQTT TLS 文件",
            ))
        })?;
        if metadata.len() > MAX_TLS_FILE_BYTES {
            return Err(DomainError::InvalidConfig(
                "MQTT TLS 文件超过 4 MiB 上限".into(),
            ));
        }
        let contents = fs::read(path).map_err(|_| {
            DomainError::Mqtt(MqttError::new(
                MqttErrorCategory::Tls,
                "读取 MQTT TLS 文件",
                "无法读取 MQTT TLS 文件",
            ))
        })?;
        Ok(Some(contents))
    }

    fn mqtt_connection_error(message: String) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Network,
            "连接 MQTT Broker",
            format!("连接 MQTT Broker 失败：{message}"),
        ))
    }

    fn mqtt_client_error(operation: &'static str, message: String) -> DomainError {
        DomainError::Mqtt(MqttError::new(
            MqttErrorCategory::Protocol,
            operation,
            format!("{operation}失败：{message}"),
        ))
    }
