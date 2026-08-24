//! gizza-ai/iam-policy-linter core — validate an AWS IAM policy document and report
//! grammar errors plus overly-permissive patterns.
//!
//! Nothing here touches wafer, wasm-bindgen, WASI or the network, so the same code runs
//! in the chat block, the `gizza` CLI, and the browser page.
//!
//! The linter is deliberately *catalogue-free*: it never claims to know whether
//! `s3:GetObjectt` is a real action, because a bundled snapshot of the AWS action
//! catalogue would be megabytes and stale within a week. It checks action **grammar**
//! (`service:Action`, wildcard placement), statement structure, policy-type-specific
//! rules, condition-block sanity, ARN shape, policy-variable syntax, and a curated set
//! of sensitive action families that are dangerous specifically when paired with an
//! unconstrained `Resource`.
//!
//! Every finding carries a stable rule code, a severity (`high`/`medium`/`low`), a
//! JSONPath into the document (`$.Statement[1].Action[0]`) and — when the raw text can
//! be located — the source line.

use serde_json::{Map, Value};
use std::collections::HashMap;

/// AWS managed-policy character quota (whitespace excluded, as AWS measures it).
pub const MAX_POLICY_CHARS: usize = 6144;

/// Hard cap on the input document. Larger inputs are rejected rather than linted.
pub const MAX_INPUT_CHARS: usize = 200_000;

/// Hard cap on how many findings a single report may carry.
pub const MAX_FINDINGS: usize = 500;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Finding severity, ordered so `High > Medium > Low`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Severity {
    Low,
    Medium,
    High,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
        }
    }
    pub fn parse(s: &str) -> Option<Severity> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Severity::Low),
            "medium" | "med" => Some(Severity::Medium),
            "high" => Some(Severity::High),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Policy type
// ---------------------------------------------------------------------------

/// Which kind of policy document is being linted. AWS validates the same JSON
/// differently depending on where it is attached.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PolicyType {
    /// Attached to a user, group or role (`Principal` is not allowed).
    Identity,
    /// Attached to a resource — bucket policy, queue policy, key policy
    /// (`Principal` is required).
    Resource,
    /// A role's `AssumeRolePolicyDocument` (`Principal` required, `Resource` not allowed).
    Trust,
    /// An Organizations service control policy (`Principal` not allowed).
    Scp,
}

impl PolicyType {
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyType::Identity => "identity",
            PolicyType::Resource => "resource",
            PolicyType::Trust => "trust",
            PolicyType::Scp => "scp",
        }
    }
    pub fn parse(s: &str) -> Option<PolicyType> {
        match s.trim().to_ascii_lowercase().as_str() {
            "identity" => Some(PolicyType::Identity),
            "resource" => Some(PolicyType::Resource),
            "trust" => Some(PolicyType::Trust),
            "scp" => Some(PolicyType::Scp),
            _ => None,
        }
    }
    /// Whether a `Principal` element belongs in this policy type.
    fn wants_principal(self) -> bool {
        matches!(self, PolicyType::Resource | PolicyType::Trust)
    }
}

// ---------------------------------------------------------------------------
// Rule codes
// ---------------------------------------------------------------------------

/// Every rule code the linter can emit. `ignore` is validated against this list, so an
/// unknown code is an error rather than a silent no-op.
pub const RULE_CODES: &[&str] = &[
    // structure / grammar
    "MISSING-STATEMENT",
    "INVALID-STATEMENT",
    "MISSING-VERSION",
    "LEGACY-VERSION",
    "INVALID-VERSION",
    "MISSING-EFFECT",
    "INVALID-EFFECT",
    "MISSING-ACTION",
    "MISSING-RESOURCE",
    "MISSING-PRINCIPAL",
    "UNKNOWN-ELEMENT",
    "DUPLICATE-SID",
    "INVALID-SID",
    "ELEMENT-CONFLICT",
    "INVALID-ACTION",
    "INVALID-ARN",
    "INVALID-PRINCIPAL",
    "INVALID-VARIABLE",
    "POLICY-SIZE",
    // permissiveness
    "ADMIN-STAR",
    "ACTION-STAR",
    "RESOURCE-STAR",
    "SERVICE-ACTION-STAR",
    "RESOURCE-EFFECTIVELY-STAR",
    "NOT-ACTION-ALLOW",
    "NOT-RESOURCE-ALLOW",
    "NOT-PRINCIPAL-ALLOW",
    "PRINCIPAL-STAR",
    "WILDCARD-PRINCIPAL-KEY",
    "PASS-ROLE-STAR",
    "PRIV-ESC",
    "CREDENTIAL-EXPOSURE",
    "RESOURCE-EXPOSURE",
    "DATA-EXFIL",
    // policy-type specific
    "PRINCIPAL-UNSUPPORTED",
    "SCP-PRINCIPAL",
    "SCP-ACTION-WILDCARD",
    "SCP-ALLOW-RESOURCE",
    "TRUST-POLICY-RESOURCE",
    "TRUST-POLICY-ACTION",
    // conditions
    "INVALID-CONDITION-OPERATOR",
    "INVALID-CONDITION-KEY",
    "INVALID-CONDITION-VALUE",
    "NULL-IF-EXISTS",
    "INVALID-CIDR",
    "PRIVATE-SOURCE-IP",
    "EMPTY-CONDITION",
];

// ---------------------------------------------------------------------------
// Curated sensitive action families (Cloudsplaining's insight: the finding is the
// sensitive action *paired with an unconstrained resource*, not the action alone).
// Families are checked in order and the first match wins, so one action never
// produces two overlapping findings.
// ---------------------------------------------------------------------------

const PRIV_ESC_ACTIONS: &[&str] = &[
    "iam:CreatePolicyVersion",
    "iam:SetDefaultPolicyVersion",
    "iam:AttachUserPolicy",
    "iam:AttachGroupPolicy",
    "iam:AttachRolePolicy",
    "iam:PutUserPolicy",
    "iam:PutGroupPolicy",
    "iam:PutRolePolicy",
    "iam:AddUserToGroup",
    "iam:UpdateAssumeRolePolicy",
    "iam:CreateServiceLinkedRole",
    "sts:AssumeRole",
    "lambda:CreateFunction",
    "lambda:UpdateFunctionCode",
    "lambda:UpdateFunctionConfiguration",
    "glue:CreateDevEndpoint",
    "glue:UpdateDevEndpoint",
    "cloudformation:CreateStack",
    "cloudformation:UpdateStack",
    "datapipeline:CreatePipeline",
    "datapipeline:PutPipelineDefinition",
    "ec2:RunInstances",
    "ssm:SendCommand",
    "ssm:StartSession",
    "codebuild:CreateProject",
    "codebuild:StartBuild",
    "codestar:CreateProject",
    "sagemaker:CreateNotebookInstance",
    "sagemaker:CreatePresignedNotebookInstanceUrl",
];

const CREDENTIAL_ACTIONS: &[&str] = &[
    "iam:CreateAccessKey",
    "iam:UpdateAccessKey",
    "iam:CreateLoginProfile",
    "iam:UpdateLoginProfile",
    "iam:CreateServiceSpecificCredential",
    "iam:ResetServiceSpecificCredential",
    "iam:UploadSigningCertificate",
    "iam:UpdateSigningCertificate",
    "iam:DeactivateMFADevice",
    "iam:ResyncMFADevice",
    "ec2:GetPasswordData",
    "sts:GetFederationToken",
    "sts:GetSessionToken",
    "ecr:GetAuthorizationToken",
    "ecr-public:GetAuthorizationToken",
    "redshift:GetClusterCredentials",
    "connect:GetFederationToken",
    "cognito-identity:GetOpenIdToken",
    "cognito-identity:GetCredentialsForIdentity",
    "lightsail:GetInstanceAccessDetails",
    "gamelift:RequestUploadCredentials",
];

const RESOURCE_EXPOSURE_ACTIONS: &[&str] = &[
    "s3:PutBucketPolicy",
    "s3:DeleteBucketPolicy",
    "s3:PutBucketAcl",
    "s3:PutObjectAcl",
    "s3:PutBucketPublicAccessBlock",
    "s3:PutAccountPublicAccessBlock",
    "ecr:SetRepositoryPolicy",
    "ecr:PutRegistryPolicy",
    "sns:AddPermission",
    "sqs:AddPermission",
    "lambda:AddPermission",
    "lambda:AddLayerVersionPermission",
    "kms:PutKeyPolicy",
    "kms:CreateGrant",
    "secretsmanager:PutResourcePolicy",
    "efs:PutFileSystemPolicy",
    "glacier:SetVaultAccessPolicy",
    "es:UpdateElasticsearchDomainConfig",
    "opensearch:UpdateDomainConfig",
    "ses:PutIdentityPolicy",
    "mediastore:PutContainerPolicy",
    "backup:PutBackupVaultAccessPolicy",
    "serverlessrepo:PutApplicationPolicy",
    "codeartifact:PutDomainPermissionsPolicy",
    "apigateway:UpdateRestApiPolicy",
    "iot:AttachPolicy",
];

const DATA_EXFIL_ACTIONS: &[&str] = &[
    "s3:GetObject",
    "s3:GetObjectVersion",
    "s3:GetObjectAcl",
    "ssm:GetParameter",
    "ssm:GetParameters",
    "ssm:GetParametersByPath",
    "secretsmanager:GetSecretValue",
    "secretsmanager:BatchGetSecretValue",
    "kms:Decrypt",
    "dynamodb:GetItem",
    "dynamodb:BatchGetItem",
    "dynamodb:Query",
    "dynamodb:Scan",
    "logs:GetLogEvents",
    "logs:FilterLogEvents",
    "sqs:ReceiveMessage",
    "cloudformation:GetTemplate",
    "lambda:GetFunction",
    "ecr:BatchGetImage",
    "rds:CopyDBSnapshot",
    "ec2:CreateSnapshot",
    "ec2:CopySnapshot",
    "ec2:ModifySnapshotAttribute",
    "athena:GetQueryResults",
    "codecommit:GitPull",
];

