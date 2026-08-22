# iam-policy-linter — competitor analysis (2026-08-22)

Scan run **before** implementation, per `/create-next-tool` step 4. Everything below is a
paraphrase of publicly documented behaviour — no competitor copy, branding, wording or
trademarked asset is reproduced or reused. Product names appear only to identify what was
studied.

## Scope of the tool

Backlog row: `iam-policy-linter` — "Validates AWS IAM policy JSON and flags overly-permissive
patterns like Action/Resource wildcards and NotAction." Type hint: `pure`.

## Competitors studied (top 3 + one secondary)

### 1. AWS IAM Access Analyzer policy validation (official, first-party)

Reference: the IAM User Guide "policy validation check reference".

- Emits four finding classes: **errors** (policy will not work / is rejected), **security
  warnings**, **general warnings**, and **suggestions**. Each finding has a stable check ID.
- Errors are overwhelmingly *grammar* checks: `MISSING_EFFECT`, `INVALID_EFFECT`,
  `MISSING_STATEMENT`, `MISSING_ACTION`, `MISSING_RESOURCE`, `MISSING_PRINCIPAL`,
  `UNSUPPORTED_PRINCIPAL`, `INVALID_VERSION`, `JSON_SYNTAX_ERROR`,
  `UNSUPPORTED_ELEMENT_COMBINATION` (Action+NotAction / Resource+NotResource /
  Principal+NotPrincipal in one statement), `INVALID_POLICY_ELEMENT`, `UNIQUE_SIDS_REQUIRED`,
  a large family of ARN-shape checks (`INVALID_ARN_PREFIX`, `INVALID_ARN_ACCOUNT`,
  `MISSING_ARN_FIELD`, `INVALID_PARTITION`, `INVALID_ARN_SERVICE_CASE`, …), a condition family
  (`INVALID_CONDITION_OPERATOR`, `INVALID_CONDITION_KEY_FORMAT`, `NULL_WITH_IF_EXISTS`,
  `TYPE_MISMATCH_IP_RANGE`, `DUPLICATE_KEYS_WITH_DIFFERENT_CASE`,
  `INVALID_CONDITION_MULTIPLE_BOOLEAN`), and a policy-variable family
  (`MISSING_BRACE_IN_VARIABLE`, `EMPTY_VARIABLE`, `UNSUPPORTED_SPACE_IN_VARIABLE`,
  `VARIABLE_UNSUPPORTED_IN_ELEMENT`).
- **Policy type is an explicit input.** The same JSON is validated differently as an
  identity policy, a resource policy, a role trust policy, an SCP or an RCP — e.g.
  `ROLE_TRUST_POLICY_SYNTAX_ERROR_RESOURCE` (no `Resource` in a trust policy),
  `ROLE_TRUST_POLICY_UNSUPPORTED_WILDCARD_IN_PRINCIPAL`, `SCP_SYNTAX_ERROR_PRINCIPAL`,
  `SCP_SYNTAX_ERROR_ACTION_WILDCARD` (wildcard only at the end of an action string),
  `RCP_SYNTAX_ERROR_NOTACTION`, and the per-type size quotas.
- Warnings include practical dead-configuration checks such as `PRIVATE_IP_ADDRESS`
  (`aws:SourceIp` never matches an RFC1918 address) and `PRIVATE_NOT_IP_ADDRESS`.
- UX: validation is inline in the console policy editor, findings grouped by class, each row
  links to a longer explanation and points at the offending statement.

### 2. Parliament (duo-labs) — the de-facto open-source IAM linting library

- Library + CLI. Finding IDs with severity tiers **LOW / MEDIUM / HIGH**; documented examples
  include `UNKNOWN_ACTION` (low), `RESOURCE_MISMATCH` (medium), `MALFORMED` (statement with
  unknown elements, or both `Principal` and `NotPrincipal`), and `RESOURCE_EFFECTIVELY_STAR`
  (a resource that is more than `*` textually but still matches everything).
- **Config-driven suppression**: a YAML override file can change a finding's severity or
  description, or ignore findings selectively by filepath / action / resource regex.
