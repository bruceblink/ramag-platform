# System Pulse source notices

This directory records upstream notices for the System Pulse monitoring UI and
collector code absorbed into Ramag. The collector reference was inspected at
commit `f1be5d51d24c21fa8c740be79200bdda3df3a00c` from `F:/project/system-pulse`.
The source project declares `GPL-3.0-or-later` in its workspace manifest. The
absorbed application and collector sources therefore remain subject to the
applicable GPL obligations; this file does not relicense them.

## Font assets

The UI bundles font files copied from System Pulse's `assets/fonts/` directory.
Their notices are preserved with the assets:

- `Michroma-Regular.ttf`: Copyright 2011 The Michroma Project Authors; SIL Open
  Font License 1.1, in `crates/ramag-ui/assets/Michroma-OFL.txt`.
- `Inter-Regular.ttf` and `JetBrainsMono-Regular.ttf`: the System Pulse source
  records the Inter Project Authors and JetBrains Mono Project Authors under
  SIL Open Font License 1.1 in `Inter-JetBrainsMono-OFL.txt`.
- `IBMPlexSans-Regular.ttf` and `IBMPlexMono-Regular.ttf`: Copyright 2017 IBM
  Corp. with Reserved Font Name "Plex", under SIL Open Font License 1.1 in
  `IBM-Plex-LICENSE.txt`.

All five fonts and their notices are bundled in
`crates/ramag-tool-system/assets/fonts/`; font selection uses the same assets as
the standalone source application. Ramag's existing shared Michroma asset is
also retained for its other consumers.

## Native source notices

The Intel Linux collector layouts are based on Linux UAPI headers. The source
notice in `crates/ramag-infra-system/src/intel/UAPI-NOTICE` identifies the
MIT-licensed i915 and Xe definitions and the `GPL-2.0 WITH Linux-syscall-note`
perf-event definition, including their copyright holders and notice links.

The Apple native collector notice in
`crates/ramag-infra-system/src/apple/NOTICE.md` records the MIT-licensed
macmon and Stats references used for SMC and sensor-key interpretation. These
references are retained as notices and are not additional Ramag runtime
components.
