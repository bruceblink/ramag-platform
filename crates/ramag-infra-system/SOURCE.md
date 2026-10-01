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