/// The condition operators AWS documents, without the `ForAllValues:`/`ForAnyValue:`
/// prefix or the `IfExists` suffix (both handled separately).
const CONDITION_OPERATORS: &[&str] = &[
    "StringEquals",
    "StringNotEquals",
    "StringEqualsIgnoreCase",
    "StringNotEqualsIgnoreCase",
    "StringLike",
    "StringNotLike",
    "NumericEquals",
    "NumericNotEquals",
    "NumericLessThan",
    "NumericLessThanEquals",
    "NumericGreaterThan",
    "NumericGreaterThanEquals",
    "DateEquals",
    "DateNotEquals",
    "DateLessThan",
    "DateLessThanEquals",
    "DateGreaterThan",
    "DateGreaterThanEquals",
    "Bool",
    "BinaryEquals",
    "IpAddress",
    "NotIpAddress",
    "ArnEquals",
    "ArnLike",
    "ArnNotEquals",
    "ArnNotLike",
    "Null",
];

const KNOWN_TOP_LEVEL: &[&str] = &["Version", "Id", "Statement"];
const KNOWN_STATEMENT_KEYS: &[&str] = &[
    "Sid",
    "Effect",
    "Action",
    "NotAction",
    "Resource",
    "NotResource",
    "Principal",
    "NotPrincipal",
    "Condition",
];
const KNOWN_PRINCIPAL_KEYS: &[&str] = &["AWS", "Service", "Federated", "CanonicalUser"];

// ---------------------------------------------------------------------------
// Finding + report
// ---------------------------------------------------------------------------

/// One lint finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    /// JSONPath into the policy document, e.g. `$.Statement[1].Action[0]`.
    pub path: String,
    /// 1-based source line of the element, when it can be located in the raw text.
    pub line: Option<usize>,
    pub message: String,
}

/// A full lint result: the findings plus the document facts the summary line needs.
#[derive(Clone, Debug)]
pub struct Report {
    pub policy_type: PolicyType,
    pub findings: Vec<Finding>,
    pub statements: usize,
    /// Characters excluding whitespace — how AWS measures the policy quota.
    pub characters: usize,
    pub truncated: bool,
}

impl Report {
    /// `unsafe` when anything is high, `review` when anything at all was found,
    /// `clean` otherwise. Computed over the findings that survive `ignore` — a
    /// display threshold never changes the verdict.
    pub fn verdict(&self) -> &'static str {
        if self.findings.iter().any(|f| f.severity == Severity::High) {
            "unsafe"
        } else if self.findings.is_empty() {
            "clean"
        } else {
            "review"
        }
    }
    pub fn count(&self, sev: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == sev).count()
    }
}

// ---------------------------------------------------------------------------
// Source-line index
// ---------------------------------------------------------------------------

/// Maps a JSONPath (`$.Statement[0].Action[1]`) to the 1-based source line of the
/// value it points at.
///
/// Built by a permissive second pass over the raw text — `serde_json` has already
/// proven the document parses, so this scanner only has to walk it, not validate it.
/// A dedicated pass is the only way to get line numbers: `serde_json::Value` keeps no
/// spans, and reporting "statement 2" without a line is exactly the gap that makes a
/// linter annoying on a 400-line policy.
struct LineIndex {
    lines: HashMap<String, usize>,
}

impl LineIndex {
    fn build(src: &str) -> LineIndex {
        let mut s = Scanner {
            b: src.as_bytes(),
            i: 0,
            newlines: src
                .as_bytes()
                .iter()
                .enumerate()
                .filter(|(_, c)| **c == b'\n')
                .map(|(i, _)| i)
                .collect(),
            out: HashMap::new(),
        };
        s.skip_ws();
        s.value("$");
        LineIndex { lines: s.out }
    }
    fn line(&self, path: &str) -> Option<usize> {
        self.lines.get(path).copied()
    }
}

struct Scanner<'a> {
    b: &'a [u8],
    i: usize,
    newlines: Vec<usize>,
    out: HashMap<String, usize>,
}

impl<'a> Scanner<'a> {
    fn line_at(&self, off: usize) -> usize {
        // Number of newlines strictly before `off`, plus one.
        self.newlines.partition_point(|n| *n < off) + 1
    }
    fn skip_ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    /// Consume a JSON string starting at the opening quote; return its contents.
    fn string(&mut self) -> String {
        let mut out = String::new();
        if self.peek() != Some(b'"') {
            return out;
        }
        self.i += 1;
        let start = self.i;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'\\' => self.i += 2,
                b'"' => break,
                _ => self.i += 1,
            }
        }
        let raw = String::from_utf8_lossy(&self.b[start..self.i.min(self.b.len())]).into_owned();
        // The key is only used to build a path, so a JSON-unescape of the common
        // escapes is enough; anything exotic simply yields a path that never matches.
        out.push_str(&raw.replace("\\\"", "\"").replace("\\\\", "\\"));
        if self.i < self.b.len() {
            self.i += 1; // closing quote
        }
        out
    }
    fn value(&mut self, path: &str) {
        self.skip_ws();
        let start = self.i;
        self.out.insert(path.to_string(), self.line_at(start));
        match self.peek() {
            Some(b'{') => {
                self.i += 1;
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(b'}') => {
                            self.i += 1;
                            return;
                        }
                        Some(b'"') => {}
                        _ => return, // malformed; best effort only
                    }
                    let key = self.string();
                    self.skip_ws();
                    if self.peek() == Some(b':') {
                        self.i += 1;
                    }
                    let child = if path == "$" {
                        format!("$.{key}")
                    } else {
                        format!("{path}.{key}")
                    };
                    self.value(&child);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return;
                        }
                        _ => return,
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut idx = 0usize;
                loop {
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        self.i += 1;
                        return;
                    }
                    self.value(&format!("{path}[{idx}]"));
                    idx += 1;
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return;
                        }
                        _ => return,
                    }
                }
            }
            Some(b'"') => {
                let _ = self.string();
            }
            _ => {
                while self.i < self.b.len()
                    && !matches!(self.b[self.i], b',' | b'}' | b']')
                    && !(self.b[self.i] as char).is_ascii_whitespace()
                {
                    self.i += 1;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Case-insensitive glob match supporting `*` (any run) and `?` (one character) —
/// the wildcard grammar IAM itself uses for actions and ARNs.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            pi += 1;
            mark = ti;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// `true` when the string grants everything: literal `*`, or an ARN whose every
/// field is a bare wildcard.
fn matches_everything(s: &str) -> bool {
    let s = s.trim();
    if s == "*" {
        return true;
    }
    let parts = arn_parts(s);
    match parts {
        Some(p) => {
            p.len() >= 6
                && p[1..5].iter().all(|f| f.is_empty() || *f == "*")
                && p[5..].iter().all(|f| *f == "*")
        }
        None => false,
    }
}

/// Split an ARN into at most 6 fields (`arn:partition:service:region:account:resource`),
/// or `None` when the string is not ARN-shaped.
fn arn_parts(s: &str) -> Option<Vec<&str>> {
    if !s.to_ascii_lowercase().starts_with("arn:") {
        return None;
    }
    Some(s.splitn(6, ':').collect())
}

/// `true` when this resource leaves the given service completely unconstrained —
/// `*`, an all-wildcard ARN, or `arn:aws:s3:::*` for an `s3:` action.
fn unconstrained_for_service(resource: &str, service: &str) -> bool {
    if matches_everything(resource) {
        return true;
    }
    match arn_parts(resource) {
        Some(p) if p.len() >= 6 => {
            let svc = p[2];
            let res = p[5];
            (svc == "*" || glob_match(service, svc) || glob_match(svc, service)) && res == "*"
        }
        _ => false,
    }
}

/// Every string element of a policy element that may be a string or an array of
/// strings, paired with its JSONPath. Non-string entries are reported by the caller.
fn string_list<'a>(v: &'a Value, base: &str) -> (Vec<(String, &'a str)>, Vec<String>) {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    match v {
        Value::String(s) => ok.push((base.to_string(), s.as_str())),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let p = format!("{base}[{i}]");
                match item {
                    Value::String(s) => ok.push((p, s.as_str())),
                    _ => bad.push(p),
                }
            }
        }
        _ => bad.push(base.to_string()),
    }
    (ok, bad)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Format at most `max` sample actions for a family message.
fn sample(list: &[&str], max: usize) -> String {
    if list.len() <= max {
        list.join(", ")
    } else {
        format!("{}, +{} more", list[..max].join(", "), list.len() - max)
    }
}

// ---------------------------------------------------------------------------
// Linting
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    ptype: PolicyType,
    index: &'a LineIndex,
    findings: Vec<Finding>,
    truncated: bool,
}

impl<'a> Ctx<'a> {
    fn add(&mut self, code: &'static str, severity: Severity, path: &str, message: String) {
        if self.findings.len() >= MAX_FINDINGS {
            self.truncated = true;
            return;
        }
        let line = self.index.line(path).or_else(|| {
            // Fall back to the nearest ancestor that does have a line — an element
            // that is missing entirely has no span of its own.
            let mut p = path.to_string();
            loop {
                let cut = p.rfind(['.', '[']).unwrap_or(0);
                if cut == 0 {
                    return self.index.line("$");
                }
                p.truncate(cut);
                if let Some(l) = self.index.line(&p) {
                    return Some(l);
                }
            }
        });
        self.findings.push(Finding {
            code,
            severity,
            path: path.to_string(),
            line,
            message,
        });
    }
}

