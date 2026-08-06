# registry-hive-parser competitor analysis — 2026-08-06

## Scope

Build a local gizza tool for opening offline Windows registry hive files (NTUSER.DAT, SYSTEM, SOFTWARE, SAM, SECURITY, USRCLASS.DAT, Amcache.hve) and triaging headers, keys, values, and Run/RunOnce autostart locations from the browser/CLI model. Inputs must be paste-friendly text encodings of hive bytes, not local file paths.

## Competitor scan

| Tool/reference | Table-stakes observed | In-model decisions | Out-of-model / not built |
| --- | --- | --- | --- |
| Browser registry-parser style hive viewers | Client-side hive parsing, NTUSER/SYSTEM/SOFTWARE support, key browsing, common artifact shortcuts such as RunKeys, no-upload privacy messaging. | Hex/Base64 local input, structured key/path browse, root summary, RunKeys sweep, explicit local/no-upload copy. | Drag-and-drop raw file UI and rich tree visualization require page file-input/tree controls beyond this block's simple manifest form. |
| Online NTUSER.DAT viewer pages | User-focused workflows for RecentDocs/UserAssist/Run keys and clear explanations that paths are relative to the hive root. | Path mode accepts hive-root-relative paths; RunKeys mode probes NTUSER.DAT per-user autostart paths and explains HKCU/HKLM prefixes are omitted. | Artifact-specific decoders for UserAssist ROT13/counts and RecentDocs shellbags are plugin-style forensic interpretation, not the core parser pass. |
| JavaScript reg-hive-parser libraries | Parse binary regf files in browser/Node and expose object-like key/value traversal. | Use a wasm-safe Rust parser (`regf`) for structured traversal and render value types safely. | Exposing the full parse tree as JSON would be too large/noisy for chat and page output; this tool renders capped text reports. |
| DFIR Toolkit hive explorer style tools | Highlight autoruns, USB/user activity, and common forensic artifacts from multiple hive families. | RunKeys mode checks per-user, machine, policy, Winlogon, BootExecute, and 32-bit-view autostart locations across NTUSER.DAT, SOFTWARE, and SYSTEM-like hives. | USB history, shellbags, Amcache program inventory, timelines, and deleted-cell recovery need broader artifact plugins and evidence models. |
| Python yarp/python-registry examples | Robust offline parsing, damaged hive handling, and forensic scripts for selected paths. | Header parser reports dirty sequence numbers, checksum status, size/truncation, and falls back to carving `nk` key names if structured traversal fails. | Transaction log replay (`.LOG`, `.LOG1`, `.LOG2`) and deleted-cell reconstruction are intentionally not implemented in this pure gizza tool. |

## Implemented UX/control choices

- `data` textarea for pasted hive bytes.
- `input_encoding` enum: `hex` and `base64`.
- `mode` enum: `summary`, `path`, and `runkeys`.
- `path` text input for hive-root-relative key paths.
- `max_entries` capped numeric text input to prevent huge reports.
- Example chips cover non-hive rejection, path browse, and RunKeys sweep.

## Verification expectations

- Exact error for non-REGF bytes.
- Unit fixtures generated with the Rust `regf` writer cover summary, path, RunKeys, damaged-root fallback, bad checksum, dirty hive, bad encodings, and output caps.
- Page/CLI docs state no log replay, no deleted-cell recovery, and no local file-path reading.
