## About this tool

Environment-variable references hide in shell scripts, Dockerfiles, batch files, CI YAML and application code. This scanner turns a pasted file into the exact set of variables it consumes, with line numbers, default values from parameter expansions and a defined/undefined status check.

Use it before writing a `.env.example`, moving a script into CI, reviewing a Dockerfile, or auditing code that reads `process.env`, `os.environ`, `System.getenv` or similar accessors. The scanner is deterministic and local: it runs in WebAssembly in your browser and never uploads your input.

### Worked example

Paste this shell fragment and choose **Output → Aligned table**:

```sh
PORT=${PORT:-8080}
curl "$API_URL/health"
echo "$PORT" # $COMMENTED_OUT
```

With comments skipped, the result is:

```text
VARIABLE  USES  LINES  DEFAULT  STATUS
API_URL   1     2               undefined
PORT      2     1, 3   8080     defined
```

To generate a starter `.env.example`, switch **Output** to `.env.example template`; the default from `${PORT:-8080}` is carried into the generated value.

## Limits and edge cases

- This is a reference scanner, not a full shell parser. Single-quoted strings and here-doc bodies are still scanned.
- Positional/special shell parameters such as `$1`, `$@`, `$?`, `$$` and `$#` are intentionally ignored.
- `\$VAR`, `$$` and batch `%%` escapes are skipped.
- Dockerfile `ENV` and `ARG`, shell assignments and Windows `set`/`setx` can count as definitions when that checkbox is enabled.
- A run is capped at 20,000 references; split extremely large repositories into files or folders.

## FAQ

<details>
<summary>Does this read my actual environment variables?</summary>

No. It scans only the text you paste and the optional “variables you already provide” list. It does not read your machine's environment, shell profile or secrets.

</details>

<details>
<summary>Why does it report variables inside single quotes or here-docs?</summary>

The tool is intentionally a deterministic text scanner rather than a full shell interpreter. It catches references in mixed config/code snippets, but that means some shell-only quoting rules are not modelled. Use the line numbers to review any borderline hit.

</details>

<details>
<summary>How do I get only the variables my deployment still needs?</summary>

Paste your script or config, paste your known `.env` values into “Variables you already provide”, and enable “Show only undefined variables”. The output then lists only variables not defined in the pasted file or your provided list.

</details>

<details>
<summary>Can it scan Dockerfiles and Windows batch files?</summary>

Yes. Use Auto-detect for most inputs, or set the syntax explicitly to Dockerfile or Windows batch. Dockerfile `ENV` and `ARG` lines and Windows `set`/`setx` assignments can count as definitions.

</details>