- CLI surface: `--file`, `--directory`, `--string '<json>'`, `--config <override>`,
  `--private_auditors`, `--include-community-auditors`, `--aws-managed-policies`,
  `--auth-details-file`, `--json`.
- Its deepest checks (`UNKNOWN_ACTION`, `RESOURCE_MISMATCH`) are backed by a bundled dump of
  the full AWS IAM service/action/resource-type catalogue.

### 3. Cloudsplaining (Salesforce)

- Classifies *what a policy lets you do*, not just whether it parses. Five risk categories:
  **privilege escalation**, **resource exposure** (permissions that rewrite resource policies,
  e.g. `s3:PutBucketPolicy`, `ecr:SetRepositoryPolicy`), **data exfiltration** (`s3:GetObject`,
  `ssm:GetParameter`, `secretsmanager:GetSecretValue`), **infrastructure modification**, and
  **credentials exposure**.
- The core heuristic: flag sensitive actions **that are not constrained by a resource ARN** —
  i.e. sensitive action + `Resource: "*"`. That pairing, not the action alone, is the finding.
- Output: a risk-prioritised HTML report plus a JSON results file; per-policy breakdown of the
  unrestricted actions.
- Exclusions via an `exclusions.yml` (policy names, principals, actions).

### Secondary: `iam-policy-lint` (PyPI)

A thin ergonomics wrapper — accepts JSON, YAML and policies embedded in other documents, and
delegates the actual analysis to Parliament. Confirms the packaging expectation (paste a
policy, get findings) rather than adding checks.

## Table stakes → decisions

