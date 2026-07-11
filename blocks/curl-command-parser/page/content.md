## About this tool

A **curl command** is the line you copy from API docs, a README, or your
browser's DevTools "Copy as cURL" — a shell command that fires off an HTTP
request. This tool reads one apart into its pieces so you can see exactly what it
does, or rebuilds a clean, canonical version. Everything happens in your browser;
nothing you paste is uploaded.

### Parse mode

Paste a curl command and get back every component, decoded:

- **Method** — the explicit `-X`/`--request` value, or inferred (POST when a
  body or form is present, HEAD for `-I`, otherwise GET).
- **URL** and **query parameters** — the target URL, with any `?a=b&c=d` query
  string split out and percent-decoded into name/value pairs.
- **Headers** (`-H`) — each `Name: Value` as a pair, in order.
- **Content-Type** — the effective type (an explicit header wins; otherwise
  `application/x-www-form-urlencoded` for `-d` data, `multipart/form-data` for
  `-F` forms).
- **Body** — assembled from `-d`/`--data`/`--data-ascii` (newlines stripped like
  curl does), `--data-raw`/`--data-binary` (verbatim), or `--data-urlencode`
  (percent-encoded), with the source flag noted.
- **Form fields** (`-F`), **cookies** (`-b`, parsed into pairs), and **basic
  auth** (`-u user:password`, decoded into username and password).
- **Flags** — `-k`/`--insecure`, `-L`/`--location`, `--compressed`, `-I`/`--head`,
  `-G`/`--get`, plus user-agent (`-A`) and referer (`-e`).
- Any `@file` argument (e.g. `-d @body.json`, `-F file=@photo.png`) is listed as
  a **file reference**, since the file's contents aren't part of the command
  text.

The tokenizer understands single quotes, double quotes (with `\"`, `\\`, `\$`
escapes), backslash-escaped characters, and `\`-at-end-of-line continuations, so
multi-line commands pasted straight from docs parse cleanly. A leading `curl` is
optional.

### Rebuild mode

Switch the mode to **rebuild** and the same command comes back tidied: one
`-H` per line, values correctly shell-quoted, the method shown only when it isn't
a plain GET, and flags in a stable order — a canonical form that's easy to read
and diff.

### Privacy

Everything runs locally in WebAssembly. No command, URL, header, token, or
password ever leaves your device, and there is no sign-up.

## FAQ

<details>
<summary>How does it decide the HTTP method when there's no -X?</summary>

It follows curl's own rules. An explicit `-X`/`--request` always wins. Otherwise
the method is inferred: it's **POST** when the command carries a body (`-d`,
`--data-raw`, `--data-binary`, `--data-urlencode`) or a form field (`-F`), it's
**HEAD** when `-I`/`--head` is present, and it's **GET** in every other case
(including `-G`/`--get`, which sends any `-d` data as the query string instead of
a body).

</details>

<details>
<summary>Which curl flags are recognised?</summary>

The common HTTP ones: `-X`/`--request`, `-H`/`--header`, `-d`/`--data`,
`--data-ascii`, `--data-raw`, `--data-binary`, `--data-urlencode`, `-F`/`--form`,
`-b`/`--cookie`, `-u`/`--user`, `-A`/`--user-agent`, `-e`/`--referer`, `--url`,
`-G`/`--get`, `-I`/`--head`, `-k`/`--insecure`, `-L`/`--location` and
`--compressed`. Other options that curl accepts (like `-s`, `-v`, `--fail`,
`--connect-timeout`) are recognised and listed under "Other flags" rather than
dropped, so nothing in your command goes unaccounted for.

</details>

<details>
<summary>Does it read the files referenced by @filename?</summary>

No. Arguments such as `-d @body.json` or `-F file=@photo.png` tell curl to read a
local file at run time, and that file isn't part of the command text you paste.
The tool reports each one as a **file reference** so you know a file is involved,
but it can't (and doesn't) read anything from your disk.

</details>

<details>
<summary>What does rebuild mode change about my command?</summary>

It doesn't change what the request does — it just normalises the presentation.
The command is re-emitted with each header on its own `-H` line, values wrapped
in single quotes only where a shell needs them, the method printed only when it
isn't GET, and the flags in a fixed order. It's handy for turning a one-line
command copied from DevTools into something readable, or for producing a
consistent form you can commit and diff.

</details>

<details>
<summary>Is a basic-auth password shown in plain text?</summary>

Yes — `-u user:password` is split into its username and password exactly as
written, because that's what the command already contains. Nothing is sent
anywhere (the whole tool runs in your browser), but bear in mind that a curl
command with `-u` embeds the credentials in plain text, so treat such commands as
secrets and avoid pasting them into tools you don't trust.

</details>
