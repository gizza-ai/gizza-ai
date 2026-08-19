## About this tool

HashiCorp Configuration Language is pleasant to write by hand, but many scripts and review tools want JSON. This converter parses HCL2 — the syntax used by Terraform `.tf` and `.tfvars` files, plus Packer, Nomad, Vault, and Consul configs — and renders the equivalent JSON document locally in your browser.

Attributes become JSON properties. Blocks become nested objects keyed by the block type and then by each quoted label. If the same block header appears more than once, that spot becomes a JSON array. Expressions JSON cannot represent, such as `var.region`, `upper("x")`, or `var.enabled ? 1 : 0`, are preserved as Terraform-style `${...}` strings so they are visible instead of being dropped.

### Worked example

Input HCL:

```hcl
resource "aws_instance" "web" {
  ami           = "ami-0123456789"
  instance_type = "t3.micro"
  monitoring    = true
  tags = {
    Name = "web"
    Env  = "prod"
  }
}
```

Default JSON output:

```json
{
  "resource": {
    "aws_instance": {
      "web": {
        "ami": "ami-0123456789",
        "instance_type": "t3.micro",
        "monitoring": true,
        "tags": {
          "Name": "web",
          "Env": "prod"
        }
      }
    }
  }
}
```

### Options

- **Block shape** — `nested` keeps single block bodies as objects and uses arrays only for repeated block headers. `arrays` always wraps block bodies in arrays, which is convenient for scripts that want one stable shape.
- **Expression handling** — `template` preserves non-JSON expressions as `${...}` strings. `simplify` first evaluates constant expressions, so `1 + 2` becomes `3`, while unknown variables still remain as `${var.name}`.
- **Pretty print** and **indent** — choose human-readable output with two spaces, four spaces, or tabs, or turn pretty printing off for compact single-line JSON.
- **Sort keys** — alphabetize object keys at every level for cleaner diffs.

### Limits and edge cases

- Input is capped at **1,048,576 bytes** to keep browser and command-line runs predictable.
- Comments are parsed and then dropped because JSON has no comment syntax.
- HCL expressions are not evaluated against Terraform variables, providers, functions, or files. Unknown expressions are represented as strings instead.
- Duplicate attributes are rejected by the HCL parser; using the same name as both an attribute and a block produces a clear conflict error.
- The output is a structural JSON equivalent of one pasted file, not a Terraform plan, state file, provider schema expansion, or module loader.

## FAQ

<details>
<summary>Is this the same as running Terraform?</summary>

No. It parses one HCL document and maps its syntax to JSON. It does not initialize providers, load modules, resolve variables, evaluate data sources, or produce a plan. That keeps the tool fast and local, but it also means runtime-only Terraform values stay as expression strings.

</details>

<details>
<summary>Why do some values appear as `${...}` strings?</summary>

JSON cannot directly represent HCL expressions such as variable references, function calls, conditionals, or traversals. Preserving them as interpolation strings makes the output explicit and mirrors the convention used by Terraform's JSON configuration syntax. Set expression handling to **simplify** when you want constant arithmetic and boolean expressions folded first.

</details>

<details>
<summary>When should I choose the arrays block shape?</summary>

Use **arrays** when another program will consume the JSON and you want every block body to have the same container type even when there is only one block today. The default **nested** shape is more compact and only creates an array when the same block header repeats.

</details>

<details>
<summary>Can it convert `.tfvars` files?</summary>

Yes. A `.tfvars` file is just HCL attributes at the top level, so `region = "us-east-1"` becomes `{ "region": "us-east-1" }`. It also handles object and list values in variable files.

</details>

<details>
<summary>Does my configuration leave the browser?</summary>

No. The parser and JSON renderer run in WebAssembly inside the page. The same conversion code is also available through the command-line tool for local scripts.

</details>
