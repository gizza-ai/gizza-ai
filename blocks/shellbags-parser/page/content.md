## About this tool

Shellbags Parser is a local DFIR helper for offline Windows shellbag analysis. Paste a `UsrClass.dat` or Windows XP `NTUSER.DAT` hive encoded as hex or Base64, then reconstruct the folder paths recorded under the `BagMRU` tree. Shellbags are useful because they can preserve folders that no longer exist, removable-drive paths, and network locations that Explorer or file dialogs browsed in the past.

The parser walks known shellbag roots, follows `MRUListEx` ordering, decodes common shell item classes (root GUID folders, volumes, file entries, network locations, URI/control-panel/delegate items), and reports `NodeSlot`, MRU position, shell-item timestamps, key last-write time, and NTFS MFT references when a `0xBEEF0004` extension block carries one. Output can be an indented tree, a flat path list, CSV, Sleuth Kit bodyfile lines, or raw per-item diagnostics for damaged and vendor-specific shell items.

### Worked example

To reconstruct a tree from a `UsrClass.dat` file after installing the CLI:

```bash
gizza tool shellbags-parser --data "$(xxd -p -c 256 UsrClass.dat)" --input-encoding hex --mode tree --bag-root auto --max-entries 200 --max-depth 32
```

For a quick smoke test without a hive, paste `72656766` with the default hex encoding. The tool should reject it as a truncated registry hive and explain that a full `regf` base block is required.

### Limits and edge cases

- This tool reads one pasted hive at a time. It does not read from live registry paths, mounted disk images, directories of profiles, or multiple correlated hives.
- It does not replay `.LOG`, `.LOG1`, or `.LOG2` registry transaction logs, so a dirty hive may miss the newest shellbag entries until logs are replayed with a forensic workstation.
- Shellbags record folders browsed by Explorer and common dialogs; they do not prove a file inside the folder was opened or that access succeeded.
- Shell-item MAC timestamps describe the folder item as recorded in the shellbag, not necessarily the exact browse time. Registry key last-write time is usually the better interaction proxy.
- Unknown shell item classes are reported with their class byte and a hex preview instead of guessed names.

## FAQ

<details>
<summary>Do I paste a file path or the hive bytes?</summary>

Paste the hive bytes encoded as hex or Base64. Browser and chat blocks cannot read your local disk path directly, so encode the file first with a command such as `xxd -p -c 256 UsrClass.dat` or `base64 -w0 UsrClass.dat`.

</details>

<details>
<summary>Which hive should I use for shellbags?</summary>

On Windows Vista and later, start with the user's `UsrClass.dat`; the default auto mode checks `Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU`. For Windows XP, shellbags are commonly in `NTUSER.DAT` under `Software\\Microsoft\\Windows\\ShellNoRoam\\BagMRU` or `Software\\Microsoft\\Windows\\Shell\\BagMRU`.

</details>

<details>
<summary>Why does the output include folders that no longer exist?</summary>

That is one of the reasons shellbags are useful. Explorer stores view settings for folders it has seen, so the registry can retain paths from deleted folders, disconnected USB drives, or unavailable network shares.

</details>

<details>
<summary>Can this tell exactly when a user opened a folder?</summary>

Not exactly. The tool reports shell-item timestamps and the registry key last-write time, but shellbags are view-preference artifacts rather than a precise audit log. Treat times as corroborating evidence and compare them with filesystem, LNK, Jump List, and event-log timelines.

</details>
