//! er-diagram-from-sql core — turn SQL `CREATE TABLE` / `ALTER TABLE` DDL into
//! a Mermaid `erDiagram`: one entity per table, its columns as typed attributes
//! with `PK`/`FK`/`UK` markers, and one crow's-foot relationship per foreign
//! key. Pure compute, no wafer/wasm-bindgen deps; shared by the chat skill
//! block and the web page.
//!
//! The DDL parsing itself is reused from `blocks/sql-schema-extractor` (a
//! lenient parser that survives a real dump — comments and non-DDL statements
//! are skipped and nothing is executed). This crate owns only the mapping of
//! that model onto Mermaid syntax: cardinality inference, attribute rendering,
//! and the identifier/type sanitizing Mermaid's grammar requires.

use serde_json::Value;

/// Diagrams past this size are unreadable and Mermaid itself struggles well
/// before it, so refuse rather than emit a wall of text.
pub const MAX_TABLES: usize = 500;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Which columns to list inside each entity block.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Attributes {
    /// Every column.
    All,
    /// Only primary-key, foreign-key and unique columns.
    Keys,
    /// No attribute blocks — entity names and relationships only.
    Hidden,
}

impl Attributes {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(Attributes::All),
            "keys" => Ok(Attributes::Keys),
            "none" => Ok(Attributes::Hidden),
            other => Err(format!(
                "invalid attributes {other:?}: expected \"all\", \"keys\", or \"none\""
            )),
        }
    }
}

/// What to write after the `:` on a relationship line.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RelLabel {
    /// The foreign-key column name(s).
    Column,
    /// The foreign-key constraint name, falling back to the columns.
    Constraint,
    /// An empty label.
    None,
}

impl RelLabel {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "column" => Ok(RelLabel::Column),
            "constraint" => Ok(RelLabel::Constraint),
            "none" => Ok(RelLabel::None),
            other => Err(format!(
                "invalid relationship_label {other:?}: expected \"column\", \"constraint\", or \"none\""
            )),
        }
    }
}

/// Optional `direction` statement emitted inside the diagram.
fn parse_direction(s: &str) -> Result<Option<&'static str>, String> {
    match s.trim().to_ascii_uppercase().as_str() {
        "" | "AUTO" => Ok(None),
        "LR" => Ok(Some("LR")),
        "RL" => Ok(Some("RL")),
        "TB" | "TD" => Ok(Some("TB")),
        "BT" => Ok(Some("BT")),
        other => Err(format!(
            "invalid direction {other:?}: expected \"auto\", \"LR\", \"RL\", \"TB\", or \"BT\""
        )),
    }
}

// ---------------------------------------------------------------------------
// Model (projected out of the shared parser's JSON)
// ---------------------------------------------------------------------------

struct Col {
    name: String,
    ty: String,
    nullable: bool,
    primary_key: bool,
    unique: bool,
}

struct Fk {
    name: Option<String>,
    columns: Vec<String>,
    ref_table: String,
}

struct Tbl {
    name: String,
    cols: Vec<Col>,
    pk: Vec<String>,
    fks: Vec<Fk>,
    /// Every column set known to be unique: the primary key, `UNIQUE`
    /// constraints, unique indexes, and column-level `UNIQUE` flags.
    unique_sets: Vec<Vec<String>>,
}