/// Lint a policy document. Returns `Err` only when the input cannot be treated as a
/// policy at all (unparseable JSON, wrong top-level type, oversized input); every
/// other problem is a finding.
pub fn lint(policy: &str, ptype: PolicyType) -> Result<Report, String> {
    if policy.trim().is_empty() {
        return Err("policy is empty — paste an AWS IAM policy JSON document".into());
    }
    if policy.chars().count() > MAX_INPUT_CHARS {
        return Err(format!(
            "policy is too large: {} characters (limit {MAX_INPUT_CHARS})",
            policy.chars().count()
        ));
    }
    let doc: Value = serde_json::from_str(policy).map_err(|e| {
        format!(
            "invalid JSON policy: {} (line {}, column {})",
            e.to_string()
                .rsplit_once(" at line")
                .map(|(m, _)| m.to_string())
                .unwrap_or_else(|| e.to_string()),
            e.line(),
            e.column()
        )
    })?;
    let root = match &doc {
        Value::Object(m) => m,
        _ => return Err("policy must be a JSON object with Version and Statement elements".into()),
    };

    let index = LineIndex::build(policy);
    let mut ctx = Ctx {
        ptype,
        index: &index,
        findings: Vec::new(),
        truncated: false,
    };

    check_top_level(&mut ctx, root);

    // Statement may be one object or an array of objects.
    let mut statements: Vec<(String, &Map<String, Value>)> = Vec::new();
    match root.get("Statement") {
        None => ctx.add(
            "MISSING-STATEMENT",
            Severity::High,
            "$",
            "The policy has no Statement element. Every policy needs at least one statement."
                .into(),
        ),
        Some(Value::Array(items)) if items.is_empty() => ctx.add(
            "MISSING-STATEMENT",
            Severity::High,
            "$.Statement",
            "Statement is an empty array. Every policy needs at least one statement.".into(),
        ),
        Some(Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                let path = format!("$.Statement[{i}]");
                match item {
                    Value::Object(m) => statements.push((path, m)),
                    _ => ctx.add(
                        "INVALID-STATEMENT",
                        Severity::High,
                        &path,
                        "Statement entries must be JSON objects.".into(),
                    ),
                }
            }
        }
        Some(Value::Object(m)) => statements.push(("$.Statement".to_string(), m)),
        Some(_) => ctx.add(
            "INVALID-STATEMENT",
            Severity::High,
            "$.Statement",
            "Statement must be an object or an array of objects.".into(),
        ),
    }

    let mut seen_sids: HashMap<String, String> = HashMap::new();
    for (path, stmt) in &statements {
        check_statement(&mut ctx, path, stmt, &mut seen_sids);
    }

    // Policy size, measured the way AWS measures it.
    let chars = policy.chars().filter(|c| !c.is_whitespace()).count();
    if chars > MAX_POLICY_CHARS {
        ctx.add(
            "POLICY-SIZE",
            Severity::Medium,
            "$",
            format!(
                "The policy is {chars} characters excluding whitespace, over the \
                 {MAX_POLICY_CHARS}-character managed-policy quota. Split it into \
                 several policies."
            ),
        );
    }

    // Highest severity first; generation order (document order) breaks ties.
    let mut findings = ctx.findings;
    findings.sort_by(|a, b| b.severity.cmp(&a.severity));

    Ok(Report {
        policy_type: ptype,
        findings,
        statements: statements.len(),
        characters: chars,
        truncated: ctx.truncated,
    })
}

fn check_top_level(ctx: &mut Ctx, root: &Map<String, Value>) {
    for key in root.keys() {
        if !KNOWN_TOP_LEVEL.contains(&key.as_str()) {
            ctx.add(
                "UNKNOWN-ELEMENT",
                Severity::Medium,
                &format!("$.{key}"),
                format!(
                    "`{key}` is not a policy element. AWS accepts Version, Id and Statement \
                     at the top level and rejects the document otherwise."
                ),
            );
        }
    }
    match root.get("Version") {
        None => ctx.add(
            "MISSING-VERSION",
            Severity::Low,
            "$",
            "No Version element. AWS then assumes the 2008-10-17 grammar, in which policy \
             variables do not work. Add \"Version\": \"2012-10-17\"."
                .into(),
        ),
        Some(Value::String(v)) if v == "2012-10-17" => {}
        Some(Value::String(v)) if v == "2008-10-17" => ctx.add(
            "LEGACY-VERSION",
            Severity::Low,
            "$.Version",
            "Version 2008-10-17 is the legacy grammar: policy variables and some condition \
             features are unavailable. Use 2012-10-17."
                .into(),
        ),
        Some(v) => ctx.add(
            "INVALID-VERSION",
            Severity::High,
            "$.Version",
            format!(
                "Version {} is not a valid policy language version; AWS accepts only \
                 \"2012-10-17\" or \"2008-10-17\".",
                compact(v)
            ),
        ),
    }
}

/// A short, quoted rendering of a JSON value for use inside a message.
fn compact(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 60 {
        format!("{}…", s.chars().take(60).collect::<String>())
    } else {
        s
    }
}

