//! release-from-commits core — compute the next semantic version AND grouped
//! release notes from a pasted Conventional Commits log. Pure and
//! dependency-free, shared by the chat/CLI block and the browser page.
//!
//! No git access: the caller pastes the log (`git log --oneline`,
//! `git log --pretty=%B`, plain bullet lists, …). Parsing rules are documented
//! on `parse_log`.

/// Largest accepted commit log (bytes).
pub const MAX_INPUT_BYTES: usize = 1_048_576;
/// Largest accepted number of parsed commits.
pub const MAX_COMMITS: usize = 5_000;

/// Semantic-version increment implied by a commit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bump {
    None,
    Patch,
    Minor,
    Major,
}

impl Bump {
    pub fn label(self) -> &'static str {
        match self {
            Bump::None => "none",
            Bump::Patch => "patch",
            Bump::Minor => "minor",
            Bump::Major => "major",
        }
    }
}

/// A parsed semantic version, keeping whatever tag prefix the input carried
/// (`v`, `web-v`, `@acme/cli@`, …) so the answer can be pasted straight back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub prefix: String,
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Vec<String>,
    pub build: String,
}

impl Version {
    /// Render without the tag prefix, e.g. `1.5.0-rc.1`.
    pub fn bare(&self) -> String {
        let mut s = format!("{}.{}.{}", self.major, self.minor, self.patch);
        if !self.pre.is_empty() {
            s.push('-');
            s.push_str(&self.pre.join("."));
        }
        if !self.build.is_empty() {
            s.push('+');
            s.push_str(&self.build);
        }
        s
    }
    /// Render with the tag prefix the input used, e.g. `v1.5.0-rc.1`.
    pub fn tagged(&self) -> String {
        format!("{}{}", self.prefix, self.bare())
    }
}

/// One commit taken from the pasted log.
#[derive(Debug, Clone)]
pub struct Commit {
    /// Short hash if the log carried one (`git log --oneline`), else empty.
    pub hash: String,
    /// Canonical lowercase type (`feat`, `fix`, …); empty for a line that is
    /// not a Conventional Commit.
    pub ctype: String,
    /// Scope inside `type(scope):`, else empty.
    pub scope: String,
    pub breaking: bool,
    pub subject: String,
    /// Text after a `BREAKING CHANGE:` footer, else empty.
    pub breaking_desc: String,
}

// ---------------------------------------------------------------- versions

pub fn parse_version(raw: &str) -> Result<Version, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(
            "current version is empty: enter the version this commit log follows, such as 1.4.2 or v1.4.2"
                .to_string(),
        );
    }
    if s.chars().any(|c| c.is_whitespace()) {
        return Err(format!(
            "invalid current version {s:?}: it must be a single token like 1.4.2 or v1.4.2, with no spaces"
        ));
    }
    let idx = s.find(|c: char| c.is_ascii_digit()).ok_or_else(|| {
        format!("invalid current version {s:?}: expected MAJOR.MINOR.PATCH, for example 1.4.2 or v1.4.2")
    })?;
    let prefix = s[..idx].to_string();
    let rest = &s[idx..];

    let (core_pre, build) = match rest.split_once('+') {
        Some((a, b)) => (a, b.to_string()),
        None => (rest, String::new()),
    };
    let (core, pre_raw) = match core_pre.split_once('-') {
        Some((a, b)) => (a, b),
        None => (core_pre, ""),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return Err(format!(
            "invalid current version {s:?}: expected three dot-separated numbers like 1.4.2, got {}",
            parts.len()
        ));
    }
    let mut nums = [0u64; 3];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "invalid current version {s:?}: {p:?} is not a number — expected MAJOR.MINOR.PATCH like 1.4.2"
            ));
        }
        nums[i] = p.parse::<u64>().map_err(|_| {
            format!("invalid current version {s:?}: {p:?} is too large for a version number")
        })?;
    }
    let mut pre: Vec<String> = Vec::new();
    if !pre_raw.is_empty() {
        for id in pre_raw.split('.') {
            if id.is_empty() {
                return Err(format!(
                    "invalid current version {s:?}: the pre-release part has an empty identifier"
                ));
            }
            pre.push(id.to_string());
        }
    }
    Ok(Version {
        prefix,
        major: nums[0],
        minor: nums[1],
        patch: nums[2],
        pre,
        build,
    })
}

