//! Read-only Windows default-route preference, mapped to sysinfo interface aliases.

use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfEntry2, GetIpForwardTable2, GetIpInterfaceEntry, MIB_IF_ROW2,
    MIB_IPFORWARD_ROW2, MIB_IPFORWARD_TABLE2, MIB_IPINTERFACE_ROW,
};
use windows::Win32::Networking::WinSock::{ADDRESS_FAMILY, AF_INET, AF_INET6};

const MAX_ROUTE_ROWS: usize = 65_536;
const MAX_DIAGNOSTICS: usize = 16;

pub(super) struct Preference {
    pub alias: Option<String>,
    pub failures: Vec<String>,
}

/// Owns the IP Helper allocation, including error paths and rejected route tables.
struct ForwardTable(*mut MIB_IPFORWARD_TABLE2);

impl Drop for ForwardTable {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // The pointer comes only from GetIpForwardTable2 and is freed exactly once.
            unsafe { FreeMibTable(self.0.cast()) };
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
    family_rank: u8,
    effective_metric: u64,
    interface_index: u32,
    alias: String,
}

impl Candidate {
    fn new(
        family_rank: u8,
        route_metric: u32,
        interface_metric: u32,
        interface_index: u32,
        alias: String,
    ) -> Self {
        // Windows uses MAXDWORD for an unspecified metric. Retain the route as
        // a fallback, ordered after routes with known metrics in the same family.
        let effective_metric = if route_metric == u32::MAX || interface_metric == u32::MAX {
            u64::MAX
        } else {
            u64::from(route_metric) + u64::from(interface_metric)
        };
        Self {
            family_rank,
            effective_metric,
            interface_index,
            alias,
        }
    }
}

/// Selects one currently discovered interface without opening sockets or changing routes.
/// IPv4 has product priority over IPv6; within a family Windows' route + interface
/// metric decides, followed by stable interface identity. Offline hosts return None;
/// Partial native-query failures accompany the best available preference. Diagnostics
/// are bounded and contain only API names, interface indices and error codes.
pub(super) fn preferred_alias(networks: &sysinfo::Networks) -> Preference {
    let mut best: Option<Candidate> = None;
    let mut failures = Vec::new();
    for (family_rank, family) in [(0, AF_INET), (1, AF_INET6)] {
        let mut raw = std::ptr::null_mut();
        // IP Helper writes the allocation pointer; ForwardTable owns it immediately.
        let status = unsafe { GetIpForwardTable2(family, &mut raw) };
        let table = ForwardTable(raw);
        if status.0 != 0 || table.0.is_null() {
            record_failure(
                &mut failures,
                format!("GetIpForwardTable2 family {}: error {}", family.0, status.0),
            );
            continue;
        }
        // The successful API call provides a contiguous Table array with NumEntries rows.
        let count = unsafe { (*table.0).NumEntries as usize };
        if count > MAX_ROUTE_ROWS {
            record_failure(
                &mut failures,
                format!(
                    "family {}: route table exceeds {MAX_ROUTE_ROWS} rows",
                    family.0
                ),
            );
            continue;
        }
        let routes = unsafe { std::slice::from_raw_parts((*table.0).Table.as_ptr(), count) };
        for route in routes
            .iter()
            .filter(|route| is_default_route(route, family))
        {
            let mut ip = MIB_IPINTERFACE_ROW {
                Family: family,
                InterfaceIndex: route.InterfaceIndex,
                ..Default::default()
            };
            let status = unsafe { GetIpInterfaceEntry(&mut ip) };
            if status.0 != 0 {
                record_failure(
                    &mut failures,
                    format!(
                        "GetIpInterfaceEntry interface {}: error {}",
                        route.InterfaceIndex, status.0
                    ),
                );
                continue;
            }
            if ip.DisableDefaultRoutes.0 != 0 || ip.Connected.0 == 0 {
                continue;
            }
            let mut interface = MIB_IF_ROW2 {
                InterfaceIndex: route.InterfaceIndex,
                ..Default::default()
            };
            let status = unsafe { GetIfEntry2(&mut interface) };
            if status.0 != 0 {
                record_failure(
                    &mut failures,
                    format!(
                        "GetIfEntry2 interface {}: error {}",
                        route.InterfaceIndex, status.0
                    ),
                );
                continue;
            }
            let end = interface
                .Alias
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(interface.Alias.len());
            let Ok(alias) = String::from_utf16(&interface.Alias[..end]) else {
                record_failure(
                    &mut failures,
                    format!(
                        "GetIfEntry2 interface {}: invalid UTF-16 alias",
                        route.InterfaceIndex
                    ),
                );
                continue;
            };
            if !networks.contains_key(&alias) {
                continue;
            }
            choose_candidate(
                &mut best,
                Candidate::new(
                    family_rank,
                    route.Metric,
                    ip.Metric,
                    route.InterfaceIndex,
                    alias,
                ),
            );
        }
    }
    Preference {
        alias: best.map(|candidate| candidate.alias),
        failures,
    }
}

