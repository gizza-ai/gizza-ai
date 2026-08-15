# hcl-to-json — competitor analysis (2026-08-15)

Scan run before completing the implementation, per `create-next-tool` step 4. Findings are paraphrased from public documentation and tool pages; no competitor copy, branding, or trademarks are reused.

Backlog row: `hcl-to-json` — "Converts HashiCorp HCL (Terraform-style) configuration into equivalent JSON." (type hint: pure)

## Duplicate check

Checked `ls blocks | grep -Ei 'hcl|terraform|tf|json|config'` and `docs/tool-skiplist.txt` for HCL/Terraform rows. The closest existing tools are generic JSON formatters/converters (`json-yaml-convert`, `json-beautify`, `json-repair`, `config-merge`) and infrastructure text tools (`wireguard-config-builder`, `network-config-parser`). None parse HCL2 blocks, labels, expressions, heredocs, or Terraform-style JSON configuration shapes. Proceeding with a new tool.

## Competitors reviewed

1. Terraform's JSON configuration syntax documentation — canonical mapping between native HCL blocks/arguments and JSON objects.
2. Terraform/HCL language expression documentation — identifiers, traversals, functions, heredocs, lists, and objects that must survive conversion.
3. `hcl2json` command-line converters — table-stakes behaviour for stdin/file conversion, pretty JSON, repeated blocks, and syntax errors.
4. HCL parser library examples (`hclparse` / HCL2 parser docs) — how robust converters expose parse diagnostics and preserve body structure.
5. Online HCL-to-JSON converters — UX expectations: paste input, immediate JSON output, copy-ready formatting, examples, and simple options.

## Table stakes → decisions

| Capability | Seen in | Fit | Decision |
| --- | --- | --- | --- |
| Parse HCL2 / Terraform `.tf` and `.tfvars` text | all | in-model | Use the pure-Rust `hcl-rs` parser in the core crate. |
| Attributes become JSON properties | Terraform JSON syntax, CLI tools | in-model | Implemented directly from `Body` attributes. |
| Blocks and quoted labels become nested JSON objects | Terraform JSON syntax | in-model | `resource "aws_instance" "web"` maps to `resource.aws_instance.web`. |
| Repeated block headers become arrays | Terraform JSON syntax, CLI tools | in-model | Default `blocks=nested` arrays repeated headers; `blocks=arrays` always arrays for stable scripting. |
| Preserve strings, numbers, bools, arrays, objects, heredocs | parser docs, online tools | in-model | `hcl-rs` values serialize through `serde_json`. |
| Preserve non-JSON expressions | Terraform JSON syntax | in-model | Unknown expressions render as Terraform-style `${...}` interpolation strings. |
| Constant expression folding | advanced parser examples | in-model | Optional `expressions=simplify` evaluates constant sub-expressions while preserving unknown parts. |
| Pretty / compact output | CLI and online tools | in-model | `pretty` plus `indent=2|4|tab`. |
| Deterministic key ordering | CLI tools and diff workflows | in-model | Optional recursive `sort_keys`. |
| Parse errors with line context | parser docs, CLI tools | in-model | Surface parser error text from `hcl-rs`. |
| Terraform plan/state/module/provider evaluation | Terraform CLI | out-of-model | Requires Terraform runtime, providers, filesystem/module graph, and external state. This tool is a syntax-to-JSON converter only. |
| Multi-file merge and variable resolution | Terraform workflows | out-of-model | Requires project-level context and evaluation order across files. Users can paste one file at a time. |
| Remote file upload / URL fetching | some online tools | out-of-model for this pure tool | Gizza page takes pasted text only; CLI can pipe files locally. |

Every table-stake item above lands in the descriptor/page or in the out-of-model list; none was dropped silently.

## Resulting descriptor

`hcl` (required multiline), `blocks` (`nested|arrays`), `expressions` (`template|simplify`), `sort_keys` (boolean), `pretty` (boolean), and `indent` (`2|4|tab`).

## Notes

- Page copy stays generic and brand-free.
- No competitor wording, trademarks, or examples were copied.
- The tool intentionally documents that it is not a Terraform evaluator or planner.
