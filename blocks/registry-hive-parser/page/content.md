## About this tool

Registry Hive Parser is a local DFIR helper for offline Windows registry hives such as `NTUSER.DAT`, `SYSTEM`, `SOFTWARE`, `SAM`, `SECURITY`, `USRCLASS.DAT`, and Amcache hives. Encode the hive as hex or Base64, paste it into the form, then choose a summary, a specific key path, or an autostart RunKeys sweep.

The summary mode validates the `regf` signature, parses the base-block metadata, recomputes the header checksum, flags dirty sequence numbers and truncation, and lists root subkeys/values when structured traversal succeeds. Path mode browses a backslash-separated key relative to the hive root, so use `Software\\Microsoft\\Windows\\CurrentVersion\\Run` for an `NTUSER.DAT` hive rather than adding `HKCU`. RunKeys mode probes common per-user, machine-wide, policy, Winlogon, BootExecute, and 32-bit-view autostart locations used during incident response.

### Worked example

To inspect an NTUSER Run key from the command line after installing the CLI:

```bash
gizza tool registry-hive-parser --data "$(xxd -p -c 256 NTUSER.DAT)" --mode path --path "Software\\Microsoft\\Windows\\CurrentVersion\\Run" --max-entries 25
```

For a quick browser smoke test, paste `504b0304140000000800` with summary mode. The tool should reject it as a ZIP header, not a registry hive. Real hive bytes begin with ASCII `regf` (`72 65 67 66` in hex) and must include the 4096-byte base block.

### Limits and edge cases

- This tool does not replay `.LOG`, `.LOG1`, or `.LOG2` transaction logs. Dirty hives are flagged so you can decide whether to replay logs in a forensic workstation first.
- Deleted-cell recovery, timeline reconstruction, transaction-log replay, and plugin-style artifact interpretation are out of scope for the current gizza model.
- Damaged hives still get a header report. When the key tree cannot be walked, the tool may carve key names from raw `nk` cells, but carved names cannot be reliably tied back to parent paths or values.
- Paste-friendly inputs are text encodings of the hive bytes. Very large hives may produce long output; use `max_entries` to keep reports reviewable.

## FAQ

<details>
<summary>Do I paste a file path or the hive bytes?</summary>

Paste the hive bytes encoded as hex or Base64. Browser and chat blocks cannot read your local disk path directly, so encode the file first, for example with `xxd -p NTUSER.DAT` or `base64 NTUSER.DAT`.

</details>

<details>
<summary>Should my path include HKCU or HKLM?</summary>

No. An offline hive starts at its own root. For `NTUSER.DAT`, enter a path such as `Software\\Microsoft\\Windows\\CurrentVersion\\Run`; for a `SOFTWARE` hive, enter `Microsoft\\Windows\\CurrentVersion\\Run`.

</details>

<details>
<summary>Can it recover deleted keys or replay registry logs?</summary>

No. It reports live keys/values when the hive tree parses and can carve key names from damaged hives as a fallback. It does not replay transaction logs or reconstruct deleted-cell timelines.

</details>

<details>
<summary>Why does RunKeys mode say a location is missing?</summary>

RunKeys mode checks known paths across NTUSER.DAT, SOFTWARE, and SYSTEM-style hives. A missing path usually means the loaded hive family does not contain that location or the software has no values configured there.

</details>
