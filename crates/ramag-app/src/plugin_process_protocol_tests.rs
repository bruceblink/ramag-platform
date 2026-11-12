use std::io::Cursor;

use ramag_domain::traits::{CURRENT_PLUGIN_API_VERSION, MAX_PLUGIN_ENTRY_PAYLOAD_BYTES};
use ramag_domain::{PluginApiVersion, PluginCapability, PluginId};
use tokio::io::duplex;

use super::*;

fn plugin_id() -> PluginId {
    PluginId::new("example.process").unwrap()
}

fn hello() -> PluginProcessMessage {
    PluginProcessMessage::Hello(PluginHello {
        protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION,
        plugin_id: plugin_id(),
        api_version: CURRENT_PLUGIN_API_VERSION,
        granted_capabilities: vec![PluginCapability::new("ui.entry")],
        max_frame_bytes: MAX_PLUGIN_FRAME_BYTES,
    })
}

fn invoke() -> PluginProcessMessage {
    PluginProcessMessage::Invoke(PluginInvoke {
        request_id: 7,
        entry_id: "example.entry".into(),
        input: br#"{"value":true}"#.to_vec(),
    })
}

#[test]
fn frame_round_trip_preserves_message_and_uses_base64_payload() {
    let message = invoke();
    let frame = encode_frame(&message).unwrap();
    assert!(String::from_utf8_lossy(&frame[4..]).contains("eyJ2YWx1ZSI6dHJ1ZX0="));
    assert_eq!(decode_frame(&frame).unwrap(), message);
}

#[tokio::test]
async fn async_frame_helpers_read_and_write_complete_frames() {
    let (mut writer, mut reader) = duplex(MAX_PLUGIN_FRAME_BYTES);
    let message = hello();
    let write = write_frame(&mut writer, &message);
    let read = read_frame(&mut reader);
    let (write_result, read_result) = tokio::join!(write, read);
    write_result.unwrap();
    assert_eq!(read_result.unwrap(), message);
}

#[test]
fn zero_truncated_and_mismatched_frames_are_rejected() {
    assert!(matches!(
        decode_frame(&0_u32.to_le_bytes()),
        Err(PluginProcessProtocolError::ZeroLengthFrame)
    ));
    assert!(matches!(
        decode_frame(&[1, 0, 0]),
        Err(PluginProcessProtocolError::Truncated)
    ));
    assert!(matches!(
        decode_frame(&[3, 0, 0, 0, b'{']),
        Err(PluginProcessProtocolError::LengthMismatch { .. })
    ));
}

#[test]
fn oversize_frame_is_rejected_before_payload_decode() {
    let length = (MAX_PLUGIN_FRAME_BYTES + 1) as u32;
    assert!(matches!(
        decode_frame(&length.to_le_bytes()),
        Err(PluginProcessProtocolError::FrameTooLarge { .. })
    ));
}

#[test]
fn handshake_rejects_wrong_protocol_api_and_unknown_capability() {
    let wrong_protocol = PluginProcessMessage::Hello(PluginHello {
        protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION + 1,
        ..match hello() {
            PluginProcessMessage::Hello(message) => message,
            _ => unreachable!(),
        }
    });
    assert!(matches!(
        encode_frame(&wrong_protocol),
        Err(PluginProcessProtocolError::UnsupportedProtocolVersion { .. })
    ));

    let wrong_api = PluginProcessMessage::Ready(PluginReady {
        protocol_version: CURRENT_PLUGIN_PROTOCOL_VERSION,
        plugin_id: plugin_id(),
        api_version: PluginApiVersion::new(2, 0),
        capabilities: Vec::new(),
    });
    assert!(matches!(
        encode_frame(&wrong_api),
        Err(PluginProcessProtocolError::UnsupportedApiVersion { .. })
    ));

    let unknown_capability = PluginProcessMessage::Hello(PluginHello {
        granted_capabilities: vec![PluginCapability::new("network.any")],
        ..match hello() {
            PluginProcessMessage::Hello(message) => message,
            _ => unreachable!(),
        }
    });
    assert!(matches!(
        encode_frame(&unknown_capability),
        Err(PluginProcessProtocolError::UnknownCapability { .. })
    ));
}

#[test]
fn invoke_and_response_enforce_request_and_payload_contracts() {
    let oversized = PluginProcessMessage::Invoke(PluginInvoke {
        request_id: 1,
        entry_id: "example.entry".into(),
        input: vec![0; MAX_PLUGIN_ENTRY_PAYLOAD_BYTES + 1],
    });
    assert!(matches!(
        encode_frame(&oversized),
        Err(PluginProcessProtocolError::PayloadTooLarge { .. })
    ));

    let zero_request = PluginProcessMessage::Shutdown(PluginShutdown { request_id: 0 });
    assert!(matches!(
        encode_frame(&zero_request),
        Err(PluginProcessProtocolError::InvalidRequestId)
    ));

    let success_with_error = PluginProcessMessage::Response(PluginResponse {
        request_id: 1,
        ok: true,
        output: Vec::new(),
        error: Some("unexpected".into()),
    });
    assert!(matches!(
        encode_frame(&success_with_error),
        Err(PluginProcessProtocolError::ResponseShape)
    ));
}

#[test]
fn response_error_is_bounded_and_binary_output_round_trips() {
    let response = PluginProcessMessage::Response(PluginResponse {
        request_id: 2,
        ok: false,
        output: Vec::new(),
        error: Some("x".repeat(MAX_PLUGIN_PROTOCOL_ERROR_BYTES + 1)),
    });
    assert!(matches!(
        encode_frame(&response),
        Err(PluginProcessProtocolError::ErrorTooLong { .. })
    ));

    let response = PluginProcessMessage::Response(PluginResponse {
        request_id: 3,
        ok: true,
        output: vec![0, 1, 2, 255],
        error: None,
    });
    let frame = encode_frame(&response).unwrap();
    assert_eq!(decode_frame(&frame).unwrap(), response);
}

#[test]
fn cursor_can_decode_a_frame_without_async_runtime() {
    let frame = encode_frame(&hello()).unwrap();
    let mut cursor = Cursor::new(frame);
    let mut prefix = [0_u8; 4];
    std::io::Read::read_exact(&mut cursor, &mut prefix).unwrap();
    let length = u32::from_le_bytes(prefix) as usize;
    let mut payload = vec![0; length];
    std::io::Read::read_exact(&mut cursor, &mut payload).unwrap();
    let mut full = prefix.to_vec();
    full.extend(payload);
    assert_eq!(decode_frame(&full).unwrap(), hello());
}