fn check_statement(
    ctx: &mut Ctx,
    path: &str,
    stmt: &Map<String, Value>,
    seen_sids: &mut HashMap<String, String>,
) {
    // --- unknown keys -------------------------------------------------------
    for key in stmt.keys() {
        if !KNOWN_STATEMENT_KEYS.contains(&key.as_str()) {
            ctx.add(
                "UNKNOWN-ELEMENT",
                Severity::Medium,
                &format!("{path}.{key}"),
                format!(
                    "`{key}` is not a statement element. AWS rejects a policy that carries an \
                     unrecognised key; check the spelling and capitalisation."
                ),
            );
        }
    }

    // --- Sid ----------------------------------------------------------------
    if let Some(sid) = stmt.get("Sid") {
        let sid_path = format!("{path}.Sid");
        match sid {
            Value::String(s) => {
                if let Some(prev) = seen_sids.insert(s.clone(), sid_path.clone()) {
                    ctx.add(
                        "DUPLICATE-SID",
                        Severity::Medium,
                        &sid_path,
                        format!(
                            "Sid \"{s}\" is already used at {prev}. Statement ids must be unique \
                             within a policy."
                        ),
                    );
                }
                if matches!(ctx.ptype, PolicyType::Identity | PolicyType::Scp)
                    && !s.chars().all(|c| c.is_ascii_alphanumeric())
                {
                    ctx.add(
                        "INVALID-SID",
                        Severity::Low,
                        &sid_path,
                        format!(
                            "Sid \"{s}\" contains characters other than letters and digits. \
                             Identity policies and SCPs allow only [A-Za-z0-9]; resource \
                             policies additionally allow spaces."
                        ),
                    );
                }
            }
            _ => ctx.add(
                "INVALID-SID",
                Severity::Low,
                &sid_path,
                "Sid must be a string.".into(),
            ),
        }
    }

    // --- Effect -------------------------------------------------------------
    let effect = match stmt.get("Effect") {
        None => {
            ctx.add(
                "MISSING-EFFECT",
                Severity::High,
                &format!("{path}.Effect"),
                "The statement has no Effect element. Every statement needs \"Effect\": \
                 \"Allow\" or \"Deny\"."
                    .into(),
            );
            None
        }
        Some(Value::String(s)) if s == "Allow" || s == "Deny" => Some(s.as_str()),
        Some(v) => {
            ctx.add(
                "INVALID-EFFECT",
                Severity::High,
                &format!("{path}.Effect"),
                format!(
                    "Effect is {}; it must be exactly \"Allow\" or \"Deny\" (case-sensitive).",
                    compact(v)
                ),
            );
            None
        }
    };
    let is_allow = effect == Some("Allow");

    // --- element conflicts ---------------------------------------------------
    for (a, b) in [
        ("Action", "NotAction"),
        ("Resource", "NotResource"),
        ("Principal", "NotPrincipal"),
    ] {
        if stmt.contains_key(a) && stmt.contains_key(b) {
            ctx.add(
                "ELEMENT-CONFLICT",
                Severity::High,
                &format!("{path}.{b}"),
                format!(
                    "The statement uses both {a} and {b}. AWS rejects a statement that carries \
                     both forms; keep one."
                ),
            );
        }
    }

    // --- Action / NotAction --------------------------------------------------
    let mut actions: Vec<(String, &str)> = Vec::new();
    let mut has_action_element = false;
    for key in ["Action", "NotAction"] {
        let Some(v) = stmt.get(key) else { continue };
        has_action_element = true;
        let base = format!("{path}.{key}");
        let (ok, bad) = string_list(v, &base);
        for p in bad {
            ctx.add(
                "INVALID-ACTION",
                Severity::Medium,
                &p,
                format!("{key} entries must be strings such as \"s3:GetObject\" or \"s3:*\"."),
            );
        }
        if ok.is_empty() && matches!(v, Value::Array(_)) {
            ctx.add(
                "MISSING-ACTION",
                Severity::High,
                &base,
                format!("{key} is an empty array, so the statement grants nothing."),
            );
        }
        for (p, a) in &ok {
            check_action_grammar(ctx, p, a, key);
        }
        if key == "Action" {
            actions = ok;
        } else {
            for (p, a) in &ok {
                if is_allow {
                    ctx.add(
                        "NOT-ACTION-ALLOW",
                        Severity::High,
                        p,
                        format!(
                            "Allow with NotAction \"{a}\" grants every action except the listed \
                             ones — including actions AWS adds in future. Enumerate the actions \
                             you mean to allow instead."
                        ),
                    );
                }
            }
        }
    }
    if !has_action_element {
        ctx.add(
            "MISSING-ACTION",
            Severity::High,
            &format!("{path}.Action"),
            "The statement has no Action or NotAction element, so it grants or denies nothing."
                .into(),
        );
    }

    // --- Resource / NotResource ----------------------------------------------
    let mut resources: Vec<(String, &str)> = Vec::new();
    let mut has_resource_element = false;
    let mut allow_not_resource = false;
    for key in ["Resource", "NotResource"] {
        let Some(v) = stmt.get(key) else { continue };
        has_resource_element = true;
        let base = format!("{path}.{key}");
        if ctx.ptype == PolicyType::Trust {
            ctx.add(
                "TRUST-POLICY-RESOURCE",
                Severity::High,
                &base,
                format!(
                    "A role trust policy must not contain a {key} element — the resource is the \
                     role itself. AWS rejects the document."
                ),
            );
        }
        let (ok, bad) = string_list(v, &base);
        for p in bad {
            ctx.add(
                "INVALID-ARN",
                Severity::Medium,
                &p,
                format!("{key} entries must be strings — an ARN or \"*\"."),
            );
        }
        for (p, r) in &ok {
            check_arn(ctx, p, r, key);
            check_variable(ctx, p, r);
        }
        if key == "Resource" {
            resources = ok;
        } else {
            allow_not_resource = is_allow;
            for (p, r) in &ok {
                if is_allow {
                    ctx.add(
                        "NOT-RESOURCE-ALLOW",
                        Severity::High,
                        p,
                        format!(
                            "Allow with NotResource \"{r}\" grants access to every resource \
                             except the listed ones. Use an explicit Resource list, or move the \
                             NotResource to a Deny statement."
                        ),
                    );
                }
            }
        }
    }
    if !has_resource_element {
        match ctx.ptype {
            PolicyType::Identity | PolicyType::Scp => ctx.add(
                "MISSING-RESOURCE",
                Severity::High,
                &format!("{path}.Resource"),
                "The statement has no Resource or NotResource element. Identity policies and \
                 SCPs require one."
                    .into(),
            ),
            PolicyType::Resource => ctx.add(
                "MISSING-RESOURCE",
                Severity::Medium,
                &format!("{path}.Resource"),
                "The statement has no Resource element. Most resource policies require one; \
                 a few (KMS key policies, for example) imply the attached resource."
                    .into(),
            ),
            PolicyType::Trust => {}
        }
    }

    // --- Principal / NotPrincipal --------------------------------------------
    let has_condition = stmt
        .get("Condition")
        .map(|c| matches!(c, Value::Object(m) if !m.is_empty()))
        .unwrap_or(false);
    let mut has_principal_element = false;
    for key in ["Principal", "NotPrincipal"] {
        let Some(v) = stmt.get(key) else { continue };
        has_principal_element = true;
        let base = format!("{path}.{key}");
        match ctx.ptype {
            PolicyType::Identity => ctx.add(
                "PRINCIPAL-UNSUPPORTED",
                Severity::High,
                &base,
                format!(
                    "Identity policies do not support a {key} element — the principal is \
                     whoever the policy is attached to. AWS rejects the document."
                ),
            ),
            PolicyType::Scp => ctx.add(
                "SCP-PRINCIPAL",
                Severity::High,
                &base,
                format!("Service control policies do not support a {key} element. Remove it."),
            ),
            _ => {}
        }
        if key == "NotPrincipal" && is_allow {
            ctx.add(
                "NOT-PRINCIPAL-ALLOW",
                Severity::High,
                &base,
                "Allow with NotPrincipal grants access to every principal except the listed \
                 ones, including anonymous callers. AWS documents NotPrincipal as a Deny-only \
                 construct; list the principals you mean to allow instead."
                    .into(),
            );
        }
        check_principal(ctx, &base, v, is_allow, has_condition, key);
    }
    if !has_principal_element && ctx.ptype.wants_principal() {
        let what = if ctx.ptype == PolicyType::Trust {
            "A role trust policy"
        } else {
            "A resource policy"
        };
        ctx.add(
            "MISSING-PRINCIPAL",
            Severity::High,
            &format!("{path}.Principal"),
            format!("{what} statement must name the Principal it applies to."),
        );
    }

    // --- Condition ------------------------------------------------------------
    if let Some(cond) = stmt.get("Condition") {
        check_condition(ctx, &format!("{path}.Condition"), cond);
    }

    // --- policy-type specific -------------------------------------------------
    if ctx.ptype == PolicyType::Trust {
        for (p, a) in &actions {
            if !a.to_ascii_lowercase().starts_with("sts:") && *a != "*" {
                ctx.add(
                    "TRUST-POLICY-ACTION",
                    Severity::Medium,
                    p,
                    format!(
                        "\"{a}\" is not an sts action. A role trust policy can only grant the \
                         sts:AssumeRole family."
                    ),
                );
            }
        }
    }
    if ctx.ptype == PolicyType::Scp {
        for (p, a) in &actions {
            if let Some(pos) = a.find('*') {
                if pos != a.len() - 1 {
                    ctx.add(
                        "SCP-ACTION-WILDCARD",
                        Severity::Medium,
                        p,
                        format!(
                            "\"{a}\" puts a wildcard in the middle of the action. Service control \
                             policies only support a wildcard at the end of the string."
                        ),
                    );
                }
            }
        }
        if is_allow {
            for (p, r) in &resources {
                if *r != "*" {
                    ctx.add(
                        "SCP-ALLOW-RESOURCE",
                        Severity::Medium,
                        p,
                        format!(
                            "An SCP Allow statement must use Resource \"*\"; \"{r}\" is not \
                             supported. Restrict resources with a Deny statement instead."
                        ),
                    );
                }
            }
        }
    }

    // --- permissiveness -------------------------------------------------------
    if !is_allow {
        return;
    }
    let resource_star = resources.iter().any(|(_, r)| *r == "*") || allow_not_resource;
    let everything = resources.iter().any(|(_, r)| matches_everything(r)) || allow_not_resource;
    let action_star = actions
        .iter()
        .find(|(_, a)| *a == "*" || a.eq_ignore_ascii_case("*:*"));

    for (p, r) in &resources {
        if *r == "*" && action_star.is_none() {
            ctx.add(
                "RESOURCE-STAR",
                Severity::Medium,
                p,
                "Allow with Resource \"*\" lets the listed actions run against every resource \
                 in the account. Scope it to the ARNs the workload really touches."
                    .into(),
            );
        } else if *r != "*" && matches_everything(r) {
            ctx.add(
                "RESOURCE-EFFECTIVELY-STAR",
                Severity::Medium,
                p,
                format!(
                    "\"{r}\" looks scoped but every field is a wildcard, so it matches exactly \
                     what \"*\" matches."
                ),
            );
        }
    }

    if let Some((p, _)) = action_star {
        if everything {
            ctx.add(
                "ADMIN-STAR",
                Severity::High,
                p,
                "Allow with Action \"*\" on an unconstrained Resource grants full \
                 administrator access — every action, on every resource, in the account."
                    .into(),
            );
        } else {
            ctx.add(
                "ACTION-STAR",
                Severity::High,
                p,
                "Allow with Action \"*\" grants every action AWS offers on the listed \
                 resources, including actions that do not exist yet. Enumerate the actions \
                 the workload needs."
                    .into(),
            );
        }
    }

    for (p, a) in &actions {
        if *a == "*" || a.eq_ignore_ascii_case("*:*") {
            continue; // already reported as ADMIN-STAR / ACTION-STAR
        }
        let service = a.split(':').next().unwrap_or("*");
        let unconstrained = everything
            || allow_not_resource
            || resources
                .iter()
                .any(|(_, r)| unconstrained_for_service(r, service));

        if a.ends_with(":*") && a.matches('*').count() == 1 {
            ctx.add(
                "SERVICE-ACTION-STAR",
                Severity::Medium,
                p,
                format!(
                    "\"{a}\" grants every action in the {service} service, including \
                     destructive and policy-changing ones."
                ),
            );
        }
        if !unconstrained {
            continue;
        }
        if glob_match(a, "iam:PassRole") {
            ctx.add(
                "PASS-ROLE-STAR",
                Severity::High,
                p,
                format!(
                    "\"{a}\" covers iam:PassRole with an unconstrained Resource, so this \
                     principal can hand *any* role to a service and inherit its permissions. \
                     Restrict the Resource to the exact role ARNs."
                ),
            );
        }
        let families: [(&'static str, Severity, &[&str], &str); 4] = [
            (
                "PRIV-ESC",
                Severity::High,
                PRIV_ESC_ACTIONS,
                "privilege-escalation",
            ),
            (
                "CREDENTIAL-EXPOSURE",
                Severity::High,
                CREDENTIAL_ACTIONS,
                "credential-exposure",
            ),
            (
                "RESOURCE-EXPOSURE",
                Severity::Medium,
                RESOURCE_EXPOSURE_ACTIONS,
                "resource-exposure",
            ),
            (
                "DATA-EXFIL",
                Severity::Medium,
                DATA_EXFIL_ACTIONS,
                "data-exfiltration",
            ),
        ];
        for (code, sev, list, label) in families {
            let hits: Vec<&str> = list
                .iter()
                .copied()
                .filter(|candidate| glob_match(a, candidate))
                .collect();
            if hits.is_empty() {
                continue;
            }
            ctx.add(
                code,
                sev,
                p,
                format!(
                    "\"{a}\" covers {} on an unconstrained Resource ({}).",
                    plural(
                        hits.len(),
                        &format!("{label} action"),
                        &format!("{label} actions")
                    ),
                    sample(&hits, 4)
                ),
            );
            break; // first family wins — one finding per action
        }
    }
    let _ = resource_star;
}

fn check_action_grammar(ctx: &mut Ctx, path: &str, action: &str, key: &str) {
    if action == "*" {
        return;
    }
    if action.trim().is_empty() {
        ctx.add(
            "INVALID-ACTION",
            Severity::Medium,
            path,
            format!("{key} contains an empty string."),
        );
        return;
    }
    if action.contains("${") {
        ctx.add(
            "INVALID-VARIABLE",
            Severity::High,
            path,
            format!(
                "\"{action}\" uses a policy variable in {key}. Policy variables are only \
                 substituted in Resource and Condition values; here the text is matched \
                 literally."
            ),
        );
        return;
    }
    let Some((service, op)) = action.split_once(':') else {
        ctx.add(
            "INVALID-ACTION",
            Severity::Medium,
            path,
            format!(
                "\"{action}\" is not a valid action string. Actions are written \
                 `service:Operation`, e.g. \"s3:GetObject\" or \"s3:*\"."
            ),
        );
        return;
    };
    if service.is_empty() || op.is_empty() {
        ctx.add(
            "INVALID-ACTION",
            Severity::Medium,
            path,
            format!("\"{action}\" is missing the service prefix or the operation name."),
        );
        return;
    }
    if op.contains(':') {
        ctx.add(
            "INVALID-ACTION",
            Severity::Medium,
            path,
            format!("\"{action}\" has more than one colon; an action is `service:Operation`."),
        );
        return;
    }
    if service.chars().any(|c| c.is_ascii_uppercase()) {
        ctx.add(
            "INVALID-ACTION",
            Severity::Medium,
            path,
            format!(
                "The service prefix in \"{action}\" must be lowercase — AWS matches it \
                 case-sensitively."
            ),
        );
    }
    if service
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '*'))
    {
        ctx.add(
            "INVALID-ACTION",
            Severity::Medium,
            path,
            format!("The service prefix in \"{action}\" contains unexpected characters."),
        );
    }
}

