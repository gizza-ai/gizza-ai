# curl-command-parser — competitor analysis (2026-07-11)

Function: parse a `curl` command line into a structured HTTP request (method, URL,
query params, headers, body, auth, cookies, flags) and rebuild a clean canonical
`curl` command from the same input. Pure-Rust, runs locally.

## Competitors scanned (top 3)

1. **Toolinix cURL Parser** — https://toolinix.com/tools/curl-parser
2. **Ez Parser cURL Parser** — https://ezparser.com/curl-parser
3. **tyrchen/curl-parser (Rust crate)** — https://github.com/tyrchen/curl-parser

(Also noted: curlconverter.com, TryAPI cURL Parser — same shape.)

## Table-stakes feature matrix

| Capability | Toolinix | Ez Parser | tyrchen crate | in gizza | fit |
|---|---|---|---|---|---|
| `-X`/`--request` method | ✅ | ✅ | ✅ | ✅ | in-model |
| `-H`/`--header` headers | ✅ | ✅ | ✅ | ✅ | in-model |
| `-d`/`--data`/`--data-ascii` body | ✅ | ✅ | ✅ | ✅ | in-model |
| `--data-raw` / `--data-binary` | ✅ | ✅ | ➖ | ✅ | in-model |
| `--data-urlencode` | ✅ | ➖ | ➖ | ✅ | in-model |
| `-b`/`--cookie` cookies | ✅ | ✅ | ➖ | ✅ | in-model |
| `-u`/`--user` basic auth (decode user/pass) | ✅ | ✅ | ✅ | ✅ | in-model |
| `-k`/`--insecure` | ✅ | ✅ | ✅ | ✅ | in-model |
| `-L`/`--location` follow redirects | ✅ | ✅ | ✅ | ✅ | in-model |
| `--compressed` | ✅ | ✅ | ➖ | ✅ | in-model |
| `-I`/`--head` (HEAD) | ✅ | ➖ | ➖ | ✅ | in-model |
| `-G`/`--get` (data→query) | ➖ | ➖ | ➖ | ✅ | in-model |
| `--url` explicit URL | ➖ | ➖ | ➖ | ✅ | in-model |
| `-F`/`--form` form fields | ✅ | ✅ | ➖ | ✅ | in-model |
| `-A`/`--user-agent`, `-e`/`--referer` | ✅ | ➖ | ➖ | ✅ | in-model |
| Query params split out of URL | ✅ | ✅ (implied) | ➖ | ✅ | in-model |
| Infer POST when body present | ✅ | ✅ | ✅ | ✅ | in-model |
| Shell tokenizer (single/double quotes, `\` line-continuation) | ✅ | ✅ | ✅ (pest) | ✅ | in-model |
| Rebuild a clean, canonical curl command | ➖ | ➖ | ➖ | ✅ | in-model |
| Effective Content-Type inference | ✅ | ✅ | ➖ | ✅ | in-model |
| Copy JSON / copy sections | ✅ | ✅ | n/a | ➖ (page copy button) | in-model (page) |
| **Generate fetch()/axios/Python/Ruby code** | ✅ | ✅ | ➖ (reqwest) | ❌ | **out-of-scope** |

## Decisions

- **Two modes** (`parse` default, `rebuild`), single input field `command`, mirroring the
  dual-direction pattern of `magnet-link-parser`. `parse` → structured JSON (chat/CLI) or an
  aligned human view (page); `rebuild` → a clean multi-line canonical `curl` command with `\`
  continuations, one `-H` per line, values shell-quoted.
- **Robust shell tokenizer**: POSIX single quotes (literal), double quotes (with `\"`/`\\`/`\$`/
  backtick/newline escapes), backslash escapes outside quotes, and `\`+newline line
  continuations. Strips a leading `curl`. Handles attached short-flag values (`-XPOST`,
  `-d@file`) and `--flag=value`.
- **Every table-stakes flag lands in the descriptor's parsing** (see matrix). `--data-urlencode`
  percent-encodes; `@file` data/form values are surfaced as a `body_file`/file reference rather
  than silently dropped (we can't read local files in the browser/sandbox).
- **OUT OF SCOPE (not built): multi-language code generation** (fetch/axios/Python/Ruby). That is
  a code-generator, a materially different tool; our remit is the structured request + a clean
  rebuilt curl command. Listed here, not implemented. (Never copy competitor copy/branding.)

## UX

- `[[example]]` preset chips prefill common commands (GET with headers, POST JSON, basic-auth,
  form upload) since competitors ship "Load Sample" buttons.
- `mode` renders as a `<select>`; `command` is a multiline textarea.
