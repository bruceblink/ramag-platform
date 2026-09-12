fn input(
    window: &mut Window,
    cx: &mut Context<MqttView>,
    max_bytes: usize,
    placeholder: &'static str,
    masked: bool,
    default_value: &'static str,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .validate(move |value, _| value.len() <= max_bytes)
            .placeholder(placeholder)
            .masked(masked)
            .default_value(default_value)
    })
}

fn set_value(
    field: &Entity<InputState>,
    value: impl Into<String>,
    window: &mut Window,
    cx: &mut Context<MqttView>,
) {
    field.update(cx, |state, cx| state.set_value(value.into(), window, cx));
}

fn value(field: &Entity<InputState>, cx: &App) -> String {
    field.read(cx).value().trim().to_string()
}

fn optional_value(field: &Entity<InputState>, cx: &App) -> Option<String> {
    let value = value(field, cx);
    (!value.is_empty()).then_some(value)
}

fn input_frame<E: IntoElement>(selector: &'static str, input: E) -> gpui::Div {
    div()
        .debug_selector(move || selector.into())
        .w_full()
        .min_w_0()
        .child(input)
}

fn parse_group_bindings(text: &str) -> std::result::Result<Vec<MosquittoGroupBinding>, String> {
    parse_binding_names(text, "Group").map(|names| {
        names
            .into_iter()
            .map(|group_name| MosquittoGroupBinding {
                group_name,
                priority: -1,
            })
            .collect()
    })
}

fn client_permission_rows(
    snapshot: &MosquittoDynamicSecuritySnapshot,
    client: &MosquittoClient,
) -> (Vec<ClientPermissionRow>, Vec<String>) {
    let mut rows = Vec::new();
    let mut missing_roles = Vec::new();
    for binding in &client.roles {
        append_role_permissions(
            snapshot,
            &mut rows,
            &mut missing_roles,
            &binding.role_name,
            "直接 Role".into(),
            binding.priority,
        );
    }
    for group_binding in &client.groups {
        let Some(group) = snapshot
            .groups
            .iter()
            .find(|group| group.group_name == group_binding.group_name)
        else {
            continue;
        };
        for role_binding in &group.roles {
            append_role_permissions(
                snapshot,
                &mut rows,
                &mut missing_roles,
                &role_binding.role_name,
                format!("Group {}", group.group_name),
                role_binding.priority,
            );
        }
    }
    (rows, missing_roles)
}

fn append_role_permissions(
    snapshot: &MosquittoDynamicSecuritySnapshot,
    rows: &mut Vec<ClientPermissionRow>,
    missing_roles: &mut Vec<String>,
    role_name: &str,
    source: String,
    binding_priority: i32,
) {
    let Some(role) = snapshot
        .roles
        .iter()
        .find(|role| role.role_name == role_name)
    else {
        if !missing_roles.iter().any(|missing| missing == role_name) {
            missing_roles.push(role_name.to_string());
        }
        return;
    };
    rows.extend(role.acls.iter().cloned().map(|acl| ClientPermissionRow {
        source: source.clone(),
        role_name: role.role_name.clone(),
        binding_priority,
        acl,
    }));
}

fn parse_role_bindings(text: &str) -> std::result::Result<Vec<MosquittoRoleBinding>, String> {
    parse_binding_names(text, "Role").map(|names| {
        names
            .into_iter()
            .map(|role_name| MosquittoRoleBinding {
                role_name,
                priority: -1,
            })
            .collect()
    })
}

fn parse_binding_names(text: &str, label: &str) -> std::result::Result<Vec<String>, String> {
    let mut names = Vec::new();
    for name in text.split([',', '\n']) {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if names.iter().any(|existing| existing == name) {
            return Err(format!("{label} 名称不能重复：{name}"));
        }
        names.push(name.to_string());
    }
    Ok(names)
}

