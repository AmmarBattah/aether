# Diagnostic bundles

Create a portable directory for an installation or bug report:

```sh
aether diagnose --out ./aether-diagnostics
aether --socket /path/to/aetherd.sock diagnose --out ./run-diagnostics \
  --run run_01J00000000000000000000004
aether diagnose --out ./recent-diagnostics --last 10
```

`--out` is required and must name a new directory under an existing parent.
The command prints its absolute path and a short file manifest. Add `--json`
to print one object with `path` and the full `manifest` instead. The directory
is private (0700), with private files (0600). Existing destinations, including
symlinks, are refused. Copy or archive this directory with your usual tools.
No archive dependency is needed.

The command reads the selected installation; it never starts, stops or changes
a daemon. `--socket` and `AETHER_SOCKET` select the socket as usual;
`AETHER_CONFIG_DIR` and `AETHER_DATA_DIR` select local configuration and logs.
For a remote installation, local config/log checks describe the CLI machine,
while status and events describe the selected daemon.

The directory contains:

- `manifest.json`: format version, collection time, file list, limits and
  redaction policy. It is written last; an interrupted or failed write leaves
  an incomplete directory without a complete manifest.
- `build.json`: CLI version, protocol version, operating system, architecture
  and whether debug assertions were enabled.
- `daemon.json`: selected socket, connection result and redacted daemon status.
- `doctor.json`: the same installation check names, levels and overall verdict
  as `aether doctor`. Check details and remediation text are redacted.
- `config.json`: loaded configuration defaults and effective built-in providers
  plus `providers.toml` overrides, with strings and caller-supplied map keys
  redacted. Numeric and boolean settings remain visible. Load failures are
  recorded without source excerpts. Unset optional scheduler settings retain
  `null`, meaning the daemon's documented default applies; CLI overrides on
  a running daemon are represented in `daemon.json`.
- `logs.json`: up to 200 lines from the last 64 KiB of the current daemon log
  segment, newest first. Only parsed timestamps and recognized log levels are
  retained; message text is redacted. Older segments are not read.
- `history.json`: metadata for up to 200 recent daemon events and up to 200
  events per selected run, with sequence cursors and replay turn ranges.
  The existing replay fold derives turn ranges from this bounded window,
  so frames may be partial and a turn whose start predates the window may
  have no replay frame. Event payloads, objectives and actor text are omitted.

By default the last five runs are selected. `--last` accepts 1 through 100;
`--run` selects one run and cannot be combined with an explicit `--last`.
A run with no retained events has empty event and replay lists.

Redaction is deliberately conservative: **all arbitrary string values** are
omitted, including innocently named environment values in MCP configuration,
provider URLs/headers, model names, status error messages, doctor details and
log text. This avoids depending on a list of known credential names or token
formats. Status counters, configuration numbers/booleans, doctor verdicts and
event IDs/types/cursors remain useful for triage. The selected socket path is
retained; home-directory paths are not anonymized. Environment contents,
credential files (including `~/.claude`, `~/.codex` and `credentials.json`),
artifacts, snapshots and full transcripts are never included. There is no
transcript inclusion flag.

A down or incompatible daemon still produces a bundle. Connection/request
errors are recorded as classes and OS error codes, rather than arbitrary
server error text. Connection attempts and daemon requests have five-second
limits; the doctor collection has a thirty-second limit. Successful bundle
creation exits zero even if checks fail or some sources are unavailable.
Invalid arguments or failed output writes exit nonzero. Review the manifest
and recorded failures before sharing the directory.
