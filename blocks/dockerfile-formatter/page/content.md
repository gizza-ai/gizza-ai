## About this tool

Dockerfiles are compact, but formatting drift makes reviews harder: one stage might use lowercase
`from`, another has two-space continuations, comments are glued to `#`, and multi-stage builds run
together with no visual break. This formatter normalizes the layout while leaving instruction
arguments alone, so it is safe for package-manager commands, JSON-array forms and shell snippets.

**Worked example.** Input:

```dockerfile
from alpine:3.20 as build
run apk add --no-cache curl \
  && curl --version
#runtime
from alpine
copy --from=build /usr/bin/curl /usr/bin/curl
```

with the defaults becomes:

```dockerfile
FROM alpine:3.20 AS build
RUN apk add --no-cache curl \
    && curl --version

# runtime
FROM alpine
COPY --from=build /usr/bin/curl /usr/bin/curl
```

The defaults match common Docker style: uppercase instruction keywords, four spaces before
continuation lines, one blank line between build stages, and a single space after ordinary comment
markers.

**What it deliberately does not rewrite.** It does not reorder instructions, sort packages, parse shell
commands inside `RUN`, or change the arguments after an instruction keyword. Top-of-file parser
directives such as `# syntax=...`, `# escape=...` and `# check=...` are preserved. Heredoc bodies are
copied byte for byte because indentation inside them can be part of a generated file or script.

**Limits and edge cases.** `indent` accepts 0–8 spaces and `max_blank_lines` accepts 0–5. Unknown
instruction names, dangling line continuations and unterminated heredocs return line-numbered errors
instead of partial output. The formatter recognizes both `\` and a backtick continuation character
when a valid `# escape=` directive appears at the top of the file.

## FAQ

<details>
<summary>Is this a linter?</summary>

No. It formats layout and reports syntax-shaped mistakes that would make formatting unsafe, such as
an unknown instruction or a dangling continuation. It does not enforce best-practice rules like
pinning versions, using non-root users or avoiding `latest` tags.

</details>

<details>
<summary>Will it change my RUN command?</summary>

It changes the Dockerfile instruction keyword and leading continuation indentation only. The shell
text after `RUN` is not parsed, sorted or reflowed, so package names, flags and quoting stay in the
same order you pasted.

</details>

<details>
<summary>Why are heredoc bodies preserved exactly?</summary>

A heredoc often writes a config file or shell script, and spaces inside that body can be meaningful.
The formatter can clean the `RUN <<EOF` instruction line, but it copies every following heredoc body
line through until the delimiter.

</details>

<details>
<summary>What does the escape directive change?</summary>

Dockerfiles normally use `\` for line continuations. Windows Dockerfiles sometimes begin with
`# escape=`` and then use a backtick. When that directive is present at the top, this formatter uses
the backtick as the continuation marker and leaves the directive itself untouched.

</details>

<details>
<summary>Why is there an option to preserve instruction casing?</summary>

Some teams intentionally use lowercase Dockerfiles or keep generated files exactly as their source
system emits them. Pick `preserve` when you only want indentation, blank-line and comment cleanup
without changing keyword case.

</details>
