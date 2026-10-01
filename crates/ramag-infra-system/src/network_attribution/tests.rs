use super::*;
use std::cell::Cell;

fn independent_address(
    row: &TcpLocalRow,
    family_bytes: usize,
) -> Result<IpAddr, Box<dyn std::error::Error>> {
    let raw = row
        .local_address_hex
        .as_deref()
        .ok_or_else(|| std::io::Error::other("validated row must retain its local address"))?;
    if raw.len() != family_bytes * 2 || !raw.len().is_multiple_of(8) {
        return Err(std::io::Error::other("raw address has an invalid word layout").into());
    }
    let mut bytes = Vec::with_capacity(family_bytes);
    for chunk in raw.as_bytes().as_chunks::<8>().0 {
        let word = std::str::from_utf8(chunk)?;
        bytes.extend_from_slice(&u32::from_str_radix(word, 16)?.to_le_bytes());
    }
    match family_bytes {
        4 => Ok(IpAddr::from(<[u8; 4]>::try_from(bytes.as_slice())?)),
        16 => {
            let ipv6 = std::net::Ipv6Addr::from(<[u8; 16]>::try_from(bytes.as_slice())?);
            Ok(ipv6
                .to_ipv4_mapped()
                .map(IpAddr::V4)
                .unwrap_or(IpAddr::V6(ipv6)))
        }
        _ => Err(std::io::Error::other("unsupported address family length").into()),
    }
}
fn header() -> &'static str {
    "  sl  local_address rem_address   st tx_queue rx_queue\n"
}
fn row(local: &str, state: &str) -> String {
    let remote = if local.len() == 32 {
        "00000000000000000000000001020304"
    } else {
        "01020304"
    };
    format!("0: {local}:1234 {remote}:ABCD {state} 00000000:00000000 0 0 12345 99\n")
}
fn table(family: IpVersion, text: &str) -> TcpTableObservation {
    capture_table(
        if family == IpVersion::Ipv4 {
            "/proc/net/tcp"
        } else {
            "/proc/net/tcp6"
        },
        family,
        WordByteOrder::LittleEndian,
        || 10,
        || Ok(text.into()),
    )
}
fn capture(interfaces: &[(&str, &[&str])], v4: &str, v6: &str) -> NetworkAttribution {
    NetworkAttribution {
        interface_addresses: capture_addresses(
            || 5,
            || {
                interfaces
                    .iter()
                    .map(|(n, a)| (n.to_string(), a.iter().map(|a| a.to_string()).collect()))
                    .collect()
            },
        ),
        tcp_v4: table(IpVersion::Ipv4, v4),
        tcp_v6: table(IpVersion::Ipv6, v6),
    }
}
#[test]
fn shared_evidence_keeps_raw_rows_without_excluded_socket_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let text = format!(
        "{}{}{}{}",
        header(),
        row("0100007F", "01"),
        row("00000000", "01"),
        row("0100007F", "0A")
    );
    let c = capture(&[("lo", &["127.0.0.1"])], &text, header());
    assert_eq!(c.tcp_v4.rows.len(), 3);
    assert_eq!(
        c.tcp_v4.rows[1].local_address_hex.as_deref(),
        Some("00000000")
    );
    assert_eq!(c.tcp_v4.rows[2].state_hex.as_deref(), Some("0A"));
    let snapshot = Snapshot {
        network_attribution: Some(c),
        ..Snapshot::default()
    };
    let value = serde_json::to_value(&snapshot)?;
    let row = value["network_attribution"]["tcp_v4"]["rows"][0]
        .as_object()
        .ok_or_else(|| std::io::Error::other("serialized TCP row must be an object"))?;
    assert_eq!(
        row.keys().map(String::as_str).collect::<Vec<_>>(),
        ["line_number", "local_address_hex", "state_hex"]
    );
    let serialized = serde_json::to_string(&snapshot)?;
    assert!(!serialized.contains("ABCD"));
    assert!(!serialized.contains("12345"));
    let restored: Snapshot = serde_json::from_str(&serialized)?;
    let attribution = restored
        .network_attribution
        .ok_or_else(|| std::io::Error::other("serialized attribution must be restored"))?;
    assert_eq!(attribution.tcp_v4.rows.len(), 3);
    Ok(())
}
#[test]
fn address_and_each_table_keep_separate_query_windows_even_after_read_failure() {
    let clock = Cell::new(100u64);
    let now = || clock.get();
    let addresses = capture_addresses(now, || {
        clock.set(200);
        BTreeMap::new()
    });
    let v4 = capture_table(
        "/proc/net/tcp",
        IpVersion::Ipv4,
        WordByteOrder::LittleEndian,
        now,
        || {
            clock.set(400);
            Err("/proc/net/tcp: permission denied".into())
        },
    );
    let v6 = capture_table(
        "/proc/net/tcp6",
        IpVersion::Ipv6,
        WordByteOrder::LittleEndian,
        now,
        || {
            clock.set(900);
            Ok(header().into())
        },
    );
    assert_eq!(
        (addresses.query.read_started_ns, addresses.query.captured_ns),
        (100, 200)
    );
    assert_eq!((v4.query.read_started_ns, v4.query.captured_ns), (200, 400));
    assert_eq!((v6.query.read_started_ns, v6.query.captured_ns), (400, 900));
    assert_eq!(v4.query.availability, Availability::Failed);
    assert!(v4.query.errors[0].contains("permission denied"));
    assert_eq!(v6.query.availability, Availability::Available);
    assert!(addresses.interfaces.is_empty());
    assert_eq!(addresses.query.availability, Availability::Available);
    assert!(addresses.query.errors.is_empty());
}
#[test]
fn attribution_uses_distinct_interface_owners_and_preserved_tokens()
-> Result<(), Box<dyn std::error::Error>> {
    let v4 = format!(
        "{}{}{}{}{}{}",
        header(),
        row("0100000A", "01"),
        row("0100000A", "0A"),
        row("00000000", "01"),
        row("0200000A", "01"),
        row("0300000A", "01")
    );
    let v6 = format!(
        "{}{}{}{}",
        header(),
        row("0000000000000000FFFF00000100000A", "01"),
        row("B80D0120000000000000000001000000", "01"),
        row("00000000000000000000000000000000", "01")
    );
    let c = capture(
        &[
            ("first", &["10.0.0.1", "10.0.0.1", "2001:db8::1"]),
            ("shared-a", &["10.0.0.2"]),
            ("shared-b", &["10.0.0.2"]),
            ("second", &["10.0.0.3"]),
            ("empty", &[]),
        ],
        &v4,
        &v6,
    );
    let readings = connection_readings(&c);
    assert_eq!(readings["first"].value, Some(3.0));
    assert_eq!(readings["second"].value, Some(1.0));
    assert_eq!(readings["shared-a"].availability, Availability::Unavailable);
    assert_eq!(readings["shared-b"].value, None);
    assert_eq!(readings["empty"].value, None);
    // Independent little-endian expansion of the retained words, not the collector decoder.
    let owned = ["10.0.0.1".parse::<IpAddr>()?, "2001:db8::1".parse()?];
    let mut independent = 0;
    for (tcp_row, family_bytes) in c
        .tcp_v4
        .rows
        .iter()
        .map(|row| (row, 4))
        .chain(c.tcp_v6.rows.iter().map(|row| (row, 16)))
        .filter(|(row, _)| row.state_hex.as_deref() == Some("01"))
    {
        if owned.contains(&independent_address(tcp_row, family_bytes)?) {
            independent += 1;
        }
    }
    assert_eq!(independent, 3);
    assert_eq!(readings["first"].value, Some(independent as f64));
    assert_eq!(readings["first"].observations.len(), 1);
    Ok(())
}
#[test]
fn ipv4_ipv6_and_word_order_are_decoded_from_raw_tokens() -> Result<(), String> {
    for (hex, family, order, expected) in [
        (
            "0100007F",
            IpVersion::Ipv4,
            WordByteOrder::LittleEndian,
            "127.0.0.1",
        ),
        (
            "7F000001",
            IpVersion::Ipv4,
            WordByteOrder::BigEndian,
            "127.0.0.1",
        ),
        (
            "00000000000000000000000001000000",
            IpVersion::Ipv6,
            WordByteOrder::LittleEndian,
            "::1",
        ),
        (
            "20010DB8000000000000000000000001",
            IpVersion::Ipv6,
            WordByteOrder::BigEndian,
            "2001:db8::1",
        ),
        (
            "0000000000000000FFFF00000100007F",
            IpVersion::Ipv6,
            WordByteOrder::LittleEndian,
            "127.0.0.1",
        ),
    ] {
        let r = TcpLocalRow {
            line_number: 2,
            local_address_hex: Some(hex.into()),
            state_hex: Some("01".into()),
        };
        assert_eq!(address(&r, family, order)?.to_string(), expected);
    }
    Ok(())
}
#[test]
fn uncertain_column_ownership_redacts_both_fields_and_propagated_errors()
-> Result<(), Box<dyn std::error::Error>> {
    for line in [
        "0100007F:1234 DEADBEEF:CAFE 01 00000000:00000000",
        "0: DEADBEEF:CAFE 01 00000000:00000000",
        "0: 0100007F:1234 01 00000000:00000000",
        "0: 0100007F:1234 DEADBEEF:CAFE 00000000:00000000",
        "0: inserted 0100007F:1234 DEADBEEF:CAFE 01",
        "0: 0100007F:1234 inserted DEADBEEF:CAFE 01",
        "0: 0100007F:1234 DEADBEEF:CAFE inserted 01",
        "0: 0100007F:1234 DEADBEEF:CAFE 01:CAFE",
    ] {
        let text = format!("{}{line}\n", header());
        let captured = capture(&[("lo", &["127.0.0.1"])], &text, header());
        assert_eq!(captured.tcp_v4.query.availability, Availability::Failed);
        let raw = &captured.tcp_v4.rows[0];
        assert_eq!(raw.local_address_hex, None, "uncertain row: {line}");
        assert_eq!(raw.state_hex, None, "uncertain row: {line}");
        let readings = connection_readings(&captured);
        assert_eq!(readings["lo"].availability, Availability::Failed);
        for serialized in [
            serde_json::to_string(&captured)?,
            serde_json::to_string(&readings)?,
        ] {
            for excluded in ["DEADBEEF", "CAFE", "0100007F", "inserted"] {
                assert!(!serialized.contains(excluded), "leaked row: {serialized}");
            }
        }
    }
    let invalid_header = table(
        IpVersion::Ipv4,
        "sl remote_address local_address st\n0: DEADBEEF:CAFE 0100007F:1234 01\n",
    );
    assert_eq!(invalid_header.query.availability, Availability::Failed);
    assert_eq!(invalid_header.rows[0].local_address_hex, None);
    assert_eq!(invalid_header.rows[0].state_hex, None);
    assert!(!serde_json::to_string(&invalid_header)?.contains("DEADBEEF"));
    let valid = table(
        IpVersion::Ipv4,
        &format!("{}{}", header(), row("0100007f", "0a")),
    );
    assert_eq!(valid.query.availability, Availability::Available);
    assert_eq!(valid.rows[0].local_address_hex.as_deref(), Some("0100007f"));
    assert_eq!(valid.rows[0].state_hex.as_deref(), Some("0a"));
    Ok(())
}
#[test]
fn malformed_shifted_fields_cannot_serialize_excluded_socket_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    for (local, state, excluded) in [
        ("0100007F", "DEADBEEF:CAFE", "DEADBEEF"),
        ("0100007F", "12345", "12345"),
        ("0100007F", "00000000:CAFEBABE", "CAFEBABE"),
        ("private-owner", "01", "private-owner"),
        ("12345", "01", "12345"),
    ] {
        let text = format!("{}{}", header(), row(local, state));
        let captured = capture(&[("lo", &["127.0.0.1"])], &text, header());
        assert_eq!(captured.tcp_v4.query.availability, Availability::Failed);
        let serialized = serde_json::to_string(&captured)?;
        assert!(!serialized.contains(excluded), "leaked token: {serialized}");
        assert!(!serialized.contains("CAFE"));
        let readings = connection_readings(&captured);
        assert_eq!(readings["lo"].availability, Availability::Failed);
        assert!(!serde_json::to_string(&readings)?.contains(excluded));
        let raw = &captured.tcp_v4.rows[0];
        assert_eq!(raw.local_address_hex, None);
        assert_eq!(raw.state_hex, None);
    }
    let shifted = table(
        IpVersion::Ipv4,
        &format!("{}0: DEADBEEF 01020304:CAFE 01\n", header()),
    );
    assert_eq!(shifted.query.availability, Availability::Failed);
    assert_eq!(shifted.rows[0].local_address_hex, None);
    let serialized = serde_json::to_string(&shifted)?;
    assert!(!serialized.contains("DEADBEEF"));
    assert!(!serialized.contains("CAFE"));
    Ok(())
}
#[test]
fn malformed_tables_retain_allowed_tokens_and_context_without_faking_zero() {
    for text in [
        "".to_string(),
        format!("{}{}", header(), row("0100007F", "GG")),
        format!("{}{}", header(), row("XYZ", "0A")),
        format!("{}broken\n", header()),
    ] {
        let c = capture(&[("lo", &["127.0.0.1"])], &text, header());
        assert_eq!(c.tcp_v4.query.availability, Availability::Failed);
        assert!(!c.tcp_v4.query.errors.is_empty());
        assert!(
            c.tcp_v4
                .query
                .errors
                .iter()
                .all(|e| e.contains("/proc/net/tcp")
                    && !e.contains("ABCD")
                    && !e.contains("12345"))
        );
        let readings = connection_readings(&c);
        assert_eq!(readings["lo"].value, None);
        assert_eq!(readings["lo"].availability, Availability::Failed);
    }
    let mut c = capture(&[("lo", &["127.0.0.1"])], header(), header());
    c.tcp_v6 = capture_table(
        "/proc/net/tcp6",
        IpVersion::Ipv6,
        WordByteOrder::LittleEndian,
        || 1,
        || Err("/proc/net/tcp6: read failed".into()),
    );
    assert_eq!(
        connection_readings(&c)["lo"].availability,
        Availability::Failed
    );
}