| # | Table-stakes item (from the scan) | Fit | Decision |
|---|---|---|---|
| 1 | Severity-ranked findings with stable rule codes | in-model | **Built.** 30 rule codes, high/medium/low, stable strings. |
| 2 | Grammar/structural errors (missing/invalid `Effect`, `Action`, `Resource`, `Statement`, `Version`; unknown statement keys; duplicate `Sid`) | in-model | **Built** — `MISSING-EFFECT`, `INVALID-EFFECT`, `MISSING-ACTION`, `MISSING-RESOURCE`, `MISSING-STATEMENT`, `INVALID-VERSION`, `MISSING-VERSION`, `UNKNOWN-ELEMENT`, `DUPLICATE-SID`, `INVALID-SID`. |
| 3 | Mutually-exclusive element combinations | in-model | **Built** — `ELEMENT-CONFLICT`. |
| 4 | `Action: "*"` / `Resource: "*"` / full-admin detection | in-model | **Built** — `ACTION-STAR`, `RESOURCE-STAR`, `ADMIN-STAR`, `SERVICE-ACTION-STAR`, `RESOURCE-EFFECTIVELY-STAR`. |
| 5 | `NotAction` / `NotResource` / `NotPrincipal` with `Effect: Allow` | in-model | **Built** — `NOT-ACTION-ALLOW`, `NOT-RESOURCE-ALLOW`, `NOT-PRINCIPAL-ALLOW`. Deny-side use is deliberately *not* flagged; it is the correct idiom. |
| 6 | Public/anonymous principal on a resource or trust policy | in-model | **Built** — `PRINCIPAL-STAR` (downgraded when a `Condition` scopes it), `WILDCARD-PRINCIPAL-KEY`. |
| 7 | `iam:PassRole` with an unconstrained resource | in-model | **Built** — `PASS-ROLE-STAR`. |
| 8 | Privilege-escalation / credentials-exposure / resource-exposure / data-exfiltration action families on `Resource: "*"` (Cloudsplaining's core idea) | in-model | **Built** — `PRIV-ESC`, `CREDENTIAL-EXPOSURE`, `RESOURCE-EXPOSURE`, `DATA-EXFIL`, each with a curated, documented action list. |
| 9 | Policy-type awareness (identity / resource / trust / SCP) | in-model | **Built** — `policy_type` param drives principal requirements, `Resource`-in-trust-policy, and SCP-specific rules (`SCP-PRINCIPAL`, `SCP-ACTION-WILDCARD`, `SCP-EFFECT`). |
| 10 | Condition-block validation (operator, key format, `Null` + `IfExists`, CIDR shape) | in-model | **Built** — `INVALID-CONDITION-OPERATOR`, `INVALID-CONDITION-KEY`, `NULL-IF-EXISTS`, `INVALID-CIDR`, `PRIVATE-SOURCE-IP`, `EMPTY-CONDITION`. |
| 11 | ARN shape checking (prefix, field count, partition, account digits, lowercase service) | in-model | **Built** — `INVALID-ARN`. Depth stops at grammar; see out-of-model #1. |
| 12 | Policy-variable syntax checks | in-model | **Built** — `INVALID-VARIABLE`. |
| 13 | Size-quota check | in-model | **Built** — `POLICY-SIZE` against the 6,144-character managed-policy quota (whitespace excluded, as AWS measures it). |
| 14 | Machine-readable output for CI | in-model | **Built** — `format = json` returns `{ verdict, summary, findings[] }`; `format = csv` for tickets/spreadsheets. |
| 15 | Suppression of reviewed findings | in-model | **Built** — `ignore` takes rule codes; unknown codes are an error, not a silent no-op. |
| 16 | Severity threshold for display | in-model | **Built** — `min_severity`; a display filter only, the verdict always counts every finding. |
| 17 | Worked example / preset policies to click | in-model | **Built** — five `[[example]]` preset chips (admin star, NotAction allow, public bucket policy, trust policy, least-privilege clean). |
| 18 | Location pointer for each finding | in-model | **Built** — every finding carries the **source line** of its statement plus a JSON path (`Statement[1].Action[0]`). Parliament reports no line; Access Analyzer only highlights in its own editor. This is our differentiator. |

### Out-of-model (listed, deliberately not built)

1. **Full AWS action/service/resource-type catalogue** (Parliament's `UNKNOWN_ACTION` and
   `RESOURCE_MISMATCH`; Access Analyzer's `INVALID_ACTION`, `INVALID_SERVICE`,
   `UNSUPPORTED_ACTION_IN_POLICY`, and every service-specific ARN rule). The catalogue is
   megabytes and changes weekly; embedding a snapshot in a browser wasm bundle would be both
   heavy and quietly stale, and a stale allowlist produces confident false "unknown action"
   errors. We validate action *grammar* (`service:Action`, wildcard placement) and say plainly
   on the page that action existence is not checked.
2. **Account-wide / organisation-wide scanning** (Cloudsplaining's `download` + account crawl,
   `aws-lint-iam-policies`, Access Analyzer's external-access findings). Needs AWS credentials
   and a backend; gizza tools are browser-local and credential-free.
3. **CloudFormation / Terraform template extraction** (`cfn-policy-validator`). Out of scope
   for a single-policy linter; the user can paste the policy body.
4. **HTML report artefacts and account dashboards** (Cloudsplaining). The page renders the
   ranked report directly; JSON/CSV cover the export need.
5. **YAML input and policies embedded in other documents** (`iam-policy-lint`). Considered and
   rejected for now: it doubles the parse surface for a format AWS itself does not accept for
   policy bodies. JSON only, stated on the page.
6. **Config-file-driven severity overrides** (Parliament's YAML override). The `ignore`
   parameter covers the suppression half; per-rule severity rewriting is configuration state a
   stateless single-shot tool cannot carry.

### UX control patterns adopted

- **Policy type as a first-class select**, mirroring Access Analyzer's policy-type-aware
  validation — the single biggest correctness lever, and cheap.
- **Severity tiers named high/medium/low**, matching Parliament so findings are comparable.
- **"Sensitive action + unconstrained resource" as the finding**, not the action alone —
  Cloudsplaining's insight, and what keeps the report from screaming about every `s3:GetObject`.
- **Preset chips** instead of a wall of instructions: five one-click policies covering the
  failure modes people arrive with.
- **A verdict line first** (`unsafe` / `review` / `clean`), so the answer is readable before
  the findings are.