fn check_arn(ctx: &mut Ctx, path: &str, resource: &str, key: &str) {
    if resource == "*" {
        return;
    }
    if resource.trim().is_empty() {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!("{key} contains an empty string."),
        );
        return;
    }
    let Some(parts) = arn_parts(resource) else {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!("\"{resource}\" is neither \"*\" nor an ARN. {key} values start with `arn:`."),
        );
        return;
    };
    if parts.len() < 6 {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!(
                "\"{resource}\" has {} of the 6 ARN fields. The shape is \
                 `arn:partition:service:region:account-id:resource`.",
                parts.len()
            ),
        );
        return;
    }
    let (partition, service, region, account, res) =
        (parts[1], parts[2], parts[3], parts[4], parts[5]);
    if !partition.contains('*')
        && !partition.contains('?')
        && !matches!(
            partition,
            "aws" | "aws-cn" | "aws-us-gov" | "aws-iso" | "aws-iso-b"
        )
    {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!(
                "\"{partition}\" is not an AWS partition. Use aws, aws-cn or aws-us-gov \
                 (in \"{resource}\")."
            ),
        );
    }
    if service.chars().any(|c| c.is_ascii_uppercase()) {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!("The service field in \"{resource}\" must be lowercase."),
        );
    }
    if service.is_empty() {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!("\"{resource}\" has an empty service field."),
        );
    }
    if !account.is_empty()
        && !account.contains('*')
        && !account.contains('?')
        && !account.contains("${")
        && !(account.len() == 12 && account.chars().all(|c| c.is_ascii_digit()))
        && account != "aws"
    {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!(
                "\"{account}\" is not a 12-digit account id (in \"{resource}\"). Leave the \
                 field empty for services that do not use it."
            ),
        );
    }
    if res.is_empty() {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!("\"{resource}\" has an empty resource field."),
        );
    }
    if region.chars().any(|c| c.is_ascii_uppercase()) {
        ctx.add(
            "INVALID-ARN",
            Severity::Medium,
            path,
            format!("The region field in \"{resource}\" must be lowercase."),
        );
    }
}

/// Policy-variable syntax: `${aws:username}`. Unbalanced braces, an empty variable or
/// spaces inside the braces all make AWS reject the policy.
fn check_variable(ctx: &mut Ctx, path: &str, s: &str) {
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == '$' && bytes[i + 1] == '{' {
            let start = i + 2;
            let mut j = start;
            while j < bytes.len() && bytes[j] != '}' {
                j += 1;
            }
            if j >= bytes.len() {
                ctx.add(
                    "INVALID-VARIABLE",
                    Severity::High,
                    path,
                    format!("\"{s}\" opens a policy variable with `${{` but never closes it."),
                );
                return;
            }
            let inner: String = bytes[start..j].iter().collect();
            if inner.is_empty() {
                ctx.add(
                    "INVALID-VARIABLE",
                    Severity::High,
                    path,
                    format!("\"{s}\" contains an empty policy variable `${{}}`."),
                );
            } else if inner.contains(' ') {
                ctx.add(
                    "INVALID-VARIABLE",
                    Severity::High,
                    path,
                    format!(
                        "The policy variable `${{{inner}}}` contains a space; AWS rejects it. \
                         Write `${{aws:username}}` with no padding."
                    ),
                );
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
}

fn check_principal(
    ctx: &mut Ctx,
    base: &str,
    v: &Value,
    is_allow: bool,
    has_condition: bool,
    key: &str,
) {
    let mut entries: Vec<(String, &str)> = Vec::new();
    match v {
        Value::String(s) => entries.push((base.to_string(), s.as_str())),
        Value::Object(m) => {
            for (k, val) in m {
                let p = format!("{base}.{k}");
                if !KNOWN_PRINCIPAL_KEYS.contains(&k.as_str()) {
                    ctx.add(
                        "INVALID-PRINCIPAL",
                        Severity::Medium,
                        &p,
                        format!(
                            "\"{k}\" is not a principal type. AWS accepts AWS, Service, \
                             Federated and CanonicalUser."
                        ),
                    );
                    continue;
                }
                let (ok, bad) = string_list(val, &p);
                for bp in bad {
                    ctx.add(
                        "INVALID-PRINCIPAL",
                        Severity::Medium,
                        &bp,
                        format!("{key} entries must be strings or arrays of strings."),
                    );
                }
                entries.extend(ok);
            }
        }
        _ => {
            ctx.add(
                "INVALID-PRINCIPAL",
                Severity::Medium,
                base,
                format!(
                    "{key} must be \"*\" or an object keyed by principal type \
                     (AWS / Service / Federated / CanonicalUser)."
                ),
            );
            return;
        }
    }

    for (p, s) in entries {
        if s == "*" {
            if key == "NotPrincipal" {
                continue; // NOT-PRINCIPAL-ALLOW already covers the risk
            }
            if is_allow {
                let (sev, tail) = if has_condition {
                    (
                        Severity::Medium,
                        "A Condition narrows it, so confirm the condition really pins the \
                         caller down (an org id, an account, a source ARN).",
                    )
                } else {
                    (
                        Severity::High,
                        "Nothing narrows it, so anyone on the internet with an AWS account \
                         can call these actions.",
                    )
                };
                ctx.add(
                    "PRINCIPAL-STAR",
                    sev,
                    &p,
                    format!("Principal \"*\" makes this statement public. {tail}"),
                );
            }
        } else if s.contains('*') || s.contains('?') {
            if is_allow {
                ctx.add(
                    "WILDCARD-PRINCIPAL-KEY",
                    Severity::Medium,
                    &p,
                    format!(
                        "\"{s}\" uses a wildcard inside a principal. AWS does not expand \
                         wildcards in a principal ARN the way it does in a Resource — the value \
                         is matched literally, so the statement either fails to match or is far \
                         broader than intended."
                    ),
                );
            }
        }
    }
}

fn check_condition(ctx: &mut Ctx, base: &str, cond: &Value) {
    let Value::Object(ops) = cond else {
        ctx.add(
            "EMPTY-CONDITION",
            Severity::Medium,
            base,
            "Condition must be an object keyed by condition operator.".into(),
        );
        return;
    };
    if ops.is_empty() {
        ctx.add(
            "EMPTY-CONDITION",
            Severity::Medium,
            base,
            "The Condition block is empty, so it constrains nothing. Remove it or fill it in."
                .into(),
        );
        return;
    }
    for (op, body) in ops {
        let op_path = format!("{base}.{op}");
        // Strip the set-operator prefix and the IfExists suffix before matching.
        let mut core = op.as_str();
        for prefix in ["ForAllValues:", "ForAnyValue:"] {
            if let Some(rest) = core.strip_prefix(prefix) {
                core = rest;
            }
        }
        let if_exists = core.ends_with("IfExists");
        if if_exists {
            core = &core[..core.len() - "IfExists".len()];
        }
        let known = CONDITION_OPERATORS.iter().any(|k| *k == core);
        if !known {
            ctx.add(
                "INVALID-CONDITION-OPERATOR",
                Severity::High,
                &op_path,
                format!(
                    "\"{op}\" is not a condition operator. AWS rejects the policy; check the \
                     spelling and capitalisation (StringEquals, StringLike, ArnLike, \
                     NumericLessThan, DateGreaterThan, Bool, IpAddress, Null, …)."
                ),
            );
            continue;
        }
        if core == "Null" && if_exists {
            ctx.add(
                "NULL-IF-EXISTS",
                Severity::High,
                &op_path,
                "Null cannot be combined with the IfExists suffix — Null already tests for the \
                 key's presence. AWS rejects the policy."
                    .into(),
            );
        }
        let Value::Object(keys) = body else {
            ctx.add(
                "INVALID-CONDITION-VALUE",
                Severity::Medium,
                &op_path,
                format!("The body of \"{op}\" must be an object of condition key → value."),
            );
            continue;
        };
        if keys.is_empty() {
            ctx.add(
                "EMPTY-CONDITION",
                Severity::Medium,
                &op_path,
                format!("\"{op}\" has no condition keys, so it constrains nothing."),
            );
            continue;
        }
        for (key, val) in keys {
            let key_path = format!("{op_path}.{key}");
            check_condition_key(ctx, &key_path, key);
            check_condition_value(ctx, &key_path, core, key, val);
        }
    }
}

fn check_condition_key(ctx: &mut Ctx, path: &str, key: &str) {
    let bad = key.trim().is_empty()
        || !key.contains(':')
        || key.starts_with(':')
        || key.contains(' ')
        || key
            .split(':')
            .next()
            .map(|p| p.is_empty() || p.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '-')))
            .unwrap_or(true);
    if bad {
        ctx.add(
            "INVALID-CONDITION-KEY",
            Severity::Medium,
            path,
            format!(
                "\"{key}\" is not a condition key. Keys are namespaced — `aws:SourceIp`, \
                 `aws:PrincipalOrgID`, `s3:prefix`, `kms:ViaService`."
            ),
        );
    }
}