fn choose_candidate(best: &mut Option<Candidate>, candidate: Candidate) {
    if best.as_ref().is_none_or(|best| candidate < *best) {
        *best = Some(candidate);
    }
}

fn record_failure(failures: &mut Vec<String>, failure: String) {
    if failures.len() < MAX_DIAGNOSTICS && !failures.contains(&failure) {
        failures.push(failure);
    }
}

fn is_default_route(route: &MIB_IPFORWARD_ROW2, family: ADDRESS_FAMILY) -> bool {
    if route.DestinationPrefix.PrefixLength != 0 || route.ValidLifetime == 0 {
        return false;
    }
    // Prefix is a C union. Its discriminator is checked before reading the matching address.
    unsafe {
        let prefix = route.DestinationPrefix.Prefix;
        prefix.si_family == family
            && match family {
                AF_INET => prefix.Ipv4.sin_addr.S_un.S_addr == 0,
                AF_INET6 => prefix.Ipv6.sin6_addr.u.Byte == [0; 16],
                _ => false,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_uses_effective_metrics_family_priority_and_stable_ties() {
        let ethernet = Candidate::new(0, 10, 20, 7, "Ethernet".into());
        let expensive = Candidate::new(0, 1, 100, 1, "Virtual".into());
        let ipv6 = Candidate::new(1, 0, 0, 2, "IPv6".into());
        assert!(ethernet < expensive);
        assert!(ethernet < ipv6);
        assert!(Candidate::new(0, u32::MAX, 0, 1, String::new()) > ethernet);
        assert!(Candidate::new(0, 0, u32::MAX, 1, String::new()) > ethernet);
        assert!(Candidate::new(0, 10, 20, 6, "Other".into()) < ethernet);
        let mut best = None;
        choose_candidate(&mut best, ipv6);
        assert_eq!(
            best.as_ref().map(|candidate| candidate.alias.as_str()),
            Some("IPv6")
        );
        choose_candidate(&mut best, expensive);
        choose_candidate(&mut best, ethernet);
        assert_eq!(
            best.map(|candidate| candidate.alias),
            Some("Ethernet".into())
        );
        let mut unknown = None;
        choose_candidate(
            &mut unknown,
            Candidate::new(0, u32::MAX, 0, 7, "Ethernet".into()),
        );
        assert_eq!(
            unknown.map(|candidate| candidate.alias),
            Some("Ethernet".into())
        );
    }

    #[test]
    fn default_routes_exclude_subnets_expired_entries_and_wrong_families() {
        let mut route = MIB_IPFORWARD_ROW2 {
            ValidLifetime: u32::MAX,
            ..Default::default()
        };
        route.DestinationPrefix.Prefix.si_family = AF_INET;
        assert!(is_default_route(&route, AF_INET));
        assert!(!is_default_route(&route, AF_INET6));
        route.DestinationPrefix.PrefixLength = 24;
        assert!(!is_default_route(&route, AF_INET));
        route.DestinationPrefix.PrefixLength = 0;
        route.ValidLifetime = 0;
        assert!(!is_default_route(&route, AF_INET));
        route.ValidLifetime = u32::MAX;
        route.DestinationPrefix.Prefix.si_family = AF_INET6;
        assert!(is_default_route(&route, AF_INET6));
    }

    #[test]
    fn native_preference_only_returns_a_currently_discovered_interface() {
        let networks = sysinfo::Networks::new_with_refreshed_list();
        let preference = preferred_alias(&networks);
        assert!(preference.failures.len() <= MAX_DIAGNOSTICS);
        if let Some(alias) = preference.alias {
            assert!(networks.contains_key(&alias));
        }
    }

    #[test]
    fn partial_failure_diagnostics_are_unique_and_bounded() {
        let mut failures = Vec::new();
        for code in 0..MAX_DIAGNOSTICS + 1 {
            record_failure(&mut failures, format!("error {code}"));
            record_failure(&mut failures, format!("error {code}"));
        }
        assert_eq!(failures.len(), MAX_DIAGNOSTICS);
    }
}
