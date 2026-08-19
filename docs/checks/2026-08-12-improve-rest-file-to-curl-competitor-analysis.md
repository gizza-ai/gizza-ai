# rest-file-to-curl — competitor analysis (2026-08-12)

Scope: tools that turn a `.http` / `.rest` / `.ain` request file (with `{{variables}}` and an
environment) into runnable `curl` commands. Scan run BEFORE implementation; all notes are
paraphrased observations of publicly documented behaviour — no competitor copy, branding, or
trademarks are reproduced in the tool or its page.

## Competitors reviewed

1. **VS Code REST Client extension** (`Huachao/vscode-restclient`) — the de-facto definition of the
   `.http`/`.rest` file format, and the source of the "copy request as cURL" workflow. Documented
   behaviour: request line is `[METHOD] URL [HTTP/1.1]` with `GET` as the default method; headers
   run until the first blank line; the body is everything after it; `###` separates requests in one
   file; `#` and `//` start comments; `# @name <id>` names a request; `@name = value` declares a
   file variable; `{{name}}` references a variable; query strings may continue on following lines
   starting with `?`/`&`; `application/x-www-form-urlencoded` bodies may continue on lines starting
   with `&`; `< path` / `<@ path` reads the body from a file; system variables include `{{$guid}}`,
   `{{$timestamp [offset unit]}}`, `{{$datetime <format> [offset unit]}}`, `{{$randomInt min max}}`,
   `{{$processEnv NAME}}`, `{{$dotenv NAME}}`.
2. **JetBrains HTTP Client** (IntelliJ IDEA `.http` files + `ijhttp` CLI) — same file format with a
   different variable story: environments live in `http-client.env.json` (a JSON object of NAMED
   environments, e.g. `development` / `production`), overridden by `http-client.private.env.json`;
   the CLI selects one with `--env-file` + `--env <name>`. In-place `@name = value` variables are
   file-scoped; dynamic variables include `$uuid`, `$timestamp`, `$randomInt`.
3. **curlconverter** (curlconverter.com, incl. its `/http/` page) — the widely used converter in the
   opposite direction (curl ⇄ HTTP/40+ languages). Relevant here for the *output* conventions users
   expect: everything runs client-side with no upload, the target "language"/flavour is a single
   dropdown, and generated commands quote the URL, put one `-H` per header, and use an explicit
   data flag for the body.

## Table stakes → decision

| Capability | Seen in | In model? | Decision |
|---|---|---|---|
| `.http`/`.rest` request-line parsing, default `GET`, optional `HTTP/1.1` suffix | 1, 2 | yes | built |
| Headers until blank line, body after | 1, 2 | yes | built |
| `###` multi-request files | 1, 2 | yes | built; `request` param selects one by index or name, default all |
| `#` / `//` comments, `# @name id` | 1, 2 | yes | built (separator title is a fallback name) |
| `@name = value` file variables, `{{name}}` references (nested, recursive) | 1, 2 | yes | built |
| Environment supplied as JSON | 2 | yes | built — `env` accepts a flat JSON object |
| NAMED environments in one JSON file (`{"dev":{…},"prod":{…}}`) + selector | 2 | yes | built — `env` + `environment` params |
| `.env` / `KEY=VALUE` style variables | 1 (`$dotenv`) | yes | built — `env` also accepts `KEY=VALUE` lines (`export ` and quotes stripped) |
| `{{$processEnv X}}` / `{{$dotenv X}}` | 1 | partly | built, resolved from the supplied `env` map (a browser/CLI tool has no access to the caller's process environment — documented on the page) |
| Dynamic `{{$guid}}`/`{{$uuid}}`, `{{$timestamp [off unit]}}`, `{{$datetime fmt [off unit]}}`, `{{$randomInt a b}}` | 1, 2 | yes | built (deterministic per run from the supplied clock) |
| Multi-line query continuation (`?`/`&` lines) | 1 | yes | built |
| Form-body continuation lines starting with `&` | 1 | yes | built (when the content type is `application/x-www-form-urlencoded`) |
| `< file` / `<@ file` body references | 1 | yes | built — mapped to `curl --data-binary '@file'` (we cannot read local files, but curl can) |
| Copy request as cURL | 1 | yes | the whole point of this tool |
| Shell flavour of the generated command (bash vs Windows cmd vs PowerShell) | 3 (and every "copy as curl" UI) | yes | built — `shell` param drives quoting + line-continuation character; PowerShell emits `curl.exe` so it doesn't hit the `Invoke-WebRequest` alias |
| Short vs long flags (`-H` vs `--header`) | 3 | yes | built — `flag_style` |
| One-line vs backslash-continued output | 3 | yes | built — `multiline` |
| Common extra flags (`-L`, `--compressed`, `-k`) | 3 | yes | built as booleans |
| Client-side only, nothing uploaded | 3 | yes | inherent — runs as wasm in the browser / locally in the CLI |
| Preset examples for one-click trials | 3 | yes | built — `[[example]]` chips on the page |
| `.ain` template files (`[Host] [Headers] [Query] [Body]` sections, `$VAR` substitution) | (`ain` CLI, named in the backlog description) | yes | built — `format = auto` sniffs it, `format = ain` forces it |

## Out of model (listed, not built)

- **Request chaining** (`{{login.response.body.$.token}}`): needs a response from a previously
  *executed* request; this tool never sends anything.
- **Pre-request / response JavaScript** (`{% … %}` scripts, `client.global.set`): would require a JS
  engine inside the block.
- **Reading the caller's real process environment or a `.env` file from disk**: the block is
  sandboxed; the same values can be pasted into `env`.
- **`http-client.private.env.json` overlay resolution**: only one env document is accepted; merge
  the private values into it yourself (documented in the FAQ).
- **Actually sending the request / showing a response**: that is the existing `http-request` tool.
- **Converting `curl` back into a request file**: covered by the existing `curl-command-parser`
  block (`mode=rebuild`), so it is deliberately not duplicated here.

## Neighbouring gizza blocks checked for duplication

- `curl-command-parser` — curl → structured request (opposite direction). Not a duplicate.
- `http-request-builder` — params → a raw HTTP/1.1 message, not a curl command, no file/variables.
- `http-request` — actually performs a request.
- `parse-http-message`, `har-request-extract` — different input formats.

No overlap: this is the only block that reads a `.http`/`.rest`/`.ain` file with variables.