fn join_group_bindings(bindings: &[MosquittoGroupBinding]) -> String {
    bindings
        .iter()
        .map(|binding| binding.group_name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_role_bindings(bindings: &[MosquittoRoleBinding]) -> String {
    bindings
        .iter()
        .map(|binding| binding.role_name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_acls(text: &str) -> std::result::Result<Vec<MosquittoAcl>, String> {
    let mut acls = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts = line.split('|').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "ACL 第 {} 行必须使用 acltype|topic|allow/deny|priority 格式",
                line_number + 1
            ));
        }
        let acl_type = parse_acl_type(parts[0])
            .ok_or_else(|| format!("ACL 第 {} 行的类型不受支持：{}", line_number + 1, parts[0]))?;
        let decision = match parts[2].to_ascii_lowercase().as_str() {
            "allow" | "true" => MosquittoAclDecision::Allow,
            "deny" | "false" => MosquittoAclDecision::Deny,
            _ => {
                return Err(format!(
                    "ACL 第 {} 行的决定必须是 allow 或 deny",
                    line_number + 1
                ));
            }
        };
        let priority = parts[3]
            .parse::<i32>()
            .map_err(|_| format!("ACL 第 {} 行的优先级必须是整数", line_number + 1))?;
        acls.push(MosquittoAcl {
            acl_type,
            topic: parts[1].to_string(),
            decision,
            priority,
        });
    }
    Ok(acls)
}

fn parse_acl_type(value: &str) -> Option<MosquittoAclType> {
    Some(match value {
        "publishClientSend" => MosquittoAclType::PublishClientSend,
        "publishClientReceive" => MosquittoAclType::PublishClientReceive,
        "subscribeLiteral" => MosquittoAclType::SubscribeLiteral,
        "subscribePattern" => MosquittoAclType::SubscribePattern,
        "unsubscribeLiteral" => MosquittoAclType::UnsubscribeLiteral,
        "unsubscribePattern" => MosquittoAclType::UnsubscribePattern,
        _ => return None,
    })
}

fn serialize_acls(acls: &[MosquittoAcl]) -> String {
    acls.iter()
        .map(|acl| {
            format!(
                "{}|{}|{}|{}",
                acl.acl_type.as_str(),
                acl.topic,
                match acl.decision {
                    MosquittoAclDecision::Allow => "allow",
                    MosquittoAclDecision::Deny => "deny",
                },
                acl.priority
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn field<E: IntoElement>(label: &'static str, input: E) -> gpui::Div {
    v_flex()
        .flex_1()
        .min_w(px(180.0))
        .gap(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                .child(label),
        )
        .child(div().w_full().min_w_0().child(input))
}

fn row() -> gpui::Div {
    h_flex()
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_end()
        .gap(px(10.0))
}

fn section_heading(
    title: &'static str,
    subtitle: &'static str,
    theme: &gpui_component::Theme,
) -> gpui::Div {
    v_flex()
        .w_full()
        .gap(px(2.0))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(subtitle),
        )
}

fn toggle_button<F>(
    id: &'static str,
    label: &'static str,
    selected: bool,
    disabled: bool,
    cx: &mut Context<MqttView>,
    handler: F,
) -> gpui_component::button::Button
where
    F: Fn(&mut MqttView) + 'static,
{
    let mut button = ramag_ui::clickable_button(id).xsmall().label(if selected {
        format!("{}：开", label)
    } else {
        format!("{}：关", label)
    });
    button = if selected {
        button.primary()
    } else {
        button.ghost()
    };
    button
        .disabled(disabled)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            handler(this);
            cx.notify();
        }))
}

fn metric(label: &'static str, value: usize, theme: &gpui_component::Theme) -> gpui::Div {
    v_flex()
        .gap(px(3.0))
        .p(px(10.0))
        .min_w(px(110.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
}

fn capability_items(capabilities: MqttTransportCapabilities) -> [(&'static str, bool); 8] {
    [
        ("Native 构建可用", capabilities.build_available),
        ("MQTT 3.1.1", capabilities.mqtt311),
        ("MQTT 5.0", capabilities.mqtt5),
        ("TCP", capabilities.tcp),
        ("TLS", capabilities.tls),
        ("发布", capabilities.publish),
        ("订阅", capabilities.subscribe),
        ("完整在线客户端目录", capabilities.online_clients),
    ]
}
