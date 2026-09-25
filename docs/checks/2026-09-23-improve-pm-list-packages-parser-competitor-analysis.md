# pm-list-packages-parser — competitor analysis (2026-09-23)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All notes are paraphrased observations of publicly documented behaviour. No competitor
copy, branding, or trademarks are reproduced, and out-of-model items are listed, not built.

## Scope

Backlog row: *"Parses `adb shell pm list packages -f` output into a clean table and filters
into system vs user vs disabled apps."* Type hint: pure.

There is no direct "paste your `pm list packages` output" web tool in the results; the real
competition is (a) desktop ADB package managers that render the same data as a sortable table,
and (b) the shell one-liner recipes that every Android/QA blog publishes for chopping the raw
output apart. Both were treated as competitors.

## Competitors reviewed

### 1. Universal Android Debloater Next Generation (open-source Rust/ADB GUI)
- Connects to a device over ADB and shows the **complete installed-package list** in one table.
- Packages are **categorised** (OEM / carrier / Google / miscellaneous) and each row carries an
  annotation about what the package does and what depends on it.
- A dropdown **filters the list** (recommendation/safety lists, and enabled vs disabled vs
  uninstalled state).
- Supports **export/import of a package selection** so a curated list can be reused.

### 2. ADB AppControl (closed-source Windows ADB GUI)
- Same core surface: table of installed packages with the APK path, system-vs-user origin, and
  enabled/disabled state, with search and multi-select.
- Notably does **not** ship per-package descriptions (the UAD-NG wiki calls this out as the
  differentiator), so its value is purely the table + filtering + bulk actions.

### 3. The documented shell-pipeline recipes (adbshell.com man page, Linux Command Library,
   the widely-copied "list all installed packages" gist, and the Android QA blog posts)
- These define the **de-facto expected input formats**, and they are the reason a parser is
  wanted at all. Documented flags and their output shapes:
  - `-f` → `package:/data/app/~~<hash>==/<pkg>-<hash>==/base.apk=com.example.app`
  - plain → `package:com.example.app`
  - `-s` (system only), `-3` (third-party only), `-d` (disabled only), `-e` (enabled only),
    `-u` (include uninstalled-with-data)
  - `-i` → appends `  installer=com.android.vending`
  - `-U` → appends ` uid:10123`
  - `--show-versioncode` → appends ` versionCode:1234`
- The recipes people copy are `cut -d: -f2`, `grep '/data/app/'`, and `sed -e 's/.*=//'` —
  i.e. "give me just the names", "only the user apps", "split path from name". Those three are
  the table stakes a parser must cover in one step.

## Table stakes → decisions

| Table stake (seen above) | Decision |
| --- | --- |
| Accept `-f` output (`package:<path>=<pkg>`) | **In model** — primary input shape. |
| Accept plain `package:<pkg>` output | **In model** — path columns simply stay empty. |
| Tolerate `-i` / `-U` / `--show-versioncode` decorations | **In model** — trailing `installer=`, `uid:`, `versionCode:` tokens are parsed into their own columns rather than corrupting the package name. |
| Split system vs user apps | **In model** — derived from the APK partition (`/data/...` = user, `/system`, `/system_ext`, `/product`, `/vendor`, `/apex`, `/oem`, `/odm` = system). |
| Distinguish an **updated** system app (preinstalled but now living in `/data/app`) | **In model** — optional `system_list` paste (`pm list packages -s`) promotes those rows to `system-updated`; without it the path heuristic would call them user apps, which is the exact mistake the `grep '/data/app/'` recipe makes. |
| Show disabled apps | **In model, honestly** — a single `-f` run carries **no** enabled/disabled bit, so an optional `disabled_list` paste (`pm list packages -d`) supplies it. Without it the status column reads `unknown` rather than guessing. |
| Filter the list (all / user / system / enabled / disabled) | **In model** — `filter` enum. |
| Sort the table | **In model** — `sort` enum (package / type / path). |
| Group into sections with counts | **In model** — `group` boolean + a totals summary line. |
| "Just the names" (the `cut`/`sed` recipes) | **In model** — `format = list`. |
| Spreadsheet / script export | **In model** — `format = csv` and `format = json`. |
| Markdown table for tickets/wikis | **In model** — `format = markdown`. |
| Preset one-click starting points | **In model** — `[[example]]` chips (mixed device dump, user apps only, CSV export). |
| Per-package human descriptions ("what is this OEM package?") | **Out of model** — needs a curated, continuously-maintained community database of thousands of OEM/carrier package IDs; that is a dataset product, not a parser. Not built. |
| Talking to a device over ADB / running the command for you | **Out of model** — gizza blocks are sandboxed wasm with no USB/ADB access. The user runs the command; this tool parses what it prints. |
| Bulk uninstall / disable / re-enable actions | **Out of model** — same reason: no device channel, and it is a destructive device action, not a pure transform. |
| Import/export of a saved selection set | **Out of model as a feature**; the `list`/`csv`/`json` outputs are the interchange format instead. |
| App display names / icons / version names | **Out of model** — `pm list packages` never prints them (they need `dumpsys package` or the APK itself); `versionCode` is supported because `--show-versioncode` does print it. |

## UX control patterns adopted

- Multiline textareas for all three paste fields (the driver strips newlines from plain inputs).
- Friendly `<select>` labels via `[input.labels]` for every enum, so the page reads in English
  while the CLI/chat values stay canonical.
- `[[example]]` preset chips, mirroring the "pick a filter from the dropdown" affordance the
  GUI competitors lead with.
- Empty optional columns are dropped from the human `table`/`markdown` views (a device with no
  `-i`/`-U` data should not render four blank columns); `csv`/`json` keep the full fixed schema
  so downstream scripts see a stable shape.

## Limits recorded on the page

- 5,000 input lines per field (a real device lists a few hundred).
- No enabled/disabled information without the `disabled_list` paste.
- No display names, no icons, no ADB connection.
