## About this tool

Dotenv Manager parses, validates, merges and secret-masks `.env` files right in
your browser. Paste a `.env` file and it reads the same syntax your app's dotenv
loader does — `KEY=VALUE` lines, `#` comments, blank lines, single- and
double-quoted values, inline comments, and an optional `export ` prefix — then
reports what's wrong before it bites you in production.

It catches the mistakes that silently break config:

- **Duplicate keys** — the same key set twice, with a note that the **last value
  wins** (the runtime dotenv semantics), so you know which value your app actually sees.
- **Missing required keys** — list the keys your app can't start without (e.g.
  `DATABASE_URL,API_KEY`) and any that are absent are flagged.
- **Lint warnings** — keys that aren't `UPPER_SNAKE_CASE`, empty values, stray
  whitespace around `=`, unterminated quotes, and barewords with no value.

Set a second **overlay** file to merge two `.env`s the way layered environments
do — the overlay's keys override the base (last-file-wins), so you can preview
exactly what `.env` + `.env.production` resolve to.

**Mask secrets** is on by default: values of sensitive-looking keys (names
containing `SECRET`, `TOKEN`, `PASSWORD`, `KEY`, `AUTH`, and more) are shown as
`ab****yz` so you can share a report without leaking credentials.

Pick the output you need: a **report** (diagnostics plus values), a **normalized**
`.env` (deduped, last value wins), a **`.env.example`** with every value blanked
for committing to source control, or **JSON** of the merged pairs. Optionally sort
keys alphabetically. Everything runs locally — your `.env` is never uploaded.

## FAQ

<details>
<summary>Is my .env file uploaded anywhere?</summary>

No. Parsing, validation, merging and masking all run locally in your browser via
WebAssembly. Your file never leaves your machine, so it's safe to paste real
secrets — though with **Mask sensitive values** on, secret-looking values are
hidden in the output anyway.

</details>

<details>
<summary>Which key gets used when a key appears twice?</summary>

The **last** one. Real dotenv loaders keep the last assignment for a repeated key,
so the tool does the same — it flags the duplicate, lists every line the key
appears on, and the `normalized` and `json` outputs keep only the final value.

</details>

<details>
<summary>How does merging two .env files work?</summary>

Paste a second file into the **overlay** box. Its keys override matching keys in
the primary file (last-file-wins, exactly how layered `.env` + `.env.production`
files resolve at runtime), and any keys unique to the overlay are appended. Keys
only in the primary file are kept as-is. The `report` output counts how many keys
the overlay overrode.

</details>

<details>
<summary>How are secrets detected and masked?</summary>

Masking is based on the **key name**, not a fuzzy content scan, so it's
deterministic and predictable. Any key whose name contains a sensitive marker —
`SECRET`, `TOKEN`, `PASSWORD`, `PASSWD`, `PWD`, `KEY`, `AUTH`, `CRED`, `PRIVATE`,
`CERT`, `SIGNATURE`, `ACCESS`, `SESSION`, or `DSN` — has its value masked. Longer
values reveal the first and last two characters (`ab****yz`); short ones become
`****`. Turn masking off to see raw values.

</details>

<details>
<summary>What's the .env.example output for?</summary>

The `example` output emits every key with a **blank value** — a `.env.example`
template you can safely commit to git so teammates know which variables to set
without exposing any real values. Combine it with **Sort keys** for a tidy,
alphabetized template.

</details>

<details>
<summary>Can I export the parsed values as JSON?</summary>

Yes. Choose the `json` output to get a JSON object of the merged key/value pairs —
handy for feeding config into a script or comparing against another environment.
Secret values are masked in the JSON too unless you disable masking.

</details>