/// node-semver style increment: an existing pre-release is *finalised* when it
/// already carries the requested bump (`2.0.0-rc.1` + major → `2.0.0`).
fn inc_finalize(v: &Version, b: Bump) -> Version {
    let mut n = v.clone();
    n.build = String::new();
    match b {
        Bump::Major => {
            if n.minor != 0 || n.patch != 0 || n.pre.is_empty() {
                n.major += 1;
            }
            n.minor = 0;
            n.patch = 0;
            n.pre.clear();
        }
        Bump::Minor => {
            if n.patch != 0 || n.pre.is_empty() {
                n.minor += 1;
            }
            n.patch = 0;
            n.pre.clear();
        }
        Bump::Patch => {
            if n.pre.is_empty() {
                n.patch += 1;
            }
            n.pre.clear();
        }
        Bump::None => {}
    }
    n
}

/// Drop any pre-release, then apply a plain stable increment.
fn inc_stable(v: &Version, b: Bump) -> Version {
    let mut n = v.clone();
    n.pre.clear();
    n.build = String::new();
    match b {
        Bump::Major => {
            n.major += 1;
            n.minor = 0;
            n.patch = 0;
        }
        Bump::Minor => {
            n.minor += 1;
            n.patch = 0;
        }
        Bump::Patch => {
            n.patch += 1;
        }
        Bump::None => {}
    }
    n
}

/// Stay on (or move to) a pre-release line: bump the counter when the pending
/// pre-release already covers the requested bump, else start `<id>.0` on the
/// new base.
fn inc_prerelease(v: &Version, b: Bump, id: &str) -> Version {
    if b == Bump::None {
        return v.clone();
    }
    let target = inc_finalize(v, b);
    let mut n = target.clone();
    let same_base = !v.pre.is_empty()
        && target.major == v.major
        && target.minor == v.minor
        && target.patch == v.patch;
    if same_base {
        let mut pre = v.pre.clone();
        let bumped = match pre.last_mut() {
            Some(last) => match last.parse::<u64>() {
                Ok(k) => {
                    *last = (k + 1).to_string();
                    true
                }
                Err(_) => false,
            },
            None => false,
        };
        if !bumped {
            pre.push("0".to_string());
        }
        n.pre = pre;
    } else {
        n.pre = vec![id.to_string(), "0".to_string()];
    }
    n
}

// ---------------------------------------------------------------- commits

const TRAILER_KEYS: [&str; 18] = [
    "signed-off-by",
    "co-authored-by",
    "coauthored-by",
    "reviewed-by",
    "acked-by",
    "tested-by",
    "reported-by",
    "suggested-by",
    "helped-by",
    "cc",
    "closes",
    "close",
    "closed",
    "fixes",
    "resolves",
    "resolved",
    "refs",
    "references",
];

fn is_breaking_footer(line: &str) -> Option<&str> {
    let t = line.trim_start();
    for key in ["BREAKING CHANGES", "BREAKING CHANGE", "BREAKING-CHANGE"] {
        if t.len() >= key.len()
            && t.is_char_boundary(key.len())
            && t[..key.len()].eq_ignore_ascii_case(key)
        {
            let rest = &t[key.len()..];
            let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix(" #"))?;
            return Some(rest.trim());
        }
    }
    None
}

fn is_trailer(line: &str) -> bool {
    let t = line.trim_start();
    match t.split_once(':') {
        Some((key, _)) => TRAILER_KEYS.contains(&key.trim().to_ascii_lowercase().as_str()),
        None => false,
    }
}

/// Drop `git log`'s default envelope lines so a full paste works too.
fn is_log_metadata(line: &str) -> bool {
    let t = line.trim();
    for key in [
        "Author:",
        "AuthorDate:",
        "Date:",
        "Commit:",
        "CommitDate:",
        "Merge:",
    ] {
        if t.starts_with(key) {
            return true;
        }
    }
    if let Some(rest) = t.strip_prefix("commit ") {
        let tok = rest.split_whitespace().next().unwrap_or("");
        if tok.len() >= 7 && tok.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
    }
    false
}

fn strip_bullet(line: &str) -> &str {
    let t = line.trim_start();
    for p in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = t.strip_prefix(p) {
            return rest.trim_start();
        }
    }
    t
}

/// Peel a leading short hash (`a1b2c3d feat: …`, `[a1b2c3d] feat: …`).
fn split_hash(line: &str) -> (String, &str) {
    let t = line.trim_start();
    let (candidate, rest) = match t.split_once(char::is_whitespace) {
        Some((a, b)) => (a, b.trim_start()),
        None => return (String::new(), t),
    };
    if rest.is_empty() {
        return (String::new(), t);
    }
    let bare = candidate.trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')');
    if bare.len() >= 7 && bare.len() <= 40 && bare.chars().all(|c| c.is_ascii_hexdigit()) {
        return (bare.to_ascii_lowercase(), rest);
    }
    (String::new(), t)
}

