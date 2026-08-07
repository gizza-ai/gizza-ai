## About this tool

Paste up to four `.env` layers in priority order and get the resolved environment. The first layer is the lowest-priority base file, and each later layer overrides keys from the ones before it. Blank layers are skipped, so you can model the files you actually have.

The report output shows every winning `KEY=value` with a `# set by ...` provenance note, then lists the full override chain for keys that changed. Use the other output modes when you need a plain merged `.env`, shell `export` lines, JSON, a Markdown table, or only conflicting keys.

### Worked example

Layer 1:

```dotenv
APP_NAME=demo
API_URL=https://dev.example.com
DEBUG=true
API_TOKEN=dev-token-123456
```

Layer 3:

```dotenv
API_URL=https://api.example.com
CDN_URL=https://cdn.example.com
```

Layer 4:

```dotenv
API_TOKEN=prod-token-987654
```

With `output = report` and secret masking enabled, the result includes:

```text
API_URL=https://api.example.com  # set by .env.production
API_TOKEN=pr****54  # set by .env.production.local

Override chain (2 keys)
API_URL
  .env = https://dev.example.com
  .env.production = https://api.example.com  (wins)
```

Set `mask_secrets = false` when you want a copyable merged file. Set `prefix_filter = VITE_` or `NEXT_PUBLIC_` to see only variables exposed by a frontend framework.

## Limits and edge cases

This tool parses pasted dotenv text; it does not read files from disk or mutate a running process environment. It handles comments, blank lines, `export KEY=...`, single and double quotes, inline comments, duplicate keys and `${VAR}` references when expansion is enabled. Up to 20,000 distinct keys are kept.

## FAQ

<details>
<summary>Which layer wins when the same key appears multiple times?</summary>

The highest-priority layer wins. Within a single layer, the later line wins for that layer and a warning is shown with the repeated line numbers.

</details>

<details>
<summary>Why are secret values masked?</summary>

`mask_secrets` is on by default so report, JSON, shell and Markdown outputs do not accidentally display values for keys containing words like `SECRET`, `TOKEN`, `PASSWORD`, `KEY`, `AUTH` or `DSN`. Turn it off when you need a usable merged file.

</details>

<details>
<summary>Does it expand ${VAR} references?</summary>

Only when `expand_vars` is enabled. References resolve against the merged result, so they see the final winning value from any layer. Single-quoted values stay literal, and unresolved or circular references become empty strings with warnings.

</details>

<details>
<summary>Can it discover .env files automatically?</summary>

No. Browser and CLI runs operate on pasted text fields. If you want to model shell or CI variables that outrank files, paste those variables into the highest-priority layer.

</details>
