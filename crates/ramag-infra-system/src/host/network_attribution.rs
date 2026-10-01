//! Capture once, then derive all interface counts from the retained, shareable inputs.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
const ADDRESS_SOURCE: &str = "sysinfo::Networks::refresh / NetworkData::ip_networks";
impl HostCollector {
    pub(super) fn capture_network_attribution(&mut self) -> NetworkAttribution {
        let origin = self.origin;
        let fixed_ns = self.fixed_ns;
        let clock =
            || fixed_ns.unwrap_or_else(|| origin.elapsed().as_nanos().min(u64::MAX as u128) as u64);
        let interface_addresses = capture_addresses(clock, || {
            // sysinfo has no error return here: retain exactly the addresses it exposes.
            if self.root == Path::new("/") {
                self.networks.refresh(true);
            }
            self.networks
                .iter()
                .map(|(name, data)| {
                    (
                        name.clone(),
                        data.ip_networks()
                            .iter()
                            .map(|network| network.addr.to_string())
                            .collect(),
                    )
                })
                .collect()
        });
        let order = if cfg!(target_endian = "little") {
            WordByteOrder::LittleEndian
        } else {
            WordByteOrder::BigEndian
        };
        // Each table is attempted independently, including when the other query fails.
        let tcp_v4 = capture_table("/proc/net/tcp", IpVersion::Ipv4, order, clock, || {
            self.read("/proc/net/tcp")
        });
        let tcp_v6 = capture_table("/proc/net/tcp6", IpVersion::Ipv6, order, clock, || {
            self.read("/proc/net/tcp6")
        });
        NetworkAttribution {
            interface_addresses,
            tcp_v4,
            tcp_v6,
        }
    }
}
fn capture_addresses(
    clock: impl Fn() -> u64,
    query: impl FnOnce() -> BTreeMap<String, Vec<String>>,
) -> InterfaceAddressObservation {
    let started = clock();
    let interfaces = query();
    InterfaceAddressObservation {
        query: SourceQuery {
            source: ADDRESS_SOURCE.into(),
            read_started_ns: started,
            captured_ns: clock(),
            availability: Availability::Available,
            errors: vec![],
        },
        interfaces,
    }
}
fn capture_table(
    source: &str,
    family: IpVersion,
    order: WordByteOrder,
    clock: impl Fn() -> u64,
    read: impl FnOnce() -> Result<String, String>,
) -> TcpTableObservation {
    let started = clock();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    match read() {
        Err(error) => errors.push(error),
        Ok(text) => {
            let mut lines = text.lines();
            let header = lines
                .next()
                .map(|line| line.split_whitespace().collect::<Vec<_>>());
            let trusted_header = header.as_ref().is_some_and(|h| {
                h.first() == Some(&"sl")
                    && h.get(1) == Some(&"local_address")
                    && matches!(h.get(2), Some(&"rem_address" | &"remote_address"))
                    && h.get(3) == Some(&"st")
            });
            if !trusted_header {
                errors.push(format!(
                    "{source}: line 1: missing local_address/state header"
                ));
            }
            for (index, line) in lines.enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let line_number = index as u64 + 2;
                let fields = line.split_whitespace().collect::<Vec<_>>();
                // Establish column ownership before copying any source contents. Otherwise a
                // missing column could cause a remote address or owner to masquerade as local.
                let hex = |token: &str, length| {
                    token.len() == length && token.bytes().all(|b| b.is_ascii_hexdigit())
                };
                let endpoint = |token: &&str| {
                    token.split_once(':').is_some_and(|(address, port)| {
                        hex(address, if family == IpVersion::Ipv4 { 8 } else { 32 }) && hex(port, 4)
                    })
                };
                let valid_slot = fields.first().is_some_and(|token| {
                    token.strip_suffix(':').is_some_and(|slot| {
                        !slot.is_empty() && slot.bytes().all(|b| b.is_ascii_digit())
                    })
                });
                let trusted_row = trusted_header
                    && valid_slot
                    && fields.get(1).is_some_and(endpoint)
                    && fields.get(2).is_some_and(endpoint)
                    && fields.get(3).is_some_and(|token| hex(token, 2));
                let (local_address_hex, state_hex) = if trusted_row {
                    (
                        fields[1].split_once(':').map(|(address, _)| address.into()),
                        Some(fields[3].to_string()),
                    )
                } else {
                    errors.push(format!(
                        "{source}: line {line_number}: uncertain slot/local endpoint/remote endpoint/state column structure"
                    ));
                    (None, None)
                };
                let row = TcpLocalRow {
                    line_number,
                    local_address_hex,
                    state_hex,
                };
                rows.push(row);
            }
        }
    }
    TcpTableObservation {
        query: SourceQuery {
            source: source.into(),
            read_started_ns: started,
            captured_ns: clock(),
            availability: if errors.is_empty() {
                Availability::Available
            } else {
                Availability::Failed
            },
            errors,
        },
        address_family: family,
        word_byte_order: order,
        rows,
    }
}
/// The returned readings are keyed by interface name. The caller assigns the stable sensor ID.
pub(super) fn connection_readings(capture: &NetworkAttribution) -> BTreeMap<String, Reading> {
    let interfaces = &capture.interface_addresses.interfaces;
    let mut owners: BTreeMap<IpAddr, BTreeSet<&str>> = BTreeMap::new();
    let mut input_errors = Vec::new();
    for (name, addresses) in interfaces {
        for value in addresses {
            match value.parse::<IpAddr>() {
                Ok(ip) => {
                    let ip = normalize(ip);
                    if !ip.is_unspecified() {
                        owners.entry(ip).or_default().insert(name);
                    }
                }
                Err(_) => input_errors.push(format!(
                    "{}: interface {name}: invalid local address",
                    capture.interface_addresses.query.source
                )),
            }
        }
    }
    let unique_owners = owners
        .into_iter()
        .filter_map(|(address, owners)| {
            if owners.len() == 1 {
                Some((address, *owners.first()?))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();
    let attributable = unique_owners.values().copied().collect::<BTreeSet<_>>();
    for query in [
        &capture.interface_addresses.query,
        &capture.tcp_v4.query,
        &capture.tcp_v6.query,
    ] {
        if query.availability != Availability::Available {
            if query.errors.is_empty() {
                input_errors.push(format!(
                    "{}: query is {:?}",
                    query.source, query.availability
                ));
            } else {
                input_errors.extend(query.errors.iter().cloned());
            }
        }
    }
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    if input_errors.is_empty() {
        for table in [&capture.tcp_v4, &capture.tcp_v6] {
            for row in &table.rows {
                let decoded = state(row).and_then(|state| {
                    address(row, table.address_family, table.word_byte_order)
                        .map(|address| (state, address))
                });
                match decoded {
                    Ok((1, address)) if !address.is_unspecified() => {
                        if let Some(owner) = unique_owners.get(&address) {
                            *counts.entry(owner).or_default() += 1;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => input_errors.push(format!(
                        "{}: line {}: {error}",
                        table.query.source, row.line_number
                    )),
                }
            }
        }
    }
    let start = [
        capture.interface_addresses.query.read_started_ns,
        capture.tcp_v4.query.read_started_ns,
        capture.tcp_v6.query.read_started_ns,
    ]
    .into_iter()
    .min()
    .unwrap_or(0);
    let end = [
        capture.interface_addresses.query.captured_ns,
        capture.tcp_v4.query.captured_ns,
        capture.tcp_v6.query.captured_ns,
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    interfaces
        .keys()
        .map(|name| {
            let reading = if !attributable.contains(name.as_str()) {
                missing(
                    "",
                    Availability::Unavailable,
                    "No uniquely attributable local interface address; wildcard and distinct-interface shared addresses are excluded".into(),
                )
            } else if !input_errors.is_empty() {
                missing("", Availability::Failed, input_errors.join("; "))
            } else {
                let count = counts.get(name.as_str()).copied().unwrap_or(0);
                let mut reading = measured("", count as f64, None);
                reading.observations.push(raw_window(
                    "Snapshot.network_attribution",
                    start,
                    end,
                    [("value", count)],
                ));
                reading
            };
            (name.clone(), reading)
        })
        .collect()
}
fn state(row: &TcpLocalRow) -> Result<u8, String> {
    let token = row.state_hex.as_deref().ok_or("state field is missing")?;
    if token.len() != 2 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("state field must contain two hexadecimal digits".into());
    }
    u8::from_str_radix(token, 16).map_err(|_| "state field is invalid".into())
}
fn normalize(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        _ => ip,
    }
}
fn address(row: &TcpLocalRow, family: IpVersion, order: WordByteOrder) -> Result<IpAddr, String> {
    let token = row
        .local_address_hex
        .as_deref()
        .ok_or("local_address field is missing")?;
    let expected = if family == IpVersion::Ipv4 { 8 } else { 32 };
    if token.len() != expected || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "local_address field must contain {expected} hexadecimal digits"
        ));
    }
    let word = |hex: &str| -> Result<[u8; 4], String> {
        let value = u32::from_str_radix(hex, 16).map_err(|_| "local_address word is invalid")?;
        Ok(match order {
            WordByteOrder::LittleEndian => value.to_le_bytes(),
            WordByteOrder::BigEndian => value.to_be_bytes(),
        })
    };
    if family == IpVersion::Ipv4 {
        Ok(IpAddr::V4(Ipv4Addr::from(word(token)?)))
    } else {
        let mut bytes = [0; 16];
        for (index, chunk) in bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            chunk.copy_from_slice(&word(&token[index * 8..index * 8 + 8])?);
        }
        Ok(normalize(IpAddr::V6(Ipv6Addr::from(bytes))))
    }
}
#[cfg(test)]
#[path = "../network_attribution/tests.rs"]
mod tests;
