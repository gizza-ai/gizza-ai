## About this tool

YAML can parse successfully while still hiding production problems: duplicate keys where the last value silently wins, `yes` and `on` values that older parsers coerce to booleans, octal-looking file modes, tabs in indentation and spacing that fails a stricter CI linter. This tool checks those issues in one local pass and reports line-and-column findings you can act on before committing a config file.

Choose `relaxed` when you only want hard failures such as syntax errors, duplicate keys and invalid indentation. Use `default` for everyday review: line length, trailing whitespace, comment spacing, colon and hyphen spacing, truthy values and octal-looking scalars. Use `strict` when a repository expects document start markers, sorted keys and explicit non-empty values. The `disable` field accepts rule ids such as `truthy` or `line-length`, so you can keep a project-specific exception without turning off the rest of the lint.

### Worked example

Input:

```yaml
name: demo
name: duplicate
debug: yes
list:
  -   too many spaces
```

With the default preset, the report includes a duplicate-key error, a truthy-value warning and a hyphen-spacing warning. Turn on `strict_warnings` when you want warnings counted as errors for CI output, or choose `report_format=json` for a machine-readable summary.

### Limits and edge cases

- Maximum input size is 1 MiB and reports are capped at the first 500 findings.
- Multi-document streams separated by `---` are supported and counted.
- Block scalar bodies (`|` and `>`) are not style-linted as YAML, so scripts and literal text do not produce colon or truthy false positives.
- This is a syntax and style linter, not a schema validator for Kubernetes, Docker Compose or other application formats.
- Formatting and auto-fix are intentionally separate from linting; use a YAML formatter when you want rewritten output.

## FAQ

<details>
<summary>Can this find duplicate YAML keys?</summary>

Yes. Mapping keys are tracked per mapping scope, so a later `name:` in the same object is reported with the line where the first value appeared. Identical keys in different nested objects are allowed.

</details>

<details>
<summary>Why does it warn about values like `yes`, `on` or `0755`?</summary>

Some YAML 1.1 parsers treat unquoted words such as `yes`, `no`, `on` and `off` as booleans, and leading-zero numbers can be interpreted as octal. Quoting those values makes the intended string explicit across parsers.

</details>

<details>
<summary>What is the difference between relaxed, default and strict?</summary>

`relaxed` keeps the checks closest to correctness: syntax, duplicate keys and indentation. `default` adds common style and portability warnings. `strict` adds opinionated rules such as requiring `---`, alphabetical key ordering and warnings for empty values.

</details>

<details>
<summary>Can I turn off a noisy rule?</summary>

Yes. Add rule ids to the disabled-rules field, separated by commas, spaces or new lines. For example, `truthy, line-length` keeps duplicate-key and indentation checks while allowing those two style choices.

</details>
