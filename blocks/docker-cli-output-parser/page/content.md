## About this tool

Docker already has `--format '{{json .}}'`, but that only helps when you can rerun the command. This parser is for the other common case: a pasted terminal transcript, a CI log, a ticket comment, or a saved `docker ps` / `docker images` / `docker stats --no-stream` table that needs to become structured data.

The tool uses Docker's header line as a fixed-width ruler. That matters because many Docker columns contain spaces in both the header and the value: `CONTAINER ID`, `IMAGE ID`, `MEM USAGE / LIMIT`, `NET I/O`, `BLOCK I/O`, `COMMAND`, `CREATED`, `STATUS`, and `PORTS`. A plain whitespace split mangles those rows; this parser keeps each cell in the column Docker printed.

With typed parsing enabled, the JSON output goes beyond raw strings:

- percentages such as `CPU %` and `MEM %` become numbers;
- `PIDS` becomes an integer;
- `NAMES` and `PORTS` become arrays;
- sizes gain byte counts;
- `MEM USAGE / LIMIT`, `NET I/O`, and `BLOCK I/O` split into separate usage/limit/input/output fields;
- Docker's `--` placeholders become `null`.

Output can be JSON, CSV, TSV, Markdown, or an aligned text table. Choose snake_case keys for scripts, Docker-style keys for comparison with `--format`, or the original printed headers for spreadsheets.

### Worked example

Input from `docker ps`:

```text
CONTAINER ID   IMAGE          COMMAND                  CREATED         STATUS         PORTS                    NAMES
9f21a1b2c3d4   nginx:1.25     "/docker-entrypoint.…"   3 minutes ago   Up 3 minutes   0.0.0.0:8080->80/tcp     web
0011deadbeef   postgres:16    "docker-entrypoint.s…"   2 hours ago     Up 2 hours                              db
```

JSON output starts like:

```json
[
  {
    "container_id": "9f21a1b2c3d4",
    "image": "nginx:1.25",
    "command": "/docker-entrypoint.…",
    "created": "3 minutes ago",
    "status": "Up 3 minutes",
    "ports": ["0.0.0.0:8080->80/tcp"],
    "names": ["web"]
  }
]
```

Switch to CSV when you are pasting into a spreadsheet, Markdown for an incident report, or a plain table when you only want selected columns aligned for a chat reply.

### Limits and edge cases

- The input must include a header row. Without it, there is no reliable way to know where columns begin or what schema the output should use.
- Docker version and flags change headers. Unknown columns are preserved and normalized; derived typed fields are added only for known Docker shapes.
- `strict` mode is useful for CI logs: it fails on a header/kind mismatch or a row that looks truncated. Leave it off for messy ticket snippets.
- Fixed-width parsing follows the header spans Docker printed. If a terminal wrapped a long row onto multiple lines, unwrap it before parsing.
- This does not connect to Docker, inspect containers, or read a Docker socket. It only parses pasted text locally in your browser.

## FAQ

<details>
<summary>Why not just use docker's own JSON output?</summary>

Use Docker's JSON output when you can rerun the command. This tool exists for saved output: copied terminal tables, CI logs, incident reports, and Slack snippets where the original command is gone or the host is no longer available. It turns that pasted table back into data.

</details>

<details>
<summary>Why does the header line matter?</summary>

Docker headers are the schema and the ruler. `MEM USAGE / LIMIT` is one column, not four, and `COMMAND` or `STATUS` values often contain spaces. The parser uses the header positions to slice each row, so it can keep values with spaces intact.

</details>

<details>
<summary>What does typed parsing change?</summary>

Typed parsing converts obvious Docker values into machine-friendly forms: percentages become numbers, byte sizes gain `*_bytes` fields, `NAMES` and `PORTS` become arrays, `PIDS` becomes an integer, and composite stats columns split into input/output or usage/limit pairs. Turn it off when you need every cell exactly as Docker printed it.

</details>

<details>
<summary>How do I parse a custom docker --format table?</summary>

Include a `table ...` header in the command output. Tab-separated custom tables are supported, and columns are normalized the same way as built-in Docker headers. If you print values without a header, use Docker's native JSON format instead because there is no column schema left for this parser to infer.

</details>

<details>
<summary>Can this read live containers?</summary>

No. It never connects to Docker and never uploads your text. The browser page and the CLI both run the same local parser over the text you provide. That keeps it safe for logs copied from production hosts.

</details>
