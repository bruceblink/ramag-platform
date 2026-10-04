# Source and licensing

The collector implementation in this crate is derived from the
`crates/collectors` source tree of System Pulse at
`https://github.com/eas4ai/system-pulse`, commit
`f1be5d51d24c21fa8c740be79200bdda3df3a00c`.

The imported and adapted collector code is licensed under
`GPL-3.0-or-later`. The complete upstream license text is included in
`COPYING`. Upstream notices for Apple thermal interfaces and Intel DRM UAPI
are preserved alongside their corresponding modules under `src/apple/NOTICE.md`
and `src/intel/UAPI-NOTICE`.

The Windows thermal backend carries the upstream PawnIO Intel MSR module and
its LGPL-2.1-or-later license, provenance, and complete pinned source archive
under `vendor/pawnio-intel-msr/`. `IntelMSR.bin` and `source-upstream.tar.gz`
are preserved byte-for-byte from the upstream System Pulse checkout.

The AMD Zen 3 extension adds the unmodified signed `AMDFamily17.bin` from
PawnIO.Modules 0.2.11 under `vendor/pawnio-amd-smn/`. Its provenance notice
references the same pinned complete source archive and LGPL license above.
The extension is limited to AMD family 0x19 / model 0x21 and the fixed package
temperature SMN register 0x59800, with a bounded shared PCI mutex. It does not
expose arbitrary hardware operations. The local helper protocol tags the
Intel/AMD operands separately, validates the selected backend, and preserves
terminal error frames after a short-lived helper exits. Driver availability
is checked before requesting elevation; driver installation remains separate.

The upstream collector already depends on sysinfo `0.37.2`. Ramag's former
workspace and business-crate sysinfo declarations are replaced by this collector;
sysinfo remains a private platform-backend dependency rather than a second
application collector. Adaptations in this crate pin sysinfo to `0.38.4`, use native process creation
timestamps on macOS and Windows, add an immediate refresh request to
`SamplingService`, expose a bounded current-process memory query for Ramag
telemetry, split long upstream modules into submodules, and make bundled test
fixtures and native module paths local to this crate. GPU, energy, temperature,
and process-control backends remain included.

The `pulse-snapshot` executable was not imported. Keep this attribution and
license information with redistributions of this crate and include the
applicable license text in release packages.
