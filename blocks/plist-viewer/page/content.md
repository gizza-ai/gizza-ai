## About this tool

Paste an Apple property list and this tool renders it in a form that is easier to inspect, diff, and copy into scripts. It accepts XML plist text directly, and it also accepts Base64-encoded binary plist (`bplist00`) bytes for cases where the original file is not text.

Choose JSON when you want machine-readable output, or `tree` for a compact `plutil -p`-style view. Dictionary key order is preserved by default; turn on sorting when you want stable diffs.

Everything runs locally in your browser. The plist contents are not uploaded.