fn check_condition_value(ctx: &mut Ctx, path: &str, op: &str, key: &str, val: &Value) {
    let values: Vec<&Value> = match val {
        Value::Array(items) => {
            if items.is_empty() {
                ctx.add(
                    "EMPTY-CONDITION",
                    Severity::Medium,
                    path,
                    format!("Condition key \"{key}\" has an empty value list."),
                );
                return;
            }
            items.iter().collect()
        }
        v => vec![v],
    };
    for v in &values {
        if matches!(v, Value::Object(_) | Value::Array(_) | Value::Null) {
            ctx.add(
                "INVALID-CONDITION-VALUE",
                Severity::Medium,
                path,
                format!(
                    "Condition values must be strings, numbers or booleans; \"{key}\" got {}.",
                    compact(v)
                ),
            );
        }
        if let Value::String(s) = v {
            check_variable(ctx, path, s);
        }
    }
    if op == "Bool" {
        for v in &values {
            let ok = match v {
                Value::Bool(_) => true,
                Value::String(s) => s == "true" || s == "false",
                _ => false,
            };
            if !ok {
                ctx.add(
                    "INVALID-CONDITION-VALUE",
                    Severity::Medium,
                    path,
                    format!(
                        "Bool conditions take exactly \"true\" or \"false\"; \"{key}\" got {}.",
                        compact(v)
                    ),
                );
            }
        }
        if values.len() > 1 {
            ctx.add(
                "INVALID-CONDITION-VALUE",
                Severity::Medium,
                path,
                format!(
                    "Bool condition key \"{key}\" lists {} values. A boolean key takes one.",
                    values.len()
                ),
            );
        }
    }
    if op == "IpAddress" || op == "NotIpAddress" {
        for v in &values {
            let Value::String(s) = v else { continue };
            match parse_cidr(s) {
                None => ctx.add(
                    "INVALID-CIDR",
                    Severity::High,
                    path,
                    format!(
                        "\"{s}\" is not a valid IP address or CIDR block. {op} takes values \
                         such as \"203.0.113.0/24\" or \"2001:db8::/32\"."
                    ),
                ),
                Some(octets) => {
                    if key.eq_ignore_ascii_case("aws:SourceIp") && is_private_v4(&octets) {
                        ctx.add(
                            "PRIVATE-SOURCE-IP",
                            Severity::Medium,
                            path,
                            format!(
                                "\"{s}\" is a private (RFC 1918 / loopback / link-local) range, \
                                 but aws:SourceIp carries the caller's public IP. This condition \
                                 can never match. Use aws:VpcSourceIp or aws:SourceVpce for \
                                 traffic inside a VPC."
                            ),
                        );
                    }
                }
            }
        }
    }
}

/// Parse an IPv4/IPv6 address or CIDR block. Returns the four IPv4 octets when the
/// value is IPv4 (so the private-range check can run), or an empty vec for IPv6.
fn parse_cidr(s: &str) -> Option<Vec<u8>> {
    let (addr, prefix) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    if addr.contains(':') {
        // IPv6: accept the documented shape without full canonicalisation.
        if let Some(p) = prefix {
            let n: u32 = p.parse().ok()?;
            if n > 128 {
                return None;
            }
        }
        let groups: Vec<&str> = addr.split(':').collect();
        if groups.len() < 3 || groups.len() > 8 {
            return None;
        }
        for g in groups {
            if g.is_empty() {
                continue; // `::` compression
            }
            if g.len() > 4 || !g.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
        }
        return Some(Vec::new());
    }
    if let Some(p) = prefix {
        let n: u32 = p.parse().ok()?;
        if n > 32 {
            return None;
        }
    }
    let octets: Vec<&str> = addr.split('.').collect();
    if octets.len() != 4 {
        return None;
    }
    let mut out = Vec::with_capacity(4);
    for o in octets {
        if o.is_empty() || o.len() > 3 || !o.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        out.push(o.parse::<u8>().ok()?);
    }
    Some(out)
}

fn is_private_v4(o: &[u8]) -> bool {
    if o.len() != 4 {
        return false;
    }
    o[0] == 10
        || (o[0] == 172 && (16..=31).contains(&o[1]))
        || (o[0] == 192 && o[1] == 168)
        || o[0] == 127
        || (o[0] == 169 && o[1] == 254)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Lint a policy and render the report.
///
/// * `policy_type` — `identity` | `resource` | `trust` | `scp`
/// * `format` — `text` | `json` | `csv`
/// * `min_severity` — `low` | `medium` | `high`; a **display** filter only
/// * `ignore` — comma-separated rule codes removed from the report *and* the verdict
pub fn render(
    policy: &str,
    policy_type: &str,
    format: &str,
    min_severity: &str,
    ignore: &str,
) -> Result<String, String> {
    let ptype = PolicyType::parse(policy_type).ok_or_else(|| {
        format!("unknown policy_type \"{policy_type}\" — use identity, resource, trust or scp")
    })?;
    let fmt = format.trim().to_ascii_lowercase();
    let fmt = if fmt.is_empty() {
        "text".to_string()
    } else {
        fmt
    };
    if !matches!(fmt.as_str(), "text" | "json" | "csv") {
        return Err(format!(
            "unknown format \"{format}\" — use text, json or csv"
        ));
    }
    let min = if min_severity.trim().is_empty() {
        Severity::Low
    } else {
        Severity::parse(min_severity).ok_or_else(|| {
            format!("unknown min_severity \"{min_severity}\" — use low, medium or high")
        })?
    };
    let ignored = parse_ignore(ignore)?;

    let mut report = lint(policy, ptype)?;
    let before = report.findings.len();
    report.findings.retain(|f| !ignored.contains(&f.code));
    let suppressed = before - report.findings.len();

    let shown: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity >= min)
        .collect();
    let hidden = report.findings.len() - shown.len();

    Ok(match fmt.as_str() {
        "json" => render_json(&report, &shown, suppressed, hidden, min),
        "csv" => render_csv(&shown),
        _ => render_text(&report, &shown, suppressed, hidden, min),
    })
}

/// Validate the `ignore` list. An unknown code is an error, never a silent no-op.
fn parse_ignore(ignore: &str) -> Result<Vec<&'static str>, String> {
    let mut out = Vec::new();
    for raw in ignore.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        let upper = token.to_ascii_uppercase();
        match RULE_CODES.iter().find(|c| **c == upper) {
            Some(code) => {
                if !out.contains(code) {
                    out.push(*code);
                }
            }
            None => {
                return Err(format!(
                    "unknown ignore rule code \"{token}\". Valid codes: {}",
                    RULE_CODES.join(", ")
                ))
            }
        }
    }
    Ok(out)
}

fn render_text(
    report: &Report,
    shown: &[&Finding],
    suppressed: usize,
    hidden: usize,
    min: Severity,
) -> String {
    let verdict = report.verdict();
    let total = report.findings.len();
    let mut out = String::new();
    if total == 0 {
        out.push_str("CLEAN — no findings\n");
    } else {
        out.push_str(&format!(
            "{} — {} ({} high, {} medium, {} low)\n",
            verdict.to_uppercase(),
            plural(total, "finding", "findings"),
            report.count(Severity::High),
            report.count(Severity::Medium),
            report.count(Severity::Low),
        ));
    }
    out.push_str(&format!(
        "{} policy · {} · {} characters (managed-policy limit {})\n",
        report.policy_type.as_str(),
        plural(report.statements, "statement", "statements"),
        report.characters,
        MAX_POLICY_CHARS,
    ));

    if !shown.is_empty() {
        out.push('\n');
        for f in shown {
            out.push_str(&format!(
                "[{}] {} — {}{}\n  {}\n\n",
                f.severity.as_str(),
                f.code,
                f.path,
                f.line.map(|l| format!(" (line {l})")).unwrap_or_default(),
                f.message,
            ));
        }
        out.pop();
    } else if total > 0 {
        out.push_str(&format!(
            "\nNo findings at or above the {} severity threshold.\n",
            min.as_str()
        ));
    }

    let mut notes = Vec::new();
    if hidden > 0 {
        notes.push(format!(
            "{} hidden by min_severity={}.",
            plural(hidden, "finding", "findings"),
            min.as_str()
        ));
    }
    if suppressed > 0 {
        notes.push(format!(
            "{} suppressed by ignore.",
            plural(suppressed, "finding", "findings")
        ));
    }
    if report.truncated {
        notes.push(format!(
            "Report truncated at {MAX_FINDINGS} findings; fix these and run again."
        ));
    }
    if !notes.is_empty() {
        out.push('\n');
        for n in notes {
            out.push_str(&n);
            out.push('\n');
        }
    }
    out
}