/// Parse `type(scope)!: subject`. Returns None when the line is not a
/// Conventional Commit header.
fn parse_header(line: &str) -> Option<(String, String, bool, String)> {
    let (head, subject) = line.split_once(':')?;
    let subject = subject.trim();
    if subject.is_empty() {
        return None;
    }
    let mut head = head.trim();
    let breaking = head.ends_with('!');
    if breaking {
        head = head[..head.len() - 1].trim_end();
    }
    let (ty, scope) = match head.split_once('(') {
        Some((t, s)) => {
            let s = s.strip_suffix(')')?;
            (t.trim(), s.trim().to_string())
        }
        None => (head, String::new()),
    };
    if ty.is_empty() || !ty.chars().next()?.is_ascii_alphabetic() {
        return None;
    }
    if !ty
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    if TRAILER_KEYS.contains(&ty.to_ascii_lowercase().as_str()) {
        return None;
    }
    Some((canonical_type(ty), scope, breaking, subject.to_string()))
}

/// Fold common spellings onto one canonical type so grouping and the bump
/// lists behave predictably.
pub fn canonical_type(t: &str) -> String {
    let l = t.trim().to_ascii_lowercase();
    let c = match l.as_str() {
        "feat" | "feature" | "features" => "feat",
        "fix" | "bugfix" | "bugfixes" | "bug" => "fix",
        "perf" | "performance" => "perf",
        "doc" | "docs" | "documentation" => "docs",
        "test" | "tests" | "testing" => "test",
        "style" | "styles" | "formatting" => "style",
        "chore" | "chores" => "chore",
        "refactor" | "refactoring" => "refactor",
        "revert" | "reverts" => "revert",
        "build" => "build",
        "ci" | "cicd" => "ci",
        "dep" | "deps" | "dependencies" => "deps",
        "security" | "sec" => "security",
        other => other,
    };
    c.to_string()
}

fn commit_from_block(lines: &[&str]) -> Option<Commit> {
    let first = lines.first()?;
    let stripped = strip_bullet(first);
    let (hash, header_line) = split_hash(stripped);
    let (ctype, scope, mut breaking, subject) = match parse_header(header_line) {
        Some(v) => v,
        None => {
            let subject = header_line.trim().to_string();
            if subject.is_empty() {
                return None;
            }
            (String::new(), String::new(), false, subject)
        }
    };
    let mut breaking_desc = String::new();
    let mut collecting = false;
    for line in &lines[1..] {
        if let Some(rest) = is_breaking_footer(line) {
            breaking = true;
            breaking_desc = rest.to_string();
            collecting = true;
            continue;
        }
        if collecting {
            let t = line.trim();
            if t.is_empty() || is_trailer(line) || parse_header(strip_bullet(line)).is_some() {
                collecting = false;
                continue;
            }
            if !breaking_desc.is_empty() {
                breaking_desc.push(' ');
            }
            breaking_desc.push_str(t);
        }
    }
    Some(Commit {
        hash,
        ctype,
        scope,
        breaking,
        subject,
        breaking_desc: breaking_desc.trim().to_string(),
    })
}

/// Split a pasted log into commits.
///
/// * `git log` envelope lines (`commit <sha>`, `Author:`, `Date:`, `Merge:`) are dropped.
/// * If the log contains blank lines, **paragraphs** are commits — matching
///   `git log --pretty=%B`, where the `BREAKING CHANGE:` footer sits after a
///   blank line. A paragraph that starts with a footer or a `BREAKING CHANGE:`
///   line is attached to the previous commit instead of starting a new one.
/// * Otherwise **each line** is a commit — matching `git log --oneline`.
/// * A leading short hash and a leading `-`/`*`/`+` bullet are peeled off.
pub fn parse_log(input: &str) -> Vec<Commit> {
    let kept: Vec<&str> = input.lines().filter(|l| !is_log_metadata(l)).collect();
    let start = kept
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(kept.len());
    let end = kept
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(start);
    let lines = &kept[start..end];
    if lines.is_empty() {
        return Vec::new();
    }
    let paragraph_mode = lines.iter().any(|l| l.trim().is_empty());

    let mut blocks: Vec<Vec<&str>> = Vec::new();
    if paragraph_mode {
        let mut current: Vec<&str> = Vec::new();
        for line in lines {
            if line.trim().is_empty() {
                if !current.is_empty() {
                    blocks.push(std::mem::take(&mut current));
                }
            } else {
                current.push(line);
            }
        }
        if !current.is_empty() {
            blocks.push(current);
        }
        // Re-attach footer-only paragraphs to the commit they belong to.
        let mut merged: Vec<Vec<&str>> = Vec::new();
        for b in blocks {
            let head = b[0];
            let attaches = is_breaking_footer(head).is_some() || is_trailer(head);
            if attaches && !merged.is_empty() {
                merged.last_mut().unwrap().extend(b);
            } else {
                merged.push(b);
            }
        }
        blocks = merged;
    } else {
        for line in lines {
            let attaches = is_breaking_footer(line).is_some() || is_trailer(line);
            if attaches && !blocks.is_empty() {
                blocks.last_mut().unwrap().push(line);
            } else {
                blocks.push(vec![line]);
            }
        }
    }

    blocks
        .iter()
        .filter_map(|b| commit_from_block(b))
        .take(MAX_COMMITS)
        .collect()
}