impl Tbl {
    fn column(&self, name: &str) -> Option<&Col> {
        self.cols.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// Is this exact column set unique in this table (so at most one row per value)?
    fn is_unique(&self, cols: &[String]) -> bool {
        self.unique_sets.iter().any(|s| same_set(s, cols))
    }
}

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

fn same_set(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a: Vec<String> = a.iter().map(|s| lower(s)).collect();
    let mut b: Vec<String> = b.iter().map(|s| lower(s)).collect();
    a.sort();
    b.sort();
    a == b
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Project the shared extractor's JSON model onto the small shape we need.
fn project(model: &Value) -> Vec<Tbl> {
    let mut out = Vec::new();
    let Some(tables) = model.get("tables").and_then(Value::as_array) else {
        return out;
    };
    for t in tables {
        let name = t
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("table")
            .to_string();
        let pk = str_list(t.get("primary_key"));

        let mut cols = Vec::new();
        let mut unique_sets: Vec<Vec<String>> = Vec::new();
        if let Some(list) = t.get("columns").and_then(Value::as_array) {
            for c in list {
                let cname = c
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if cname.is_empty() {
                    continue;
                }
                let unique = c.get("unique").and_then(Value::as_bool).unwrap_or(false);
                if unique {
                    unique_sets.push(vec![cname.clone()]);
                }
                cols.push(Col {
                    ty: c
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    nullable: c.get("nullable").and_then(Value::as_bool).unwrap_or(true),
                    primary_key: c
                        .get("primary_key")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                        || pk.iter().any(|p| p.eq_ignore_ascii_case(&cname)),
                    unique,
                    name: cname,
                });
            }
        }

        if !pk.is_empty() {
            unique_sets.push(pk.clone());
        }
        for u in t
            .get("unique_constraints")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            let cols = str_list(u.get("columns"));
            if !cols.is_empty() {
                unique_sets.push(cols);
            }
        }
        for idx in t
            .get("indexes")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            if idx.get("unique").and_then(Value::as_bool).unwrap_or(false) {
                let cols = str_list(idx.get("columns"));
                if !cols.is_empty() {
                    unique_sets.push(cols);
                }
            }
        }

        let mut fks = Vec::new();
        for f in t
            .get("foreign_keys")
            .and_then(Value::as_array)
            .unwrap_or(&Vec::new())
        {
            let columns = str_list(f.get("columns"));
            let ref_table = f
                .get("references")
                .and_then(|r| r.get("table"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if columns.is_empty() || ref_table.is_empty() {
                continue;
            }
            fks.push(Fk {
                name: f
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|s| !s.is_empty()),
                columns,
                ref_table,
            });
        }

        out.push(Tbl {
            name,
            cols,
            pk,
            fks,
            unique_sets,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Mermaid-safe rendering helpers
// ---------------------------------------------------------------------------

/// Mermaid's attribute grammar accepts only `[A-Za-z0-9_()\[\]-]` inside a type
/// or name token and requires an alphabetic first character — so `DECIMAL(10,2)`
/// and `TIMESTAMP WITH TIME ZONE` break rendering if passed through verbatim.
/// Map anything else to `_` (keeping the precision info) rather than dropping it.
fn mermaid_token(s: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '(' | ')' | '[' | ']') {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return fallback.to_string();
    }
    match out.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => out,
        _ => format!("{fallback}_{out}"),
    }
}

/// Entity names allow a wider set but must be double-quoted when they contain
/// anything outside it (e.g. the `.` of a schema-qualified `public.users`).
fn mermaid_entity(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "\"\"".to_string();
    }
    let plain = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
    if plain {
        trimmed.to_string()
    } else {
        format!("\"{}\"", trimmed.replace('"', ""))
    }
}

/// Relationship labels are quoted strings, which cannot contain a quote.
fn mermaid_label(s: &str) -> String {
    s.replace('"', "")
}

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

struct Rel {
    parent: String,
    child: String,
    cardinality: String,
    label: String,
}

/// Crow's-foot cardinality for one foreign key.
///
/// * parent side — `||` (exactly one) when every FK column is `NOT NULL`, else
///   `|o` (zero or one);
/// * child side — `o|` (zero or one) when the FK columns are unique in the
///   child, i.e. a 1:1, else `o{` (zero or more);
/// * line — solid `--` (identifying) for a `NOT NULL` FK, dashed `..`
///   (non-identifying) for a nullable one, since a nullable FK means the child
///   can exist without a parent.
fn cardinality(not_null: bool, unique_child: bool) -> String {
    let left = if not_null { "||" } else { "|o" };
    let line = if not_null { "--" } else { ".." };
    let right = if unique_child { "o|" } else { "o{" };
    format!("{left}{line}{right}")
}

/// Are all of `cols` declared `NOT NULL` in `tbl`?
fn all_not_null(tbl: &Tbl, cols: &[String]) -> bool {
    cols.iter()
        .all(|c| tbl.column(c).map(|c| !c.nullable).unwrap_or(false))
}

/// Resolve a referenced table name to the canonical casing of a defined table.
fn canonical<'a>(tables: &'a [Tbl], name: &'a str) -> &'a str {
    tables
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
        .map(|t| t.name.as_str())
        .unwrap_or(name)
}

/// Heuristic: a `<something>_id` column with no explicit FK, where `<something>`
/// names a table (singular or plural), is treated as a reference to it.
fn infer_target<'a>(tables: &'a [Tbl], column: &str) -> Option<&'a Tbl> {
    let lc = lower(column);
    let stem = lc.strip_suffix("_id")?;
    if stem.is_empty() {
        return None;
    }
    let mut candidates = vec![stem.to_string(), format!("{stem}s"), format!("{stem}es")];
    if let Some(base) = stem.strip_suffix('y') {
        candidates.push(format!("{base}ies"));
    }
    tables
        .iter()
        .find(|t| candidates.iter().any(|c| t.name.eq_ignore_ascii_case(c)))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert `sql` DDL into a Mermaid `erDiagram`.
///
/// * `dialect` — `auto` | `mysql` | `postgres` | `sqlite` | `mssql` | `generic`.
/// * `attributes` — `all` | `keys` | `none`.
/// * `key_markers` — append `PK`/`FK`/`UK` to attribute lines.
/// * `mark_nullable` — render nullable columns with Mermaid's optional `type?` form.
/// * `infer_relations` — also link `<table>_id` columns that have no explicit FK.
/// * `relationship_label` — `column` | `constraint` | `none`.
/// * `direction` — `auto` | `LR` | `RL` | `TB` | `BT`.
/// * `fence` — wrap the diagram in a ```mermaid code fence.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    sql: &str,
    dialect: &str,
    attributes: &str,
    key_markers: bool,
    mark_nullable: bool,
    infer_relations: bool,
    relationship_label: &str,
    direction: &str,
    fence: bool,
) -> Result<String, String> {
    let attributes = Attributes::parse(attributes)?;
    let rel_label = RelLabel::parse(relationship_label)?;
    let direction = parse_direction(direction)?;

    // Reuse the lenient DDL parser; it owns dialect validation and the
    // empty-input / no-DDL errors.
    let model = gizza_ai_sql_schema_extractor_core::extract(sql, "json", dialect, true, true)?;
    let model: Value = serde_json::from_str(&model)
        .map_err(|e| format!("could not read the parsed schema model: {e}"))?;

    let tables = project(&model);
    if tables.is_empty() {
        return Err(
            "no CREATE TABLE / ALTER TABLE statements found: this tool draws a diagram from DDL, \
             not from INSERT rows or query results"
                .into(),
        );
    }
    if tables.len() > MAX_TABLES {
        return Err(format!(
            "schema has {} tables, which is more than the {MAX_TABLES}-table limit: \
             a diagram that large is unreadable — split the dump or keep only the tables you want to show",
            tables.len()
        ));
    }

    // Relationships, plus the set of columns that carry a foreign key (so the
    // attribute lines can be marked FK).
    let mut rels: Vec<Rel> = Vec::new();
    let mut fk_cols: Vec<(String, String)> = Vec::new();

    for t in &tables {
        for fk in &t.fks {
            for c in &fk.columns {
                fk_cols.push((lower(&t.name), lower(c)));
            }
            let label = match rel_label {
                RelLabel::Column => fk.columns.join(", "),
                RelLabel::Constraint => fk
                    .name
                    .clone()
                    .unwrap_or_else(|| fk.columns.join(", ")),
                RelLabel::None => String::new(),
            };
            rels.push(Rel {
                parent: mermaid_entity(canonical(&tables, &fk.ref_table)),
                child: mermaid_entity(&t.name),
                cardinality: cardinality(all_not_null(t, &fk.columns), t.is_unique(&fk.columns)),
                label: mermaid_label(&label),
            });
        }
    }

    if infer_relations {
        for t in &tables {
            for c in &t.cols {
                if fk_cols
                    .iter()
                    .any(|(tn, cn)| tn == &lower(&t.name) && cn == &lower(&c.name))
                {
                    continue;
                }
                let Some(target) = infer_target(&tables, &c.name) else {
                    continue;
                };
                if target.name.eq_ignore_ascii_case(&t.name) {
                    continue;
                }
                let cols = vec![c.name.clone()];
                let label = match rel_label {
                    RelLabel::Column | RelLabel::Constraint => c.name.clone(),
                    RelLabel::None => String::new(),
                };
                fk_cols.push((lower(&t.name), lower(&c.name)));
                rels.push(Rel {
                    parent: mermaid_entity(&target.name),
                    child: mermaid_entity(&t.name),
                    cardinality: cardinality(!c.nullable, t.is_unique(&cols)),
                    label: mermaid_label(&label),
                });
            }
        }
    }

    // Render.
    let mut out = String::from("erDiagram\n");
    if let Some(d) = direction {
        out.push_str(&format!("    direction {d}\n"));
    }

    for t in &tables {
        let entity = mermaid_entity(&t.name);
        if attributes == Attributes::Hidden {
            out.push_str(&format!("    {entity}\n"));
            continue;
        }

        let mut lines: Vec<String> = Vec::new();
        for c in &t.cols {
            let is_pk = c.primary_key;
            let is_fk = fk_cols
                .iter()
                .any(|(tn, cn)| tn == &lower(&t.name) && cn == &lower(&c.name));
            // UK is only meaningful for single-column uniqueness; a member of a
            // composite UNIQUE is not unique on its own.
            let is_uk = !is_pk && t.is_unique(&[c.name.clone()]);

            if attributes == Attributes::Keys && !(is_pk || is_fk || is_uk) {
                continue;
            }

            let mut ty = mermaid_token(&c.ty, "unknown");
            if mark_nullable && c.nullable {
                ty.push('?');
            }
            let name = mermaid_token(&c.name, "column");

            let mut keys: Vec<&str> = Vec::new();
            if key_markers {
                if is_pk {
                    keys.push("PK");
                }
                if is_fk {
                    keys.push("FK");
                }
                if is_uk {
                    keys.push("UK");
                }
            }
            if keys.is_empty() {
                lines.push(format!("        {ty} {name}"));
            } else {
                lines.push(format!("        {ty} {name} {}", keys.join(", ")));
            }
        }

        if lines.is_empty() {
            // An empty block is not valid Mermaid — emit the bare entity.
            out.push_str(&format!("    {entity}\n"));
        } else {
            out.push_str(&format!("    {entity} {{\n"));
            for l in lines {
                out.push_str(&l);
                out.push('\n');
            }
            out.push_str("    }\n");
        }
    }

    for r in &rels {
        out.push_str(&format!(
            "    {} {} {} : \"{}\"\n",
            r.parent, r.cardinality, r.child, r.label
        ));
    }

    let out = out.trim_end().to_string();
    if fence {
        Ok(format!("```mermaid\n{out}\n```"))
    } else {
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn run(sql: &str) -> String {
        generate(sql, "auto", "all", true, false, false, "column", "auto", false).unwrap()
    }

    const USERS_ORDERS: &str = "CREATE TABLE users (\n  id INT PRIMARY KEY,\n  email VARCHAR(255) NOT NULL UNIQUE\n);\nCREATE TABLE orders (\n  id INT PRIMARY KEY,\n  user_id INT NOT NULL REFERENCES users(id),\n  total DECIMAL(10,2)\n);";

    #[test]
    fn happy_path_users_and_orders() {
        assert_eq!(
            run(USERS_ORDERS),
            "erDiagram\n\
             \x20   users {\n\
             \x20       INT id PK\n\
             \x20       VARCHAR(255) email UK\n\
             \x20   }\n\
             \x20   orders {\n\
             \x20       INT id PK\n\
             \x20       INT user_id FK\n\
             \x20       DECIMAL(10_2) total\n\
             \x20   }\n\
             \x20   users ||--o{ orders : \"user_id\""
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = generate("", "auto", "all", true, false, false, "column", "auto", false)
            .unwrap_err();
        assert!(err.contains("no SQL provided"), "{err}");
    }

    #[test]
    fn insert_only_input_is_an_error() {
        let err = generate(
            "INSERT INTO users VALUES (1, 'a@b.c');",
            "auto",
            "all",
            true,
            false,
            false,
            "column",
            "auto",
            false,
        )
        .unwrap_err();
        assert!(err.contains("no CREATE TABLE"), "{err}");
    }

    #[test]
    fn invalid_attributes_is_an_error() {
        let err = generate(
            USERS_ORDERS, "auto", "some", true, false, false, "column", "auto", false,
        )
        .unwrap_err();
        assert!(err.contains("invalid attributes"), "{err}");
    }

    #[test]
    fn invalid_relationship_label_and_direction_are_errors() {
        assert!(generate(
            USERS_ORDERS, "auto", "all", true, false, false, "verb", "auto", false
        )
        .unwrap_err()
        .contains("invalid relationship_label"));
        assert!(
            generate(USERS_ORDERS, "auto", "all", true, false, false, "column", "sideways", false)
                .unwrap_err()
                .contains("invalid direction")
        );
    }

    #[test]
    fn nullable_fk_is_optional_and_dashed() {
        let out = run("CREATE TABLE users (id INT PRIMARY KEY);\nCREATE TABLE posts (id INT PRIMARY KEY, author_id INT REFERENCES users(id));");
        assert!(out.contains("users |o..o{ posts : \"author_id\""), "{out}");
    }

    #[test]
    fn unique_fk_is_one_to_one() {
        let out = run("CREATE TABLE users (id INT PRIMARY KEY);\nCREATE TABLE profiles (id INT PRIMARY KEY, user_id INT NOT NULL UNIQUE REFERENCES users(id));");
        assert!(out.contains("users ||--o| profiles : \"user_id\""), "{out}");
    }

    #[test]
    fn composite_fk_lists_both_columns_in_the_label() {
        let out = run(
            "CREATE TABLE parts (org_id INT NOT NULL, sku VARCHAR(20) NOT NULL, PRIMARY KEY (org_id, sku));\n\
             CREATE TABLE lines (id INT PRIMARY KEY, org_id INT NOT NULL, sku VARCHAR(20) NOT NULL,\n\
               FOREIGN KEY (org_id, sku) REFERENCES parts(org_id, sku));",
        );
        assert!(out.contains("parts ||--o{ lines : \"org_id, sku\""), "{out}");
    }

    #[test]
    fn constraint_label_falls_back_to_columns_when_unnamed() {
        let named = generate(
            "CREATE TABLE users (id INT PRIMARY KEY);\nCREATE TABLE orders (id INT PRIMARY KEY, user_id INT NOT NULL, CONSTRAINT fk_orders_user FOREIGN KEY (user_id) REFERENCES users(id));",
            "auto", "all", true, false, false, "constraint", "auto", false,
        )
        .unwrap();
        assert!(named.contains(": \"fk_orders_user\""), "{named}");

        let unnamed = generate(
            USERS_ORDERS, "auto", "all", true, false, false, "constraint", "auto", false,
        )
        .unwrap();
        assert!(unnamed.contains(": \"user_id\""), "{unnamed}");
    }

    #[test]
    fn label_none_emits_an_empty_quoted_label() {
        let out = generate(
            USERS_ORDERS, "auto", "all", true, false, false, "none", "auto", false,
        )
        .unwrap();
        assert!(out.contains("users ||--o{ orders : \"\""), "{out}");
    }

    #[test]
    fn attributes_keys_drops_plain_columns() {
        let out = generate(
            USERS_ORDERS, "auto", "keys", true, false, false, "column", "auto", false,
        )
        .unwrap();
        assert!(out.contains("INT user_id FK"), "{out}");
        assert!(!out.contains("total"), "{out}");
    }

    #[test]
    fn attributes_none_emits_bare_entities() {
        let out = generate(
            USERS_ORDERS, "auto", "none", true, false, false, "column", "auto", false,
        )
        .unwrap();
        assert_eq!(
            out,
            "erDiagram\n    users\n    orders\n    users ||--o{ orders : \"user_id\""
        );
    }

    #[test]
    fn key_markers_off_leaves_plain_attributes() {
        let out = generate(
            USERS_ORDERS, "auto", "all", false, false, false, "column", "auto", false,
        )
        .unwrap();
        assert!(out.contains("        INT id\n"), "{out}");
        assert!(!out.contains(" PK"), "{out}");
    }

    #[test]
    fn mark_nullable_uses_the_optional_type_form() {
        let out = generate(
            USERS_ORDERS, "auto", "all", true, true, false, "column", "auto", false,
        )
        .unwrap();
        assert!(out.contains("DECIMAL(10_2)? total"), "{out}");
        assert!(out.contains("INT user_id FK"), "{out}");
    }

    #[test]
    fn infer_relations_links_id_columns_without_a_foreign_key() {
        let sql = "CREATE TABLE companies (id INT PRIMARY KEY);\nCREATE TABLE employees (id INT PRIMARY KEY, company_id INT NOT NULL);";
        let without = run(sql);
        assert!(!without.contains("||--o{"), "{without}");
        assert!(!without.contains("company_id FK"), "{without}");

        let with = generate(sql, "auto", "all", true, false, true, "column", "auto", false).unwrap();
        assert!(
            with.contains("companies ||--o{ employees : \"company_id\""),
            "{with}"
        );
        assert!(with.contains("INT company_id FK"), "{with}");
    }

    #[test]
    fn inferred_relations_match_plural_and_y_ies_table_names() {
        let out = generate(
            "CREATE TABLE categories (id INT PRIMARY KEY);\nCREATE TABLE items (id INT PRIMARY KEY, category_id INT NOT NULL);",
            "auto", "all", true, false, true, "column", "auto", false,
        )
        .unwrap();
        assert!(out.contains("categories ||--o{ items"), "{out}");
    }

    #[test]
    fn inference_never_duplicates_an_explicit_foreign_key() {
        let out = generate(
            USERS_ORDERS, "auto", "all", true, false, true, "column", "auto", false,
        )
        .unwrap();
        assert_eq!(out.matches("users ||--o{ orders").count(), 1, "{out}");
    }

    #[test]
    fn direction_and_fence_wrap_the_diagram() {
        let out = generate(
            USERS_ORDERS, "auto", "all", true, false, false, "column", "LR", true,
        )
        .unwrap();
        assert!(out.starts_with("```mermaid\nerDiagram\n    direction LR\n"), "{out}");
        assert!(out.ends_with("\n```"), "{out}");
    }

    #[test]
    fn types_and_names_are_sanitized_for_mermaid() {
        let out = run(
            "CREATE TABLE events (id INT PRIMARY KEY, created_at TIMESTAMP WITH TIME ZONE, amount NUMERIC(12,4));",
        );
        assert!(out.contains("TIMESTAMP_WITH_TIME_ZONE created_at"), "{out}");
        assert!(out.contains("NUMERIC(12_4) amount"), "{out}");
        assert!(!out.contains(','), "no bare commas inside attribute tokens: {out}");
    }

    #[test]
    fn schema_qualifiers_are_dropped_by_the_shared_parser() {
        // The shared extractor normalizes `public.users` to the bare table name,
        // so the entity needs no quoting and an FK that names the qualified table
        // still resolves to the same entity.
        let out = run(
            "CREATE TABLE public.users (id INT PRIMARY KEY);\n\
             CREATE TABLE public.orders (id INT PRIMARY KEY, user_id INT NOT NULL REFERENCES public.users(id));",
        );
        assert!(out.contains("    users {"), "{out}");
        assert!(!out.contains("public"), "{out}");
        assert!(out.contains("users ||--o{ orders : \"user_id\""), "{out}");
    }

    #[test]
    fn table_names_needing_quotes_are_quoted() {
        let out = run("CREATE TABLE \"order items\" (id INT PRIMARY KEY);");
        assert!(out.contains("\"order items\" {"), "{out}");
    }

    #[test]
    fn alter_table_foreign_keys_are_folded_in() {
        let out = run(
            "CREATE TABLE users (id INT PRIMARY KEY);\n\
             CREATE TABLE orders (id INT PRIMARY KEY, user_id INT NOT NULL);\n\
             ALTER TABLE orders ADD CONSTRAINT fk_u FOREIGN KEY (user_id) REFERENCES users(id);",
        );
        assert!(out.contains("users ||--o{ orders : \"user_id\""), "{out}");
    }

    #[test]
    fn a_unique_index_makes_the_relationship_one_to_one() {
        let out = run(
            "CREATE TABLE users (id INT PRIMARY KEY);\n\
             CREATE TABLE settings (id INT PRIMARY KEY, user_id INT NOT NULL REFERENCES users(id));\n\
             CREATE UNIQUE INDEX ux_settings_user ON settings (user_id);",
        );
        assert!(out.contains("users ||--o| settings"), "{out}");
    }

    #[test]
    fn table_cap_is_enforced_at_the_boundary() {
        let at_cap: String = (0..MAX_TABLES)
            .map(|i| format!("CREATE TABLE t{i} (id INT PRIMARY KEY);\n"))
            .collect();
        assert!(generate(&at_cap, "auto", "none", true, false, false, "column", "auto", false).is_ok());

        let over = format!("{at_cap}CREATE TABLE overflow (id INT PRIMARY KEY);\n");
        let err = generate(&over, "auto", "none", true, false, false, "column", "auto", false)
            .unwrap_err();
        assert!(err.contains("501 tables"), "{err}");
        assert!(err.contains("500-table limit"), "{err}");
    }
}
