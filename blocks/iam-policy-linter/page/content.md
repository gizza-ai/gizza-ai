## About this tool

IAM Policy Linter checks one AWS IAM policy JSON document in your browser. It is designed for quick reviews before a policy lands in Terraform, CloudFormation, a ticket or a pull request: paste the document, choose how AWS will attach it, and get a severity-ranked report with stable rule codes, JSON paths and source lines.

The linter covers structural policy grammar, policy-type differences, dangerous wildcard grants, `Allow` statements that use `NotAction`, `NotResource` or `NotPrincipal`, public principals, `iam:PassRole` on unconstrained resources, and curated sensitive action families such as credential exposure, data exfiltration, resource exposure and privilege escalation when they are paired with `Resource: "*"`.

### Worked example

Paste this identity policy:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "*",
      "Resource": "*"
    }
  ]
}
```

The report starts with an `UNSAFE` verdict and an `ADMIN-STAR` finding at `$.Statement[0].Action`, with the source line that contains the wildcard action. Switch **Output as** to JSON when you want a CI-friendly object with `verdict`, `summary` and `findings`, or CSV when you need rows for a spreadsheet or a review ticket.

### Limits and edge cases

- JSON only. AWS IAM policy bodies are JSON; YAML templates and embedded CloudFormation/Terraform extraction are outside this single-policy tool.
- The tool is catalogue-free: it validates action grammar such as `service:Action`, but it does not embed the full AWS service/action/resource catalogue and will not claim an action name is unknown.
- It does not call AWS, inspect accounts, load managed policies or need credentials. Everything runs locally in the browser.
- The managed-policy size check uses the 6,144-character quota after whitespace is removed. The input hard cap is 200,000 characters to keep browser runs responsive.
- `min_severity` hides lower-severity rows from the rendered output only. The verdict still counts every non-ignored finding.
- `ignore` is explicit and strict: unknown rule codes are errors, so a typo cannot silently suppress nothing.

## FAQ

<details>
<summary>Is this the same as AWS IAM Access Analyzer?</summary>

No. It mirrors the most useful local checks — policy structure, policy-type rules, wildcards, public principals and common risky permission patterns — but it does not call AWS or use AWS's full private validation catalogue. Use AWS IAM Access Analyzer for authoritative service-specific validation before deployment.

</details>

<details>
<summary>Why does the linter not flag an unknown action like <code>s3:GetObjectt</code>?</summary>

A complete AWS action and resource-type catalogue is large and changes frequently. Embedding a stale snapshot in a browser tool would create confident false positives and false negatives. This linter checks action grammar and high-risk patterns; service-specific action existence remains an AWS-side validation step.

</details>

<details>
<summary>When should I use each policy type?</summary>

Use **identity** for policies attached to users, groups or roles. Use **resource** for bucket, queue, key, repository and similar resource policies where `Principal` is expected. Use **trust** for a role's `AssumeRolePolicyDocument`. Use **scp** for AWS Organizations service control policies, where several identity-policy elements have different rules.

</details>

<details>
<summary>Does ignoring a finding change the verdict?</summary>

Yes. `ignore` represents an explicit reviewed suppression, so ignored codes are removed before the verdict is computed. By contrast, `min_severity` is only a display filter and never changes the verdict.

</details>