fn render_json(
    report: &Report,
    shown: &[&Finding],
    suppressed: usize,
    hidden: usize,
    min: Severity,
) -> String {
    let findings: Vec<Value> = shown
        .iter()
        .map(|f| {
            serde_json::json!({
                "code": f.code,
                "severity": f.severity.as_str(),
                "path": f.path,
                "line": f.line,
                "message": f.message,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "verdict": report.verdict(),
        "policy_type": report.policy_type.as_str(),
        "summary": {
            "total": report.findings.len(),
            "high": report.count(Severity::High),
            "medium": report.count(Severity::Medium),
            "low": report.count(Severity::Low),
            "shown": shown.len(),
            "hidden_by_min_severity": hidden,
            "suppressed_by_ignore": suppressed,
            "min_severity": min.as_str(),
            "statements": report.statements,
            "characters": report.characters,
            "character_limit": MAX_POLICY_CHARS,
            "truncated": report.truncated,
        },
        "findings": findings,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn render_csv(shown: &[&Finding]) -> String {
    let mut out = String::from("severity,code,path,line,message\n");
    for f in shown {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_field(f.severity.as_str()),
            csv_field(f.code),
            csv_field(&f.path),
            f.line.map(|l| l.to_string()).unwrap_or_default(),
            csv_field(&f.message),
        ));
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const ADMIN: &str = r#"{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "*",
      "Resource": "*"
    }
  ]
}"#;

    const CLEAN: &str = r#"{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "ReadAppBucket",
      "Effect": "Allow",
      "Action": ["s3:GetObject"],
      "Resource": ["arn:aws:s3:::app-bucket/*"]
    }
  ]
}"#;

    fn codes(policy: &str, ptype: PolicyType) -> Vec<&'static str> {
        lint(policy, ptype)
            .expect("lints")
            .findings
            .iter()
            .map(|f| f.code)
            .collect()
    }

    fn find<'a>(report: &'a Report, code: &str) -> &'a Finding {
        report
            .findings
            .iter()
            .find(|f| f.code == code)
            .unwrap_or_else(|| panic!("expected a {code} finding, got {:?}", report.findings))
    }

    // --- happy path ---------------------------------------------------------

    #[test]
    fn least_privilege_policy_is_clean() {
        let r = lint(CLEAN, PolicyType::Identity).unwrap();
        assert_eq!(r.findings, vec![], "no findings on a scoped policy");
        assert_eq!(r.verdict(), "clean");
        assert_eq!(r.statements, 1);
    }

    #[test]
    fn clean_text_report_has_verdict_and_summary() {
        let out = render(CLEAN, "identity", "text", "low", "").unwrap();
        assert!(out.starts_with("CLEAN — no findings\n"), "{out}");
        assert!(out.contains("identity policy · 1 statement · "), "{out}");
        assert!(out.contains("(managed-policy limit 6144)"), "{out}");
    }

    #[test]
    fn admin_policy_is_unsafe_with_line_and_path() {
        let r = lint(ADMIN, PolicyType::Identity).unwrap();
        assert_eq!(r.verdict(), "unsafe");
        let f = find(&r, "ADMIN-STAR");
        assert_eq!(f.severity, Severity::High);
        assert_eq!(f.path, "$.Statement[0].Action");
        assert_eq!(f.line, Some(6));
    }

    #[test]
    fn admin_text_output_is_shaped_as_documented() {
        let out = render(ADMIN, "identity", "text", "low", "").unwrap();
        assert!(
            out.starts_with("UNSAFE — 1 finding (1 high, 0 medium, 0 low)\n"),
            "{out}"
        );
        assert!(
            out.contains("[high] ADMIN-STAR — $.Statement[0].Action (line 6)"),
            "{out}"
        );
    }

    // --- structural checks --------------------------------------------------

    #[test]
    fn missing_version_and_statement_are_reported() {
        let c = codes(r#"{}"#, PolicyType::Identity);
        assert!(c.contains(&"MISSING-STATEMENT"), "{c:?}");
        assert!(c.contains(&"MISSING-VERSION"), "{c:?}");
    }

    #[test]
    fn empty_statement_array_is_reported() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"MISSING-STATEMENT"), "{c:?}");
    }

    #[test]
    fn bad_version_and_legacy_version() {
        assert!(codes(
            r#"{"Version":"2019-01-01","Statement":{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}}"#,
            PolicyType::Identity
        )
        .contains(&"INVALID-VERSION"));
        assert!(codes(
            r#"{"Version":"2008-10-17","Statement":{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}}"#,
            PolicyType::Identity
        )
        .contains(&"LEGACY-VERSION"));
    }

    #[test]
    fn missing_and_invalid_effect() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"MISSING-EFFECT"), "{c:?}");
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"INVALID-EFFECT"), "{c:?}");
    }

    #[test]
    fn missing_action_and_resource() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"MISSING-ACTION"), "{c:?}");
        assert!(c.contains(&"MISSING-RESOURCE"), "{c:?}");
    }

    #[test]
    fn element_conflict_and_unknown_keys() {
        let c = codes(
            r#"{"Version":"2012-10-17","Effects":"x","Statement":[{"Effect":"Deny","Action":"s3:*","NotAction":"s3:GetObject","Resource":"*","Sid":"a b","Extra":1}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"ELEMENT-CONFLICT"), "{c:?}");
        assert!(c.contains(&"UNKNOWN-ELEMENT"), "{c:?}");
        assert!(c.contains(&"INVALID-SID"), "{c:?}");
    }

    #[test]
    fn duplicate_sid_is_reported() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[
                {"Sid":"Same","Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"},
                {"Sid":"Same","Effect":"Allow","Action":"s3:PutObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"DUPLICATE-SID"), "{c:?}");
    }

    #[test]
    fn statement_may_be_a_single_object() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}}"#,
            PolicyType::Identity,
        )
        .unwrap();
        assert_eq!(r.statements, 1);
        assert_eq!(r.verdict(), "clean");
    }

    // --- wildcards / Not* ---------------------------------------------------

    #[test]
    fn action_star_with_scoped_resource_is_not_admin() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"*","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"ACTION-STAR"), "{c:?}");
        assert!(!c.contains(&"ADMIN-STAR"), "{c:?}");
    }

    #[test]
    fn resource_star_and_service_wildcard() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"ec2:*","Resource":"*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"RESOURCE-STAR"), "{c:?}");
        assert!(c.contains(&"SERVICE-ACTION-STAR"), "{c:?}");
    }

    #[test]
    fn resource_effectively_star_is_caught() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"ec2:DescribeTags","Resource":"arn:*:*:*:*:*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"RESOURCE-EFFECTIVELY-STAR"), "{c:?}");
    }

    #[test]
    fn not_action_allow_is_high_but_not_action_deny_is_fine() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","NotAction":"iam:*","Resource":"*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"NOT-ACTION-ALLOW"), "{c:?}");
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Deny","NotAction":"iam:*","Resource":"*"}]}"#,
            PolicyType::Identity,
        );
        assert!(
            !c.contains(&"NOT-ACTION-ALLOW"),
            "Deny + NotAction is the idiom: {c:?}"
        );
    }

    #[test]
    fn not_resource_allow_is_reported() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","NotResource":"arn:aws:s3:::secret/*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"NOT-RESOURCE-ALLOW"), "{c:?}");
    }

    #[test]
    fn not_principal_allow_is_reported() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","NotPrincipal":{"AWS":"arn:aws:iam::123456789012:root"},"Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Resource,
        );
        assert!(c.contains(&"NOT-PRINCIPAL-ALLOW"), "{c:?}");
    }

    // --- principals ---------------------------------------------------------

    #[test]
    fn public_bucket_policy_is_unsafe() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":"*","Action":"s3:GetObject","Resource":"arn:aws:s3:::public/*"}]}"#,
            PolicyType::Resource,
        )
        .unwrap();
        let f = find(&r, "PRINCIPAL-STAR");
        assert_eq!(f.severity, Severity::High);
        assert_eq!(r.verdict(), "unsafe");
    }

    #[test]
    fn principal_star_is_downgraded_when_a_condition_scopes_it() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":"*","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"StringEquals":{"aws:PrincipalOrgID":"o-abc123"}}}]}"#,
            PolicyType::Resource,
        )
        .unwrap();
        assert_eq!(find(&r, "PRINCIPAL-STAR").severity, Severity::Medium);
    }

    #[test]
    fn wildcard_inside_principal_and_unknown_principal_type() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"AWS":"arn:aws:iam::*:root","Robot":"x"},"Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Resource,
        );
        assert!(c.contains(&"WILDCARD-PRINCIPAL-KEY"), "{c:?}");
        assert!(c.contains(&"INVALID-PRINCIPAL"), "{c:?}");
    }

    #[test]
    fn identity_policy_rejects_principal_and_resource_policy_requires_one() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":"*","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"PRINCIPAL-UNSUPPORTED"), "{c:?}");
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Resource,
        );
        assert!(c.contains(&"MISSING-PRINCIPAL"), "{c:?}");
    }

    // --- sensitive action families -----------------------------------------

    #[test]
    fn pass_role_star_is_its_own_finding() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["iam:PassRole"],"Resource":"*"}]}"#,
            PolicyType::Identity,
        )
        .unwrap();
        let f = find(&r, "PASS-ROLE-STAR");
        assert_eq!(f.severity, Severity::High);
        assert_eq!(f.path, "$.Statement[0].Action[0]");
    }

    #[test]
    fn privilege_escalation_family_fires_on_unconstrained_resource() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["iam:AttachRolePolicy"],"Resource":"*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"PRIV-ESC"), "{c:?}");
    }

    #[test]
    fn sensitive_action_scoped_to_an_arn_is_not_flagged() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["secretsmanager:GetSecretValue"],"Resource":"arn:aws:secretsmanager:us-east-1:123456789012:secret:prod/db-AbCdEf"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.is_empty(), "scoped sensitive action is the point: {c:?}");
    }

    #[test]
    fn credential_and_exfil_families_are_distinct() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["iam:CreateAccessKey","s3:GetObject","s3:PutBucketPolicy"],"Resource":"*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"CREDENTIAL-EXPOSURE"), "{c:?}");
        assert!(c.contains(&"DATA-EXFIL"), "{c:?}");
        assert!(c.contains(&"RESOURCE-EXPOSURE"), "{c:?}");
    }

    #[test]
    fn service_wildcard_reaches_the_family_lists() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"iam:*","Resource":"*"}]}"#,
            PolicyType::Identity,
        )
        .unwrap();
        assert!(find(&r, "PRIV-ESC")
            .message
            .contains("privilege-escalation actions"));
    }

    #[test]
    fn service_scoped_arn_still_counts_as_unconstrained() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::*"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"DATA-EXFIL"), "{c:?}");
    }

    // --- policy-type awareness ---------------------------------------------

    #[test]
    fn trust_policy_rejects_resource_and_non_sts_action() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Trust,
        );
        assert!(c.contains(&"TRUST-POLICY-RESOURCE"), "{c:?}");
        assert!(c.contains(&"TRUST-POLICY-ACTION"), "{c:?}");
    }

    #[test]
    fn clean_trust_policy_has_no_findings() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}"#,
            PolicyType::Trust,
        )
        .unwrap();
        assert_eq!(r.findings, vec![]);
    }

    #[test]
    fn scp_rules_fire_on_principal_wildcard_and_allow_resource() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":"*","Action":"s3:Get*Object","Resource":"arn:aws:s3:::b/*"}]}"#,
            PolicyType::Scp,
        );
        assert!(c.contains(&"SCP-PRINCIPAL"), "{c:?}");
        assert!(c.contains(&"SCP-ACTION-WILDCARD"), "{c:?}");
        assert!(c.contains(&"SCP-ALLOW-RESOURCE"), "{c:?}");
    }

    // --- conditions ---------------------------------------------------------

    #[test]
    fn unknown_condition_operator_is_high() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"StringEqual":{"aws:username":"bob"}}}]}"#,
            PolicyType::Identity,
        )
        .unwrap();
        assert_eq!(
            find(&r, "INVALID-CONDITION-OPERATOR").severity,
            Severity::High
        );
    }

    #[test]
    fn set_prefix_and_if_exists_operators_are_accepted() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"ForAllValues:StringEqualsIfExists":{"aws:TagKeys":"env"}}}]}"#,
            PolicyType::Identity,
        )
        .unwrap();
        assert_eq!(r.findings, vec![]);
    }

    #[test]
    fn null_with_if_exists_is_rejected() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"NullIfExists":{"aws:username":"true"}}}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"NULL-IF-EXISTS"), "{c:?}");
    }

    #[test]
    fn bad_cidr_and_private_source_ip() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"IpAddress":{"aws:SourceIp":"10.0.0.0/8"}}}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"PRIVATE-SOURCE-IP"), "{c:?}");
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"IpAddress":{"aws:SourceIp":"203.0.113.0/64"}}}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"INVALID-CIDR"), "{c:?}");
    }

    #[test]
    fn public_cidr_and_ipv6_are_accepted() {
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"IpAddress":{"aws:SourceIp":["203.0.113.0/24","2001:db8::/32"]}}}]}"#,
            PolicyType::Identity,
        )
        .unwrap();
        assert_eq!(r.findings, vec![]);
    }

    #[test]
    fn empty_condition_blocks_and_bad_keys() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{}}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"EMPTY-CONDITION"), "{c:?}");
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"StringEquals":{"username":"bob"}}}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"INVALID-CONDITION-KEY"), "{c:?}");
    }

    #[test]
    fn bool_condition_value_is_checked() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"Bool":{"aws:SecureTransport":"yes"}}}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"INVALID-CONDITION-VALUE"), "{c:?}");
    }

    // --- ARNs, actions, variables, size ------------------------------------

    #[test]
    fn arn_shape_problems_are_reported() {
        for (policy, why) in [
            (
                r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:b"}]}"#,
                "too few fields",
            ),
            (
                r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:amazon:s3:::b/*"}]}"#,
                "bad partition",
            ),
            (
                r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:S3:::b/*"}]}"#,
                "uppercase service",
            ),
            (
                r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3::12345:b/*"}]}"#,
                "short account",
            ),
            (
                r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"my-bucket"}]}"#,
                "not an ARN",
            ),
        ] {
            assert!(
                codes(policy, PolicyType::Identity).contains(&"INVALID-ARN"),
                "expected INVALID-ARN for {why}"
            );
        }
    }

    #[test]
    fn action_grammar_problems_are_reported() {
        for policy in [
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"S3:GetObject","Resource":"arn:aws:s3:::b/*"}]}"#,
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:","Resource":"arn:aws:s3:::b/*"}]}"#,
        ] {
            assert!(
                codes(policy, PolicyType::Identity).contains(&"INVALID-ACTION"),
                "{policy}"
            );
        }
    }

    #[test]
    fn policy_variable_syntax_is_checked() {
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/${aws:username"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"INVALID-VARIABLE"), "{c:?}");
        let c = codes(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/${ aws:username }"}]}"#,
            PolicyType::Identity,
        );
        assert!(c.contains(&"INVALID-VARIABLE"), "{c:?}");
        let r = lint(
            r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/${aws:username}/*"}]}"#,
            PolicyType::Identity,
        )
        .unwrap();
        assert_eq!(r.findings, vec![], "a well-formed variable is fine");
    }

    #[test]
    fn oversized_policy_is_flagged() {
        let filler = "a".repeat(6200);
        let policy = format!(
            r#"{{"Version":"2012-10-17","Statement":[{{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::{filler}/*"}}]}}"#
        );
        assert!(codes(&policy, PolicyType::Identity).contains(&"POLICY-SIZE"));
    }

    // --- rendering ----------------------------------------------------------

    #[test]
    fn json_output_carries_verdict_summary_and_findings() {
        let out = render(ADMIN, "identity", "json", "low", "").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["verdict"], "unsafe");
        assert_eq!(v["policy_type"], "identity");
        assert_eq!(v["summary"]["total"], 1);
        assert_eq!(v["summary"]["high"], 1);
        assert_eq!(v["findings"][0]["code"], "ADMIN-STAR");
        assert_eq!(v["findings"][0]["path"], "$.Statement[0].Action");
        assert_eq!(v["findings"][0]["line"], 6);
    }

    #[test]
    fn csv_output_has_a_header_and_quotes_messages() {
        let out = render(ADMIN, "identity", "csv", "low", "").unwrap();
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap(), "severity,code,path,line,message");
        let row = lines.next().unwrap();
        assert!(
            row.starts_with("high,ADMIN-STAR,$.Statement[0].Action,6,\""),
            "{row}"
        );
        assert!(row.contains("\"\"*\"\""), "inner quotes are doubled: {row}");
    }

    #[test]
    fn min_severity_hides_findings_but_not_the_verdict() {
        let policy =
            r#"{"Statement":[{"Effect":"Allow","Action":"ec2:DescribeTags","Resource":"*"}]}"#;
        let out = render(policy, "identity", "text", "high", "").unwrap();
        assert!(
            out.starts_with("REVIEW — 2 findings (0 high, 1 medium, 1 low)"),
            "{out}"
        );
        assert!(
            out.contains("No findings at or above the high severity threshold."),
            "{out}"
        );
        assert!(
            out.contains("2 findings hidden by min_severity=high."),
            "{out}"
        );
    }

    #[test]
    fn ignore_changes_the_verdict() {
        let out = render(ADMIN, "identity", "text", "low", "admin-star").unwrap();
        assert!(out.starts_with("CLEAN — no findings"), "{out}");
        assert!(out.contains("1 finding suppressed by ignore."), "{out}");
    }

    #[test]
    fn ignore_accepts_several_codes_and_ignores_blanks() {
        let policy =
            r#"{"Statement":[{"Effect":"Allow","Action":"ec2:DescribeTags","Resource":"*"}]}"#;
        let out = render(
            policy,
            "identity",
            "text",
            "low",
            " resource-star , ,MISSING-VERSION",
        )
        .unwrap();
        assert!(out.starts_with("CLEAN — no findings"), "{out}");
    }

    // --- errors -------------------------------------------------------------

    #[test]
    fn unknown_ignore_code_is_an_error() {
        let err = render(ADMIN, "identity", "text", "low", "NOPE").unwrap_err();
        assert!(
            err.starts_with("unknown ignore rule code \"NOPE\""),
            "{err}"
        );
        assert!(
            err.contains("ADMIN-STAR"),
            "the error lists the valid codes: {err}"
        );
    }

    #[test]
    fn invalid_json_reports_line_and_column() {
        let err = render("{\"Version\": }", "identity", "text", "low", "").unwrap_err();
        assert!(err.starts_with("invalid JSON policy:"), "{err}");
        assert!(err.contains("line 1"), "{err}");
    }

    #[test]
    fn non_object_policy_is_an_error() {
        let err = render("[1,2,3]", "identity", "text", "low", "").unwrap_err();
        assert_eq!(
            err,
            "policy must be a JSON object with Version and Statement elements"
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(render("   ", "identity", "text", "low", "")
            .unwrap_err()
            .starts_with("policy is empty"));
    }

    #[test]
    fn unknown_enum_values_are_errors() {
        assert!(render(ADMIN, "iam", "text", "low", "")
            .unwrap_err()
            .starts_with("unknown policy_type"));
        assert!(render(ADMIN, "identity", "yaml", "low", "")
            .unwrap_err()
            .starts_with("unknown format"));
        assert!(render(ADMIN, "identity", "text", "critical", "")
            .unwrap_err()
            .starts_with("unknown min_severity"));
    }

    #[test]
    fn blank_optional_params_fall_back_to_defaults() {
        let out = render(ADMIN, "identity", "", "", "").unwrap();
        assert!(out.starts_with("UNSAFE — 1 finding"), "{out}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_CHARS + 1);
        assert!(render(&big, "identity", "text", "low", "")
            .unwrap_err()
            .starts_with("policy is too large"));
    }

    // --- helpers ------------------------------------------------------------

    #[test]
    fn glob_matching_follows_iam_wildcards() {
        assert!(glob_match("s3:*", "s3:GetObject"));
        assert!(glob_match("iam:*", "iam:PassRole"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("s3:Get?bject", "s3:GetObject"));
        assert!(!glob_match("s3:Get*", "ec2:GetConsoleOutput"));
        assert!(
            glob_match("S3:GETOBJECT", "s3:GetObject"),
            "case-insensitive"
        );
    }

    #[test]
    fn line_index_locates_nested_paths() {
        let idx = LineIndex::build(ADMIN);
        assert_eq!(idx.line("$"), Some(1));
        assert_eq!(idx.line("$.Version"), Some(2));
        assert_eq!(idx.line("$.Statement[0].Effect"), Some(5));
        assert_eq!(idx.line("$.Statement[0].Resource"), Some(7));
    }

    #[test]
    fn every_emitted_code_is_in_the_rule_table() {
        // Any code the linter can emit must be ignorable — otherwise `ignore` would
        // reject a code that appears in the report.
        let policies: [(&str, PolicyType); 6] = [
            (ADMIN, PolicyType::Identity),
            (
                r#"{"Bogus":1,"Version":"9","Statement":[{"Effect":"maybe","NotAction":"x","NotResource":"y","Principal":"*","Sid":"a b","Condition":{"Nope":{"bad key":[]}}}]}"#,
                PolicyType::Identity,
            ),
            (
                r#"{"Statement":[{"Effect":"Allow","Action":["iam:PassRole","iam:CreateAccessKey","s3:GetObject","s3:PutBucketPolicy","iam:AttachRolePolicy"],"Resource":"*"}]}"#,
                PolicyType::Identity,
            ),
            (
                r#"{"Statement":[{"Effect":"Allow","Principal":{"AWS":"arn:aws:iam::*:root"},"Action":"s3:GetObject","Resource":"arn:aws:s3:b"}]}"#,
                PolicyType::Resource,
            ),
            (
                r#"{"Statement":[{"Effect":"Allow","Principal":"*","Action":"s3:Get*Object","Resource":"arn:aws:s3:::b/*"}]}"#,
                PolicyType::Scp,
            ),
            (
                r#"{"Statement":[{"Effect":"Allow","Action":"s3:GetObject","Resource":"arn:aws:s3:::b/*","Condition":{"NullIfExists":{"aws:x":"1"},"IpAddress":{"aws:SourceIp":"999.1.1.1"}}}]}"#,
                PolicyType::Trust,
            ),
        ];
        let mut seen = 0usize;
        for (p, t) in policies {
            for f in lint(p, t).unwrap().findings {
                assert!(
                    RULE_CODES.contains(&f.code),
                    "{} missing from RULE_CODES",
                    f.code
                );
                assert!(!f.path.is_empty());
                assert!(f.line.is_some(), "{} has no source line", f.code);
                seen += 1;
            }
        }
        assert!(
            seen > 25,
            "the fixtures should exercise a broad slice of the rules"
        );
    }

    #[test]
    fn rule_codes_are_unique() {
        let mut sorted = RULE_CODES.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), before, "duplicate rule code");
    }
}
