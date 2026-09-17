use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Converts the text in the publish editor into the bytes sent to the Broker.
/// The Base64+UTF-8 and Base64+Base64 modes retain the reference tool's wire behavior.
fn encode_publish_payload(format: MqttPayloadFormat, text: &str) -> Result<Vec<u8>, String> {
    match format {
        MqttPayloadFormat::Plaintext | MqttPayloadFormat::Json => Ok(text.as_bytes().to_vec()),
        MqttPayloadFormat::Hex => {
            let compact = text.chars().filter(|character| !character.is_whitespace()).collect::<String>();
            hex::decode(compact).map_err(|error| format!("Hex 载荷解码失败：{error}"))
        }
        MqttPayloadFormat::Base64 => {
            let compact = text.chars().filter(|character| !character.is_whitespace()).collect::<String>();
            BASE64
                .decode(compact)
                .map_err(|error| format!("Base64 载荷解码失败：{error}"))
        }
        MqttPayloadFormat::Base64Utf8 => Ok(BASE64.encode(text.as_bytes()).into_bytes()),
        MqttPayloadFormat::Base64Base64 => Ok(text.as_bytes().to_vec()),
    }
}

fn format_received_payload(format: MqttPayloadFormat, payload: &[u8]) -> String {
    match format {
        MqttPayloadFormat::Plaintext | MqttPayloadFormat::Base64Utf8 => {
            String::from_utf8_lossy(payload).into_owned()
        }
        MqttPayloadFormat::Json => serde_json::from_slice::<serde_json::Value>(payload)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| String::from_utf8_lossy(payload).into_owned()),
        MqttPayloadFormat::Hex => hex::encode(payload)
            .to_ascii_uppercase()
            .as_bytes()
            .chunks(4)
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
            .collect::<Vec<_>>()
            .join(" "),
        MqttPayloadFormat::Base64 | MqttPayloadFormat::Base64Base64 => BASE64.encode(payload),
    }
}

fn payload_format_selector<F>(
    id: &'static str,
    selected: MqttPayloadFormat,
    disabled: bool,
    cx: &mut Context<MqttView>,
    handler: F,
) -> gpui::Div
where
    F: Fn(&mut MqttView, MqttPayloadFormat) + Copy + 'static,
{
    let mut controls = h_flex()
        .debug_selector(move || id.into())
        .flex_wrap()
        .items_center()
        .gap(px(4.0))
        .child(div().text_xs().child("格式"));
    for format in MqttPayloadFormat::ALL {
        let label = format.label();
        let mut button = ramag_ui::clickable_button(SharedString::from(format!(
            "{id}-{}",
            format.label()
        )))
        .debug_selector(move || format!("{id}-{label}"))
        .xsmall()
        .label(label)
        .disabled(disabled)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            handler(this, format);
            cx.notify();
        }));
        button = if selected == format {
            button.primary()
        } else {
            button.ghost()
        };
        controls = controls.child(button);
    }
    controls
}