// ---------------------------------------------------------------- grouping

const GROUP_ORDER: [&str; 13] = [
    "feat", "fix", "perf", "security", "revert", "refactor", "docs", "deps", "build", "ci",
    "style", "test", "chore",
];

fn title_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn group_title(t: &str) -> String {
    match t {
        "feat" => "Features".to_string(),
        "fix" => "Bug Fixes".to_string(),
        "perf" => "Performance".to_string(),
        "security" => "Security".to_string(),
        "revert" => "Reverts".to_string(),
        "refactor" => "Refactoring".to_string(),
        "docs" => "Documentation".to_string(),
        "deps" => "Dependencies".to_string(),
        "build" => "Build System".to_string(),
        "ci" => "Continuous Integration".to_string(),
        "style" => "Styles".to_string(),
        "test" => "Tests".to_string(),
        "chore" => "Chores".to_string(),
        "" => "Other Changes".to_string(),
        other => title_case(other),
    }
}

/// Split a comma/space/newline separated list into canonical types.
fn type_list(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s == "*" {
                "*".to_string()
            } else {
                canonical_type(s)
            }
        })
        .collect()
}

fn list_matches(list: &[String], ctype: &str) -> bool {
    list.iter().any(|t| t == "*" || t == ctype)
}

// ---------------------------------------------------------------- rendering

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Turn `#123` references into markdown links when a repository URL is known.
fn link_issues(text: &str, repo_url: &str) -> String {
    if repo_url.is_empty() || !text.contains('#') {
        return text.to_string();
    }
    let bytes: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '#' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let num: String = bytes[i + 1..j].iter().collect();
            out.push_str(&format!("[#{num}]({repo_url}/issues/{num})"));
            i = j;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

fn entry_line(c: &Commit, repo_url: &str, text: &str) -> String {
    let mut line = String::from("- ");
    if !c.scope.is_empty() {
        line.push_str(&format!("**{}:** ", c.scope));
    }
    line.push_str(&link_issues(text, repo_url));
    if !c.hash.is_empty() {
        if repo_url.is_empty() {
            line.push_str(&format!(" ({})", c.hash));
        } else {
            line.push_str(&format!(" ([{}]({}/commit/{}))", c.hash, repo_url, c.hash));
        }
    }
    line
}

// ---------------------------------------------------------------- entrypoint

/// Full run. Every parameter arrives as a string (the page, the CLI and the
/// chat schema all pass strings); empty strings select the documented default.
#[allow(clippy::too_many_arguments)]
pub fn run(
    current_version: &str,
    commits: &str,
    minor_types: &str,
    patch_types: &str,
    zero_version_policy: &str,
    prerelease_policy: &str,
    prerelease_identifier: &str,
    hidden_types: &str,
    repo_url: &str,
    release_date: &str,
    output_format: &str,
) -> Result<String, String> {
    if commits.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "commit log is too large: {} bytes, the limit is {} bytes (1 MiB)",
            commits.len(),
            MAX_INPUT_BYTES
        ));
    }
    let current = parse_version(current_version)?;

    let zero_policy = match zero_version_policy.trim() {
        "" | "standard" => "standard",
        "cautious" => "cautious",
        other => {
            return Err(format!(
                "invalid zero_version_policy {other:?}: expected standard or cautious"
            ))
        }
    };
    let pre_policy = match prerelease_policy.trim() {
        "" | "finalize" => "finalize",
        "increment" => "increment",
        "ignore" => "ignore",
        other => {
            return Err(format!(
                "invalid prerelease_policy {other:?}: expected finalize, increment or ignore"
            ))
        }
    };
    let fmt = match output_format.trim() {
        "" | "markdown" => "markdown",
        "version" => "version",
        "json" => "json",
        other => {
            return Err(format!(
                "invalid output_format {other:?}: expected markdown, version or json"
            ))
        }
    };
    let identifier = match prerelease_identifier.trim() {
        "" => "rc",
        id => {
            if !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
                || id.starts_with('.')
                || id.ends_with('.')
            {
                return Err(format!(
                    "invalid prerelease_identifier {id:?}: use letters, digits, dots or hyphens, such as rc, beta or alpha"
                ));
            }
            id
        }
    };
    let repo = repo_url.trim().trim_end_matches('/');
    if !repo.is_empty() && !(repo.starts_with("https://") || repo.starts_with("http://")) {
        return Err(format!(
            "invalid repo_url {repo:?}: expected a full URL starting with https://, such as https://github.com/acme/widget"
        ));
    }
    let date = release_date.trim();
    if date.contains('\n') || date.contains(')') {
        return Err(format!(
            "invalid release_date {date:?}: expected a plain date such as 2026-08-29"
        ));
    }

    let minor_list = {
        let l = type_list(minor_types);
        if l.is_empty() {
            vec!["feat".to_string()]
        } else {
            l
        }
    };
    let patch_list = {
        let l = type_list(patch_types);
        if l.is_empty() {
            vec!["fix".to_string(), "perf".to_string(), "revert".to_string()]
        } else {
            l
        }
    };
    let hidden = type_list(hidden_types);

    let parsed = parse_log(commits);
    if parsed.is_empty() {
        return Err(
            "no commits found: paste a commit log, for example the output of `git log --oneline v1.4.2..HEAD`"
                .to_string(),
        );
    }

    // ---- bump
    let mut level = Bump::None;
    for c in &parsed {
        let l = if c.breaking {
            Bump::Major
        } else if list_matches(&minor_list, &c.ctype) {
            Bump::Minor
        } else if list_matches(&patch_list, &c.ctype) {
            Bump::Patch
        } else {
            Bump::None
        };
        if l > level {
            level = l;
        }
    }
    if current.major == 0 && zero_policy == "cautious" {
        level = match level {
            Bump::Major => Bump::Minor,
            Bump::Minor => Bump::Patch,
            other => other,
        };
    }

    let next = if level == Bump::None {
        current.clone()
    } else {
        match pre_policy {
            "increment" => inc_prerelease(&current, level, identifier),
            "ignore" => inc_stable(&current, level),
            _ => inc_finalize(&current, level),
        }
    };
    let next_str = if level == Bump::None {
        current_version.trim().to_string()
    } else {
        next.tagged()
    };

    if fmt == "version" {
        return Ok(next_str);
    }

    // ---- grouping
    let breaking: Vec<&Commit> = parsed.iter().filter(|c| c.breaking).collect();
    let mut types: Vec<String> = Vec::new();
    for c in &parsed {
        if !types.contains(&c.ctype) {
            types.push(c.ctype.clone());
        }
    }
    types.sort_by_key(|t| {
        let known = GROUP_ORDER.iter().position(|g| *g == t.as_str());
        match known {
            Some(i) => (0usize, i, String::new()),
            None if t.is_empty() => (2, 0, String::new()),
            None => (1, 0, t.clone()),
        }
    });

    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut hidden_counts: Vec<(String, usize)> = Vec::new();
    for t in &types {
        let mut entries: Vec<&Commit> = parsed.iter().filter(|c| &c.ctype == t).collect();
        let visible = if t.is_empty() {
            !hidden.iter().any(|h| h == "other")
        } else {
            !list_matches(&hidden, t)
        };
        if !visible {
            hidden_counts.push((t.clone(), entries.len()));
            continue;
        }
        entries.sort_by(|a, b| {
            a.scope
                .to_ascii_lowercase()
                .cmp(&b.scope.to_ascii_lowercase())
                .then(
                    a.subject
                        .to_ascii_lowercase()
                        .cmp(&b.subject.to_ascii_lowercase()),
                )
        });
        let lines: Vec<String> = entries
            .iter()
            .map(|c| entry_line(c, repo, &c.subject))
            .collect();
        sections.push((group_title(t), lines));
    }

    if fmt == "json" {
        let notes = render_markdown(
            &current, &next_str, level, &parsed, &breaking, &sections, repo, date,
        );
        let mut out = String::from("{\n");
        out.push_str(&format!(
            "  \"current_version\": \"{}\",\n",
            json_escape(current_version.trim())
        ));
        out.push_str(&format!(
            "  \"next_version\": \"{}\",\n",
            json_escape(&next_str)
        ));
        out.push_str(&format!("  \"bump\": \"{}\",\n", level.label()));
        out.push_str(&format!("  \"commit_count\": {},\n", parsed.len()));
        out.push_str(&format!("  \"breaking_count\": {},\n", breaking.len()));
        out.push_str("  \"breaking_changes\": [");
        for (i, c) in breaking.iter().enumerate() {
            let text = if c.breaking_desc.is_empty() {
                &c.subject
            } else {
                &c.breaking_desc
            };
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("\n    \"{}\"", json_escape(text)));
        }
        if !breaking.is_empty() {
            out.push_str("\n  ");
        }
        out.push_str("],\n");
        out.push_str("  \"groups\": [");
        let mut first = true;
        for t in &types {
            let entries: Vec<&Commit> = parsed.iter().filter(|c| &c.ctype == t).collect();
            if !first {
                out.push(',');
            }
            first = false;
            let is_hidden = hidden_counts.iter().any(|(h, _)| h == t);
            out.push_str(&format!(
                "\n    {{ \"type\": \"{}\", \"title\": \"{}\", \"hidden\": {}, \"count\": {}, \"commits\": [",
                json_escape(t),
                json_escape(&group_title(t)),
                is_hidden,
                entries.len()
            ));
            for (i, c) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&format!(
                    "\n      {{ \"hash\": \"{}\", \"scope\": \"{}\", \"breaking\": {}, \"subject\": \"{}\" }}",
                    json_escape(&c.hash),
                    json_escape(&c.scope),
                    c.breaking,
                    json_escape(&c.subject)
                ));
            }
            if !entries.is_empty() {
                out.push_str("\n    ");
            }
            out.push_str("] }");
        }
        if !types.is_empty() {
            out.push_str("\n  ");
        }
        out.push_str("],\n");
        out.push_str(&format!("  \"notes\": \"{}\"\n}}", json_escape(&notes)));
        return Ok(out);
    }

    Ok(render_markdown(
        &current, &next_str, level, &parsed, &breaking, &sections, repo, date,
    ))
}

