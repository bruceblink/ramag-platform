pub fn validate_mqtt_topic_name(topic: &str) -> Result<(), String> {
    validate_protocol_text("MQTT Topic", topic, MAX_MQTT_TOPIC_BYTES)?;
    if topic.is_empty() {
        return Err("MQTT Topic 不能为空".into());
    }
    if topic.contains('+') || topic.contains('#') {
        return Err("MQTT 发布 Topic 不能包含 + 或 # 通配符".into());
    }
    Ok(())
}

pub fn validate_mqtt_topic_filter(filter: &str) -> Result<(), String> {
    validate_protocol_text("MQTT Topic Filter", filter, MAX_MQTT_TOPIC_BYTES)?;
    if filter.is_empty() {
        return Err("MQTT Topic Filter 不能为空".into());
    }

    let filter = if let Some(shared) = filter.strip_prefix("$share/") {
        let mut parts = shared.splitn(2, '/');
        let group = parts.next().unwrap_or_default();
        let filter = parts.next().unwrap_or_default();
        if group.is_empty() || group.contains('+') || group.contains('#') {
            return Err("共享订阅的 Group 名称无效".into());
        }
        if filter.is_empty() {
            return Err("共享订阅缺少 Topic Filter".into());
        }
        filter
    } else {
        filter
    };

    let levels = filter.split('/').collect::<Vec<_>>();
    for (index, level) in levels.iter().enumerate() {
        if level.contains('#') && (*level != "#" || index + 1 != levels.len()) {
            return Err("MQTT # 必须独占最后一个 Topic 层级".into());
        }
        if level.contains('+') && *level != "+" {
            return Err("MQTT + 必须独占一个 Topic 层级".into());
        }
    }
    Ok(())
}

fn validate_required_host(host: &str) -> Result<(), String> {
    validate_protocol_text("MQTT Broker 地址", host, MAX_MQTT_HOST_BYTES)?;
    if host.trim().is_empty() {
        return Err("MQTT Broker 地址不能为空".into());
    }
    if host.contains("://") || host.chars().any(char::is_whitespace) {
        return Err("MQTT Broker 地址不能包含协议前缀或空白字符".into());
    }
    Ok(())
}

fn validate_required_text(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label}不能为空"));
    }
    validate_single_line(label, value, max_bytes)
}

fn validate_optional_single_line(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_single_line(label, value, max_bytes)?;
    }
    Ok(())
}

fn validate_optional_protocol_text(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_protocol_text(label, value, max_bytes)?;
    }
    Ok(())
}

fn validate_optional_text(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_protocol_text(label, value, max_bytes)?;
    }
    Ok(())
}

fn validate_optional_path(label: &str, value: Option<&str>) -> Result<(), String> {
    if let Some(value) = value {
        validate_required_text(label, value, MAX_MQTT_TLS_PATH_BYTES)?;
    }
    Ok(())
}

fn validate_single_line(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    validate_protocol_text(label, value, max_bytes)?;
    if value.chars().any(char::is_control) {
        return Err(format!("{label}不能包含控制字符"));
    }
    Ok(())
}

fn validate_protocol_text(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.len() > max_bytes {
        return Err(format!(
            "{label}过长：{} bytes，最多 {max_bytes} bytes",
            value.len()
        ));
    }
    if value.contains('\0') {
        return Err(format!("{label}不能包含 NUL 字符"));
    }
    Ok(())
}

