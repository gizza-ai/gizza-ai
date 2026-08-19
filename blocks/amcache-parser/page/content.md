## About this tool

Amcache Parser is a local DFIR helper for Windows `Amcache.hve` hives. Encode the hive as hex or Base64, paste it into the form, and extract application-inventory records that often preserve executable paths, publishers, versions, SHA-1 hashes, ProgramId links and timestamps.

The parser understands both modern `Root\\InventoryApplicationFile`, `Root\\InventoryApplication`, `Root\\InventoryDriverBinary` and `Root\\InventoryApplicationShortcut` containers, plus the legacy Windows 7/8 `Root\\File` and `Root\\Programs` schema with numeric value names. Output can be a grouped report, a dense one-line list, CSV, Sleuth Kit bodyfile rows, or a de-duplicated hash list for lookup workflows.

### Worked example

To list executable records after installing the CLI:

```bash
gizza tool amcache-parser data="$(xxd -p -c 256 Amcache.hve)" input_encoding=hex section=files mode=list association=all sort=time max_entries=200
```

For a quick browser smoke test, paste `72656766` with the default hex encoding. The tool should reject it as a truncated registry hive and explain that a full `regf` base block is required.

### Limits and edge cases

- This tool parses one pasted `Amcache.hve` at a time. It does not read live registry paths, mounted disk images or folders of hives.
- It does not replay `.LOG`, `.LOG1` or `.LOG2` registry transaction logs, so dirty hives may miss the newest appraiser writes until replayed elsewhere.
- Key last-write time is the appraiser's last observation of a record, not a guaranteed first-run time. PE link dates are compiler metadata and can be forged.
- SHA-1 values are present only when Amcache recorded a `FileId`, `Hash` or `DriverId`; records without a usable hash still appear in report/list/CSV modes.
- Unknown or vendor-specific values are preserved as extra fields instead of being silently discarded.

## FAQ

<details>
<summary>Do I paste a file path or the hive bytes?</summary>

Paste the hive bytes encoded as hex or Base64. Browser and chat blocks cannot read local disk paths directly, so encode `C:\\Windows\\AppCompat\\Programs\\Amcache.hve` first with a tool such as `xxd` or `base64`.

</details>

<details>
<summary>Does Amcache prove a program executed?</summary>

Not by itself. Amcache is strong evidence that Windows appraiser observed a file or application, and it often includes useful paths and hashes, but execution claims should be corroborated with Prefetch, ShimCache, SRUM, event logs, LNK files and other artifacts.

</details>

<details>
<summary>Why are there modern and legacy schemas?</summary>

Windows changed Amcache layout over time. Modern hives use named `Inventory*` containers, while older hives store records under `Root\\File` and `Root\\Programs` with numeric value names. This tool checks both layouts and reports which one was found.

</details>

<details>
<summary>What is the association filter?</summary>

File records may carry a `ProgramId` that links them to an installed-program record. Use `associated` to focus on files tied to a known program, or `unassociated` to surface orphan executable records that may deserve closer review.

</details>
