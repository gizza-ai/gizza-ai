## About this tool

Sigma rules are portable detection rules used by security teams to describe suspicious Windows activity. This matcher lets you paste one or more Sigma YAML rules and already-parsed Windows event records, then see which events match without uploading data or running a SIEM backend.

Use it after an EVTX-to-JSON step, while developing a rule, or when you need a quick triage pass over a small set of event records. The output can be a readable report, a Markdown table, or structured JSON for another tool.

### Worked example

Rules:

```yaml
title: Encoded PowerShell
level: high
detection:
  sel:
    EventID: 4104
    ScriptBlockText|contains:
      - '-enc '
      - '-EncodedCommand'
  condition: sel
```

Events:

```json
[{"EventID":4104,"ScriptBlockText":"powershell.exe -enc SQBFAFgA"}]
```

Result: one high-severity detection, with the matching rule title and event index in the report.

### Limits and model fit

This tool evaluates parsed JSON events, not raw `.evtx` files. It implements common Sigma detection selections, modifiers, and condition expressions, but intentionally skips aggregation/correlation rules, bundled rule-set updates, backend query conversion, and filesystem directory scans. Inputs are capped at 1 MiB of rule YAML, 8 MiB of event JSON, 500 rule documents, 50,000 events, and 10,000 displayed matches.

<details>
<summary>Can this read raw Windows EVTX files?</summary>

No. Feed it JSON that was already parsed from EVTX. Keeping EVTX parsing separate makes this matcher deterministic and browser-local instead of mixing binary log decoding with rule evaluation.

</details>

<details>
<summary>Which Sigma features are supported?</summary>

The matcher supports common detection maps, list values, keyword searches, wildcards, `contains`, `startswith`, `endswith`, regex, CIDR, numeric comparisons, `exists`, `fieldref`, `all`, case-sensitive matching, base64/UTF-16/wide encodings, windash variants, and condition expressions with `and`, `or`, `not`, parentheses, and `1 of` / `all of` selection groups.

</details>

<details>
<summary>What happens to unsupported rules?</summary>

Unsupported rule documents are skipped with a reason in the report instead of being silently treated as no-match. Correlation and aggregation rules are listed as unsupported because they need stateful windows or backend pipeline context.

</details>

<details>
<summary>Why do some event fields match even when nested?</summary>

Real Windows event JSON often nests fields under `Event`, `System`, `EventData`, `UserData`, `winlog`, or `data`. The matcher searches exact keys, case-insensitive keys, dot paths, and those common containers so common EVTX-shaped JSON works without a separate field-mapping file.

</details>
