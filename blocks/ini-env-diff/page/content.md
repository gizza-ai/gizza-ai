## About this tool

**INI / .env Diff** compares two configuration files — `.env` dotenv files or
`.ini`/`.conf` files — and reports exactly which settings changed between them,
so you never have to eyeball two configs side by side again.

Paste your **first (left / old)** file and your **second (right / new)** file.
The tool parses both into key→value pairs and groups every difference into four
buckets, relative to the left→right direction:

- **Added** — keys present only in the *right* file.
- **Removed** — keys present only in the *left* file.
- **Changed** — keys in both files whose value differs (shown `old -> new`).
- **Unchanged** — keys present in both with an identical value.

A one-line count summary sits at the top, e.g.:

```
Config diff (env) — 1 added, 1 removed, 1 changed, 1 unchanged

Added (1):
  + NEW_FLAG = on

Removed (1):
  - DEBUG = true

Changed (1):
  ~ DB_HOST: localhost -> prod.internal

Unchanged (1):
  DB_PORT
```

The parser handles real-world config syntax: `KEY=VALUE` and `key = value` /
`key: value` pairs, `#` and `;` comments, blank lines, single- and double-quoted
values, inline comments, a leading `export ` prefix, and `[section]` headers
(flattened to dotted `section.key`). **Parse format** is `auto` by default (it
switches to INI parsing if either file has a `[section]` header, otherwise
treats them as flat `.env`), or you can force `env` or `ini`.

Turn on **Ignore key case** to treat `DB_HOST` and `db_host` as the same key, or
**Mask secret values** to redact values of sensitive-looking keys (names
containing `SECRET`, `TOKEN`, `PASSWORD`, `KEY`, `AUTH`, …) so the diff can be
shared safely. Switch **Output** to `json` for a structured
`{ format, summary, added, removed, changed, unchanged }` object.

Everything runs **locally in your browser** via WebAssembly — your files are
never uploaded.

### Handy for

- Catching config drift between a `.env.example` and a real `.env`, or between
  dev, staging and production.
- Reviewing exactly which settings a deployment or PR changed.
- Producing a machine-readable diff for tests, audits or change logs.

### FAQ

<details>
<summary>Which file is "old" and which is "new"?</summary>

The **left** file is the old/base one and the **right** file is the new/compared one. "Added" means a key that appears only on the right, and "removed" means a key that appears only on the left — so the diff always reads as *left → right*. Swap the two boxes to reverse the direction.

</details>

<details>
<summary>Does it work with .ini / .conf files that have [section] headers?</summary>

Yes. In `auto` mode, the moment either file contains a `[section]` header the tool switches to INI parsing and reports keys as dotted `section.key` (e.g. `db.host`). You can also force it with **Parse format → INI**. Flat `.env` files with no sections stay plain `KEY` names.

</details>

<details>
<summary>How are quotes, comments and `export ` handled?</summary>

Wrapping single or double quotes are stripped from values, full-line `#`/`;` comments and blank lines are ignored, an inline ` # …` / ` ; …` comment after an unquoted value is dropped, and a leading `export ` prefix (as in `export KEY=value`) is removed before comparing. So `export API_URL="http://x"  # dev` compares as just `API_URL = http://x`.

</details>

<details>
<summary>If a key appears twice in one file, which value is used?</summary>

The **last** occurrence wins, matching how shells and dotenv loaders read a file top-to-bottom. Only that final value takes part in the diff.

</details>

<details>
<summary>Are my files uploaded anywhere?</summary>

No. The entire diff runs in your browser via WebAssembly — nothing is sent to a server. That's also why **Mask secret values** is safe to rely on when you paste a real `.env`: the masking happens locally before anything is shown.

</details>
