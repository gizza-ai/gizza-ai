## About this tool

A **request file** is the plain-text format your editor's REST client uses — a
`.http` or `.rest` file in VS Code or IntelliJ, or an `.ain` template. It holds
one or more HTTP requests with `{{variables}}` standing in for the host, tokens
and IDs that change between environments. It is great inside the editor, and
useless the moment you need to hand the request to someone who doesn't use that
editor, drop it into a CI script, or paste it into a bug report.

This tool expands the file and prints the equivalent **curl** commands. Paste the
file, paste your variable values, and copy the result. Everything happens in your
browser — no file, URL, header or token is uploaded anywhere.

### What it understands

- **`.http` / `.rest` files** — a request line like `GET https://api.example.com/users HTTP/1.1`
  (the method is optional and defaults to `GET`, as does the `HTTP/1.1` suffix),
  header lines, a blank line, then the body.
- **Several requests in one file** — `###` separates them, and any text after the
  `###` becomes the request's name, as does a `# @name login` comment. Convert all
  of them at once, or pick one by name or 1-based index.
- **Comments** — lines starting with `#` or `//` are ignored.
- **File variables** — `@host = https://api.example.com` declares one, `{{host}}`
  uses it, and a variable may reference other variables recursively.
- **Wrapped query strings** — continuation lines starting with `?` or `&` are
  appended to the URL, so a long query can stay readable in the file.
- **Form bodies** — when the content type is `application/x-www-form-urlencoded`,
  body lines starting with `&` are joined back into one encoded string.
- **File bodies** — `< ./payload.json` and `<@ ./payload.json` become
  `--data-binary '@./payload.json'`, so curl reads the file at run time.
- **`.ain` templates** — `[Method]`, `[Host]`, `[Query]`, `[Headers]` and `[Body]`
  sections, with `$VAR` and `${VAR}` substitution. Detected automatically, or
  forced with the format selector.

### Supplying variables

The variables box takes whichever shape you already have:

- a **flat JSON object**: `{"host":"https://api.example.com","token":"abc"}`
- a **JSON document of named environments**:
  `{"dev":{...},"prod":{...}}` — the same shape as an `http-client.env.json`
  file. Put the name you want (`dev`, `prod`, …) in the environment field.
- **`KEY=VALUE` lines**, like a `.env` file. A leading `export `, `#` comments
  and surrounding quotes are stripped.

**System variables** are resolved from your browser's clock: `{{$guid}}` /
`{{$uuid}}`, `{{$timestamp}}` and `{{$datetime}}` (both accept an offset such as
`-1 d` or `2 h`, and `$datetime` accepts `iso8601`, `rfc1123` or a strftime
pattern), `{{$randomInt 1 100}}`, and `{{$processEnv NAME}}` / `{{$dotenv NAME}}`
which read from the variables you supplied.

Anything still missing is, by your choice, left in place as a `{{placeholder}}`
you can fill in by hand, replaced with an empty string, or reported as an error
listing every name it couldn't find.

### Shaping the output

Pick the **shell** the command has to run in and the quoting follows: `bash`
(single quotes, `\` continuations), `cmd` (double quotes, `^`) or `powershell`
(single quotes with doubled escapes, backtick continuations, and `curl.exe` so
the command doesn't hit PowerShell's `Invoke-WebRequest` alias). Choose **short**
flags (`-X`, `-H`, `-d`) or **long** ones (`--request`, `--header`, `--data-raw`),
wrap the command across lines or keep it on one, and add `-L`, `--compressed` or
`-k` as needed.

### Privacy

Everything runs locally in WebAssembly. Nothing is sent: this tool never performs
the request, it only writes the command. Your hosts, bearer tokens and request
bodies never leave your device, and there is no sign-up.

## FAQ

<details>
<summary>Which request-file format does this expect?</summary>

The `.http`/`.rest` format used by the VS Code REST Client extension and the
IntelliJ / JetBrains HTTP Client — they are the same format, so a file written
for one works here. `.ain` template files are supported too, and are detected
automatically by their `[Section]` headers. If auto-detection guesses wrong on an
unusual file, force the format with the selector.

</details>

<details>
<summary>How do I use my http-client.env.json environments?</summary>

Paste the whole JSON document into the variables box and put the environment's
name — `dev`, `prod`, or whatever you called it — in the environment field. If
the document contains more than one environment and you don't name one, the tool
stops and lists the names it found rather than guessing.

Private values from an `http-client.private.env.json` overlay aren't merged
automatically, because only one document is accepted. Copy the private values
into the pasted JSON (or into the `KEY=VALUE` box) yourself.

</details>

<details>
<summary>Can it read my real environment variables or a .env file on disk?</summary>

No, and that's deliberate: the tool runs sandboxed in your browser, with no
access to your machine. `{{$processEnv NAME}}` and `{{$dotenv NAME}}` still work
— they resolve against the variables you paste in, so you can keep the file
unchanged and supply the values here.

</details>

<details>
<summary>What happens to a variable I don't have a value for?</summary>

That's the "missing variables" setting. **Keep** (the default) leaves the
`{{placeholder}}` untouched in the generated command, which is handy when you
want to hand someone a template to fill in. **Empty** substitutes an empty
string. **Error** refuses to generate anything and lists every unresolved name,
which is what you want when a silently blank token would send a request you can't
explain.

</details>

<details>
<summary>Does it send the request or show me a response?</summary>

No. This tool only rewrites the file as command text — it never opens a
connection. Copy the command and run it yourself when you're ready.

</details>

<details>
<summary>Why is my body shown as --data-binary '@file' instead of its contents?</summary>

Because the body was a `< ./payload.json` file reference, and the file lives on
your disk, not in the pasted text. `--data-binary '@./payload.json'` tells curl
to read exactly that file when the command runs, which is the faithful
translation — run the command from the same directory as the request file and it
behaves identically.

</details>

<details>
<summary>Which shell should I pick?</summary>

Whichever one will run the command. Quoting rules differ enough that a bash
command pasted into `cmd.exe` frequently breaks on the first quote. Pick
**powershell** on Windows PowerShell or pwsh — it also emits `curl.exe` rather
than `curl`, because `curl` is an alias for `Invoke-WebRequest` there and doesn't
accept curl's flags.

</details>