#[allow(clippy::too_many_arguments)]
fn render_markdown(
    current: &Version,
    next_str: &str,
    level: Bump,
    parsed: &[Commit],
    breaking: &[&Commit],
    sections: &[(String, Vec<String>)],
    repo: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    let plural = |n: usize| if n == 1 { "commit" } else { "commits" };

    if level == Bump::None {
        out.push_str("## No release required\n\n");
        out.push_str(&format!(
            "_{} {} reviewed · none of them trigger a release · {} stays as it is_\n",
            parsed.len(),
            plural(parsed.len()),
            current.tagged()
        ));
    } else {
        if date.is_empty() {
            out.push_str(&format!("## {next_str}\n\n"));
        } else {
            out.push_str(&format!("## {next_str} ({date})\n\n"));
        }
        out.push_str(&format!(
            "_{} release · {} {} since {}_\n",
            level.label(),
            parsed.len(),
            plural(parsed.len()),
            current.tagged()
        ));
    }

    if !breaking.is_empty() {
        out.push_str("\n### Breaking Changes\n\n");
        for c in breaking {
            let text = if c.breaking_desc.is_empty() {
                c.subject.clone()
            } else {
                c.breaking_desc.clone()
            };
            out.push_str(&entry_line(c, repo, &text));
            out.push('\n');
        }
    }

    for (title, lines) in sections {
        if lines.is_empty() {
            continue;
        }
        out.push_str(&format!("\n### {title}\n\n"));
        for l in lines {
            out.push_str(l);
            out.push('\n');
        }
    }

    if !repo.is_empty() && level != Bump::None {
        out.push_str(&format!(
            "\n[Full changelog]({}/compare/{}...{})\n",
            repo,
            current.tagged(),
            next_str
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: (&str, &str, &str, &str, &str, &str, &str, &str) = (
        "feat,feature",
        "fix,perf,revert",
        "standard",
        "finalize",
        "rc",
        "chore,style,ci,build,test",
        "",
        "",
    );

    fn go(version: &str, log: &str) -> String {
        run(
            version, log, D.0, D.1, D.2, D.3, D.4, D.5, D.6, D.7, "markdown",
        )
        .unwrap()
    }
    fn ver(version: &str, log: &str) -> String {
        run(
            version, log, D.0, D.1, D.2, D.3, D.4, D.5, D.6, D.7, "version",
        )
        .unwrap()
    }

    #[test]
    fn happy_path_minor_release_with_grouped_notes() {
        let out = go(
            "1.4.2",
            "feat(parser): support nested scopes\nfix: handle an empty log\nchore: bump deps\n",
        );
        assert!(out.starts_with("## 1.5.0\n"), "{out}");
        assert!(
            out.contains("_minor release · 3 commits since 1.4.2_"),
            "{out}"
        );
        assert!(
            out.contains("### Features\n\n- **parser:** support nested scopes"),
            "{out}"
        );
        assert!(
            out.contains("### Bug Fixes\n\n- handle an empty log"),
            "{out}"
        );
        // chore is hidden by default
        assert!(!out.contains("Chores"), "{out}");
    }

    #[test]
    fn error_on_invalid_current_version() {
        let err = run(
            "not-a-version",
            "fix: x",
            D.0,
            D.1,
            D.2,
            D.3,
            D.4,
            D.5,
            D.6,
            D.7,
            "markdown",
        )
        .unwrap_err();
        assert!(err.contains("invalid current version"), "{err}");
        assert!(err.contains("1.4.2"), "{err}");
    }

    #[test]
    fn error_on_empty_commit_log() {
        let err = run(
            "1.0.0", "   \n\n", D.0, D.1, D.2, D.3, D.4, D.5, D.6, D.7, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("no commits found"), "{err}");
    }

    #[test]
    fn bang_marker_and_footer_both_force_major() {
        assert_eq!(ver("1.4.2", "feat!: drop node 18"), "2.0.0");
        let log = "feat: new api\n\nBREAKING CHANGE: `render()` now returns a Result\n";
        assert_eq!(ver("1.4.2", log), "2.0.0");
        let out = go("1.4.2", log);
        assert!(
            out.contains("### Breaking Changes\n\n- `render()` now returns a Result"),
            "{out}"
        );
    }

    #[test]
    fn patch_only_log_bumps_patch_and_docs_stay_visible() {
        let out = go("1.4.2", "fix: off-by-one\ndocs: rewrite the readme\n");
        assert!(out.starts_with("## 1.4.3\n"), "{out}");
        assert!(out.contains("### Documentation"), "{out}");
    }

    #[test]
    fn no_release_when_nothing_matches() {
        let out = go("1.4.2", "docs: typo\nchore: tidy\n");
        assert!(out.starts_with("## No release required\n"), "{out}");
        assert_eq!(ver("1.4.2", "docs: typo"), "1.4.2");
    }

    #[test]
    fn wildcard_patch_types_release_everything() {
        let out = run(
            "1.4.2",
            "docs: typo",
            D.0,
            "*",
            D.2,
            D.3,
            D.4,
            D.5,
            D.6,
            D.7,
            "version",
        )
        .unwrap();
        assert_eq!(out, "1.4.3");
    }

    #[test]
    fn empty_patch_types_uses_documented_default() {
        let out = run(
            "2.0.0-rc.1",
            "fix(release): update generated notes",
            D.0,
            "",
            D.2,
            "increment",
            D.4,
            D.5,
            D.6,
            D.7,
            "version",
        )
        .unwrap();
        assert_eq!(out, "2.0.0-rc.2");
    }

    #[test]
    fn zero_version_cautious_policy_downgrades_bumps() {
        let log = "feat!: rewrite the api";
        assert_eq!(ver("0.4.2", log), "1.0.0");
        let cautious = run(
            "0.4.2", log, D.0, D.1, "cautious", D.3, D.4, D.5, D.6, D.7, "version",
        )
        .unwrap();
        assert_eq!(cautious, "0.5.0");
        let feat = run(
            "0.4.2",
            "feat: add a flag",
            D.0,
            D.1,
            "cautious",
            D.3,
            D.4,
            D.5,
            D.6,
            D.7,
            "version",
        )
        .unwrap();
        assert_eq!(feat, "0.4.3");
    }

    #[test]
    fn prerelease_policies() {
        // finalize: the pending rc already carries the major bump
        assert_eq!(ver("2.0.0-rc.1", "feat!: break it"), "2.0.0");
        assert_eq!(ver("1.2.3-rc.1", "fix: x"), "1.2.3");
        assert_eq!(ver("1.2.3-rc.1", "feat: x"), "1.3.0");
        // increment: stay on the rc line, or open a new one
        let inc = |v: &str, log: &str| {
            run(
                v,
                log,
                D.0,
                D.1,
                D.2,
                "increment",
                "rc",
                D.5,
                D.6,
                D.7,
                "version",
            )
            .unwrap()
        };
        assert_eq!(inc("2.0.0-rc.1", "feat!: break it"), "2.0.0-rc.2");
        assert_eq!(inc("1.4.2", "feat: add a flag"), "1.5.0-rc.0");
        assert_eq!(inc("1.4.2-beta", "fix: x"), "1.4.2-beta.0");
        // ignore: treat the pre-release as its base version
        let ign = |v: &str, log: &str| {
            run(
                v, log, D.0, D.1, D.2, "ignore", D.4, D.5, D.6, D.7, "version",
            )
            .unwrap()
        };
        assert_eq!(ign("2.0.0-rc.1", "fix: x"), "2.0.1");
        assert_eq!(ign("2.0.0-rc.1", "feat!: y"), "3.0.0");
    }

    #[test]
    fn tag_prefixes_are_preserved() {
        assert_eq!(ver("v1.4.2", "feat: x"), "v1.5.0");
        assert_eq!(ver("web-v2.1.0", "fix: x"), "web-v2.1.1");
        assert_eq!(ver("1.4.2+build.7", "fix: x"), "1.4.3");
    }

    #[test]
    fn oneline_log_with_hashes_and_unconventional_lines() {
        let out = go(
            "1.0.0",
            "a1b2c3d feat(cli): add --json\n9f8e7d6 fix: crash on empty input\nc0ffee1 update the readme\n",
        );
        assert!(out.contains("- **cli:** add --json (a1b2c3d)"), "{out}");
        assert!(
            out.contains("### Other Changes\n\n- update the readme (c0ffee1)"),
            "{out}"
        );
    }

    #[test]
    fn full_git_log_paste_with_envelope_and_trailers() {
        let log = "commit a1b2c3d4e5f60718293a4b5c6d7e8f9012345678\n\
Author: Someone <someone@example.com>\n\
Date:   Fri Aug 28 10:00:00 2026 +0200\n\
\n\
    feat(api): add a streaming endpoint\n\
\n\
    BREAKING CHANGE: the callback argument was removed\n\
\n\
    Signed-off-by: Someone <someone@example.com>\n";
        let out = go("1.4.2", log);
        assert!(out.starts_with("## 2.0.0\n"), "{out}");
        assert!(
            out.contains("- **api:** the callback argument was removed"),
            "{out}"
        );
        assert!(!out.contains("Signed-off-by"), "{out}");
        assert!(!out.contains("Author:"), "{out}");
    }

    #[test]
    fn repo_url_adds_issue_commit_and_compare_links() {
        let out = run(
            "v1.4.2",
            "a1b2c3d fix: stop crashing on #42",
            D.0,
            D.1,
            D.2,
            D.3,
            D.4,
            D.5,
            "https://github.com/acme/widget/",
            "2026-08-29",
            "markdown",
        )
        .unwrap();
        assert!(out.starts_with("## v1.4.3 (2026-08-29)\n"), "{out}");
        assert!(
            out.contains("[#42](https://github.com/acme/widget/issues/42)"),
            "{out}"
        );
        assert!(
            out.contains("([a1b2c3d](https://github.com/acme/widget/commit/a1b2c3d))"),
            "{out}"
        );
        assert!(
            out.contains(
                "[Full changelog](https://github.com/acme/widget/compare/v1.4.2...v1.4.3)"
            ),
            "{out}"
        );
    }

    #[test]
    fn hidden_types_can_be_opened_up() {
        let out = run(
            "1.4.2",
            "chore: tidy\nfix: x",
            D.0,
            D.1,
            D.2,
            D.3,
            D.4,
            "",
            D.6,
            D.7,
            "markdown",
        )
        .unwrap();
        assert!(out.contains("### Chores"), "{out}");
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = run(
            "1.4.2",
            "feat(cli)!: rename --out to --output",
            D.0,
            D.1,
            D.2,
            D.3,
            D.4,
            D.5,
            D.6,
            D.7,
            "json",
        )
        .unwrap();
        assert!(out.contains("\"next_version\": \"2.0.0\""), "{out}");
        assert!(out.contains("\"bump\": \"major\""), "{out}");
        assert!(out.contains("\"breaking_count\": 1"), "{out}");
        assert!(out.contains("\"scope\": \"cli\""), "{out}");
        assert!(out.contains("\"notes\": \"## 2.0.0"), "{out}");
    }

    #[test]
    fn invalid_enum_values_are_rejected() {
        let err = run(
            "1.0.0", "fix: x", D.0, D.1, "loose", D.3, D.4, D.5, D.6, D.7, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("invalid zero_version_policy"), "{err}");

        let err = run(
            "1.0.0", "fix: x", D.0, D.1, D.2, "sideways", D.4, D.5, D.6, D.7, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("invalid prerelease_policy"), "{err}");

        let err = run(
            "1.0.0", "fix: x", D.0, D.1, D.2, D.3, D.4, D.5, D.6, D.7, "yaml",
        )
        .unwrap_err();
        assert!(err.contains("invalid output_format"), "{err}");

        let err = run(
            "1.0.0", "fix: x", D.0, D.1, D.2, D.3, "rc 1", D.5, D.6, D.7, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("invalid prerelease_identifier"), "{err}");

        let err = run(
            "1.0.0",
            "fix: x",
            D.0,
            D.1,
            D.2,
            D.3,
            D.4,
            D.5,
            "github.com/x",
            D.7,
            "markdown",
        )
        .unwrap_err();
        assert!(err.contains("invalid repo_url"), "{err}");
    }

    #[test]
    fn oversized_log_is_rejected() {
        let big = "fix: x\n".repeat(MAX_INPUT_BYTES / 7 + 2);
        let err = run(
            "1.0.0", &big, D.0, D.1, D.2, D.3, D.4, D.5, D.6, D.7, "markdown",
        )
        .unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }
}
