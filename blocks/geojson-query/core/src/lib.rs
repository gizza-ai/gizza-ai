//! geojson-query core — filter, sort, page and aggregate the features of a
//! GeoJSON document with a small, deterministic SQL-like query language.
//!
//! Grammar (every clause optional except an implicit `SELECT *`):
//!
//! ```text
//! [SELECT <*|item[, item]…>] [FROM <name>] [WHERE <expr>]
//! [GROUP BY <field>[, <field>]…] [ORDER BY <key> [ASC|DESC]]
//! [LIMIT <n>] [OFFSET <n>]
//! ```
//!
//! * `item`  — `field [AS alias]` or `count(*) | count|sum|avg|min|max(field) [AS alias]`
//! * `expr`  — comparisons combined with `AND` / `OR` / `NOT` and parentheses:
//!   `field <op> value`, `field IN (a, b)`, `field IS [NOT] NULL`,
//!   `field LIKE 'A%'`, plus the spatial predicates `bbox(minLon, minLat,
//!   maxLon, maxLat)` (alias `intersects`), `within(minLon, minLat, maxLon,
//!   maxLat)`, `contains(lon, lat)` and `near(lon, lat, km)`.
//! * `field` — a property name, `$type` (geometry type) or `$id` (feature id).
//!
//! Coordinates are WGS84 lon/lat, as RFC 7946 requires. Pure `serde_json`
//! (`preserve_order`), no geometry engine and no network.

use serde::Serialize;
use serde_json::{Map, Value};

/// Largest accepted input document.
pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Largest accepted feature count.
pub const MAX_FEATURES: usize = 200_000;
/// Mean Earth radius (WGS84 authalic sphere), used by `near()`.
const EARTH_RADIUS_KM: f64 = 6371.0088;

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// Bare identifier or keyword.
    Word(String),
    /// `'quoted'` or `"quoted"` — a string literal, or a quoted field name.
    Str(String),
    Num(f64),
    Sym(String),
}

impl Tok {
    fn is_kw(&self, kw: &str) -> bool {
        matches!(self, Tok::Word(w) if w.eq_ignore_ascii_case(kw))
    }
    fn is_sym(&self, s: &str) -> bool {
        matches!(self, Tok::Sym(x) if x == s)
    }
    fn describe(&self) -> String {
        match self {
            Tok::Word(w) => format!("'{w}'"),
            Tok::Str(s) => format!("'{s}'"),
            Tok::Num(n) => format!("'{}'", trim_num(*n)),
            Tok::Sym(s) => format!("'{s}'"),
        }
    }
}

fn trim_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn tokenize(q: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = q.chars().collect();
    let mut i = 0usize;
    let mut out: Vec<Tok> = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            let quote = ch;
            i += 1;
            let mut s = String::new();
            loop {
                if i >= c.len() {
                    return Err(format!(
                        "unterminated {quote}…{quote} string in the query — close the quote"
                    ));
                }
                if c[i] == quote {
                    // A doubled quote inside the literal is an escaped quote.
                    if i + 1 < c.len() && c[i + 1] == quote {
                        s.push(quote);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(c[i]);
                i += 1;
            }
            out.push(Tok::Str(s));
            continue;
        }
        let numeric_start = ch.is_ascii_digit()
            || ((ch == '-' || ch == '+')
                && i + 1 < c.len()
                && (c[i + 1].is_ascii_digit() || c[i + 1] == '.'))
            || (ch == '.' && i + 1 < c.len() && c[i + 1].is_ascii_digit());
        if numeric_start {
            let start = i;
            if c[i] == '-' || c[i] == '+' {
                i += 1;
            }
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
                let save = i;
                i += 1;
                if i < c.len() && (c[i] == '+' || c[i] == '-') {
                    i += 1;
                }
                if i < c.len() && c[i].is_ascii_digit() {
                    while i < c.len() && c[i].is_ascii_digit() {
                        i += 1;
                    }
                } else {
                    i = save;
                }
            }
            let text: String = c[start..i].iter().collect();
            let n: f64 = text
                .parse()
                .map_err(|_| format!("'{text}' is not a valid number"))?;
            out.push(Tok::Num(n));
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' || ch == '$' {
            let start = i;
            while i < c.len()
                && (c[i].is_alphanumeric()
                    || c[i] == '_'
                    || c[i] == '$'
                    || c[i] == '.'
                    || c[i] == '-'
                    || c[i] == ':')
            {
                i += 1;
            }
            out.push(Tok::Word(c[start..i].iter().collect()));
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        if matches!(two.as_str(), "!=" | "<=" | ">=" | "==" | "<>") {
            out.push(Tok::Sym(two));
            i += 2;
            continue;
        }
        if "(),*=<>".contains(ch) {
            out.push(Tok::Sym(ch.to_string()));
            i += 1;
            continue;
        }
        return Err(format!("unexpected character '{ch}' in the query"));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Like,
    NotLike,
    Contains,
    StartsWith,
    EndsWith,
}

#[derive(Debug, Clone, PartialEq)]
enum Lit {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone)]
enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Cmp {
        field: String,
        op: Op,
        value: Lit,
    },
    In {
        field: String,
        values: Vec<Lit>,
        negated: bool,
    },
    IsNull {
        field: String,
        negated: bool,
    },
    /// Bounding-box test: `within = false` is "feature bbox intersects", `true`
    /// is "feature bbox lies entirely inside".
    Box4 {
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
        within: bool,
    },
    /// Point-in-feature (polygons; exact match for points).
    ContainsPoint {
        lon: f64,
        lat: f64,
    },
    /// Any vertex within `km` great-circle distance (0 if the point is inside).
    Near {
        lon: f64,
        lat: f64,
        km: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AggFunc {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

impl AggFunc {
    fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "count" => AggFunc::Count,
            "sum" => AggFunc::Sum,
            "avg" => AggFunc::Avg,
            "min" => AggFunc::Min,
            "max" => AggFunc::Max,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            AggFunc::Count => "count",
            AggFunc::Sum => "sum",
            AggFunc::Avg => "avg",
            AggFunc::Min => "min",
            AggFunc::Max => "max",
        }
    }
}

#[derive(Debug, Clone)]
enum SelItem {
    Field {
        name: String,
        alias: String,
    },
    Agg {
        func: AggFunc,
        field: String,
        alias: String,
    },
}

impl SelItem {
    fn alias(&self) -> &str {
        match self {
            SelItem::Field { alias, .. } => alias,
            SelItem::Agg { alias, .. } => alias,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Query {
    star: bool,
    items: Vec<SelItem>,
    filter: Option<Expr>,
    group_by: Vec<String>,
    order: Option<(String, bool)>,
    limit: Option<usize>,
    offset: usize,
}

impl Query {
    fn is_aggregate(&self) -> bool {
        !self.group_by.is_empty() || self.items.iter().any(|i| matches!(i, SelItem::Agg { .. }))
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

const CLAUSE_KEYWORDS: [&str; 7] = [
    "from", "where", "group", "order", "limit", "offset", "having",
];

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.peek().map(|t| t.is_kw(kw)).unwrap_or(false) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if self.peek().map(|t| t.is_sym(s)).unwrap_or(false) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str, ctx: &str) -> Result<(), String> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(match self.peek() {
                Some(t) => format!("expected '{s}' in {ctx}, found {}", t.describe()),
                None => format!("expected '{s}' at the end of {ctx}"),
            })
        }
    }
    fn at_clause_keyword(&self) -> bool {
        match self.peek() {
            Some(Tok::Word(w)) => {
                let lw = w.to_ascii_lowercase();
                CLAUSE_KEYWORDS.contains(&lw.as_str())
            }
            _ => false,
        }
    }
    /// A field name: a bare word or a quoted name.
    fn field_name(&mut self, ctx: &str) -> Result<String, String> {
        match self.next() {
            Some(Tok::Word(w)) => Ok(w),
            Some(Tok::Str(s)) => Ok(s),
            Some(t) => Err(format!(
                "expected a property name in {ctx}, found {}",
                t.describe()
            )),
            None => Err(format!("expected a property name in {ctx}")),
        }
    }
    fn number(&mut self, ctx: &str) -> Result<f64, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(n),
            Some(t) => Err(format!(
                "expected a number in {ctx}, found {}",
                t.describe()
            )),
            None => Err(format!("expected a number in {ctx}")),
        }
    }
}

fn parse_query(q: &str) -> Result<Query, String> {
    let toks = tokenize(q)?;
    if toks.is_empty() {
        return Err("query is empty — try 'SELECT *' or 'WHERE population > 100000'".into());
    }
    let mut p = Parser { toks, pos: 0 };
    let mut out = Query::default();

    if p.eat_kw("select") {
        parse_select_list(&mut p, &mut out)?;
    } else {
        // Implicit SELECT * when the query starts with a later clause.
        out.star = true;
        if !p.at_clause_keyword() {
            return Err(format!(
                "query must start with SELECT, WHERE, GROUP BY, ORDER BY, LIMIT or OFFSET — found {}",
                p.peek().map(|t| t.describe()).unwrap_or_default()
            ));
        }
    }

    // Optional FROM <name> — accepted for SQL familiarity, then ignored: the
    // only table is the input document's feature list.
    if p.eat_kw("from") {
        p.next().ok_or_else(|| {
            "FROM needs a name (any name — the input is the only table)".to_string()
        })?;
    }

    if p.eat_kw("where") {
        out.filter = Some(parse_or(&mut p)?);
    }

    if p.eat_kw("group") {
        if !p.eat_kw("by") {
            return Err("GROUP must be written 'GROUP BY <field>'".into());
        }
        loop {
            out.group_by.push(p.field_name("GROUP BY")?);
            if !p.eat_sym(",") {
                break;
            }
        }
    }

    if p.eat_kw("having") {
        return Err("HAVING is not supported — filter in WHERE, or post-process the rows".into());
    }

    if p.eat_kw("order") {
        if !p.eat_kw("by") {
            return Err("ORDER must be written 'ORDER BY <field> [ASC|DESC]'".into());
        }
        let key = parse_order_key(&mut p)?;
        let desc = if p.eat_kw("desc") {
            true
        } else {
            p.eat_kw("asc");
            false
        };
        out.order = Some((key, desc));
    }

    if p.eat_kw("limit") {
        let n = p.number("LIMIT")?;
        if n < 0.0 || n.fract() != 0.0 {
            return Err("LIMIT must be a whole number of features (0 or more)".into());
        }
        out.limit = Some(n as usize);
    }
    if p.eat_kw("offset") {
        let n = p.number("OFFSET")?;
        if n < 0.0 || n.fract() != 0.0 {
            return Err("OFFSET must be a whole number of features (0 or more)".into());
        }
        out.offset = n as usize;
    }
    // LIMIT after OFFSET is also valid SQL in several dialects.
    if out.limit.is_none() && p.eat_kw("limit") {
        let n = p.number("LIMIT")?;
        if n < 0.0 || n.fract() != 0.0 {
            return Err("LIMIT must be a whole number of features (0 or more)".into());
        }
        out.limit = Some(n as usize);
    }

    if let Some(t) = p.peek() {
        return Err(format!(
            "unexpected {} after the end of the query — clause order is SELECT … WHERE … GROUP BY … ORDER BY … LIMIT … OFFSET",
            t.describe()
        ));
    }

    validate_aggregate_shape(&out)?;
    Ok(out)
}

fn validate_aggregate_shape(q: &Query) -> Result<(), String> {
    if !q.is_aggregate() {
        return Ok(());
    }
    if q.star {
        return Err(
            "SELECT * cannot be combined with GROUP BY — list the grouped fields and the aggregates, e.g. 'SELECT country, count(*) GROUP BY country'"
                .into(),
        );
    }
    for item in &q.items {
        if let SelItem::Field { name, .. } = item {
            if !q.group_by.iter().any(|g| g == name) {
                return Err(format!(
                    "'{name}' is selected alongside an aggregate but is not in GROUP BY — add it to GROUP BY or wrap it in min()/max()"
                ));
            }
        }
    }
    if q.items.is_empty() {
        return Err("SELECT needs at least one field or aggregate".into());
    }
    Ok(())
}

fn parse_select_list(p: &mut Parser, out: &mut Query) -> Result<(), String> {
    if p.eat_sym("*") {
        out.star = true;
        if p.eat_sym(",") {
            return Err("SELECT * cannot be combined with other fields".into());
        }
        return Ok(());
    }
    loop {
        let item = parse_select_item(p)?;
        if out.items.iter().any(|i| i.alias() == item.alias()) {
            return Err(format!(
                "duplicate output name '{}' in SELECT — give one of them an alias with AS",
                item.alias()
            ));
        }
        out.items.push(item);
        if !p.eat_sym(",") {
            break;
        }
    }
    if out.items.is_empty() {
        return Err("SELECT needs '*' or a list of fields".into());
    }
    Ok(())
}

fn parse_select_item(p: &mut Parser) -> Result<SelItem, String> {
    // count(*) / sum(field) / …
    if let (Some(Tok::Word(w)), Some(t2)) = (p.peek().cloned(), p.toks.get(p.pos + 1).cloned()) {
        if t2.is_sym("(") {
            let func = AggFunc::parse(&w).ok_or_else(|| {
                format!("unknown function '{w}(…)' — supported: count, sum, avg, min, max")
            })?;
            p.pos += 2;
            let field = if p.eat_sym("*") {
                if func != AggFunc::Count {
                    return Err(format!(
                        "{}(*) is not valid — {}() needs a field name",
                        func.name(),
                        func.name()
                    ));
                }
                "*".to_string()
            } else {
                p.field_name(&format!("{}(…)", func.name()))?
            };
            p.expect_sym(")", &format!("{}(…)", func.name()))?;
            let default_alias = format!("{}({})", func.name(), field);
            let alias = parse_alias(p)?.unwrap_or(default_alias);
            return Ok(SelItem::Agg { func, field, alias });
        }
    }
    let name = p.field_name("SELECT")?;
    let alias = parse_alias(p)?.unwrap_or_else(|| name.clone());
    Ok(SelItem::Field { name, alias })
}

fn parse_alias(p: &mut Parser) -> Result<Option<String>, String> {
    if p.eat_kw("as") {
        return Ok(Some(p.field_name("AS")?));
    }
    Ok(None)
}

/// ORDER BY accepts a plain field/alias or an aggregate written the same way it
/// appears in SELECT (`count(*)`, `sum(pop)`).
fn parse_order_key(p: &mut Parser) -> Result<String, String> {
    if let (Some(Tok::Word(w)), Some(t2)) = (p.peek().cloned(), p.toks.get(p.pos + 1).cloned()) {
        if t2.is_sym("(") {
            if let Some(func) = AggFunc::parse(&w) {
                p.pos += 2;
                let field = if p.eat_sym("*") {
                    "*".to_string()
                } else {
                    p.field_name("ORDER BY")?
                };
                p.expect_sym(")", "ORDER BY")?;
                return Ok(format!("{}({})", func.name(), field));
            }
        }
    }
    p.field_name("ORDER BY")
}

fn parse_or(p: &mut Parser) -> Result<Expr, String> {
    let mut left = parse_and(p)?;
    while p.eat_kw("or") {
        let right = parse_and(p)?;
        left = Expr::Or(Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_and(p: &mut Parser) -> Result<Expr, String> {
    let mut left = parse_unary(p)?;
    while p.eat_kw("and") {
        let right = parse_unary(p)?;
        left = Expr::And(Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_unary(p: &mut Parser) -> Result<Expr, String> {
    if p.eat_kw("not") {
        return Ok(Expr::Not(Box::new(parse_unary(p)?)));
    }
    if p.eat_sym("(") {
        let inner = parse_or(p)?;
        p.expect_sym(")", "a parenthesised condition")?;
        return Ok(inner);
    }
    parse_predicate(p)
}

fn parse_predicate(p: &mut Parser) -> Result<Expr, String> {
    // Spatial function call.
    if let (Some(Tok::Word(w)), Some(t2)) = (p.peek().cloned(), p.toks.get(p.pos + 1).cloned()) {
        if t2.is_sym("(") {
            let lw = w.to_ascii_lowercase();
            if matches!(
                lw.as_str(),
                "bbox" | "intersects" | "within" | "contains" | "near"
            ) {
                p.pos += 2;
                return parse_spatial(p, &lw);
            }
            if AggFunc::parse(&lw).is_some() {
                return Err(format!(
                    "aggregates like {lw}(…) cannot be used in WHERE — they belong in SELECT"
                ));
            }
            return Err(format!(
                "unknown function '{w}(…)' in WHERE — supported: bbox, intersects, within, contains, near"
            ));
        }
    }

    let field = p.field_name("a WHERE condition")?;

    // IS [NOT] NULL
    if p.eat_kw("is") {
        let negated = p.eat_kw("not");
        if !p.eat_kw("null") {
            return Err("IS must be written 'IS NULL' or 'IS NOT NULL'".into());
        }
        return Ok(Expr::IsNull { field, negated });
    }

    // [NOT] IN (…) / [NOT] LIKE
    let negated_word = p.eat_kw("not");
    if p.eat_kw("in") {
        p.expect_sym("(", "IN (…)")?;
        let mut values = Vec::new();
        if !p.eat_sym(")") {
            loop {
                values.push(parse_literal(p, "IN (…)")?);
                if p.eat_sym(",") {
                    continue;
                }
                p.expect_sym(")", "IN (…)")?;
                break;
            }
        }
        if values.is_empty() {
            return Err("IN (…) needs at least one value".into());
        }
        return Ok(Expr::In {
            field,
            values,
            negated: negated_word,
        });
    }
    if p.eat_kw("like") {
        let value = parse_literal(p, "LIKE")?;
        let op = if negated_word { Op::NotLike } else { Op::Like };
        return Ok(Expr::Cmp { field, op, value });
    }
    if negated_word {
        return Err(format!(
            "expected IN or LIKE after 'NOT' in the condition on '{field}'"
        ));
    }

    for (kw, op) in [
        ("contains", Op::Contains),
        ("starts_with", Op::StartsWith),
        ("ends_with", Op::EndsWith),
    ] {
        if p.eat_kw(kw) {
            let value = parse_literal(p, kw)?;
            return Ok(Expr::Cmp { field, op, value });
        }
    }

    let op = match p.next() {
        Some(Tok::Sym(s)) => match s.as_str() {
            "=" | "==" => Op::Eq,
            "!=" | "<>" => Op::Ne,
            "<" => Op::Lt,
            "<=" => Op::Le,
            ">" => Op::Gt,
            ">=" => Op::Ge,
            other => {
                return Err(format!(
                    "'{other}' is not a comparison operator — use = != < <= > >= IN LIKE CONTAINS STARTS_WITH ENDS_WITH IS NULL"
                ))
            }
        },
        Some(t) => {
            return Err(format!(
                "expected a comparison operator after '{field}', found {}",
                t.describe()
            ))
        }
        None => {
            return Err(format!(
                "condition on '{field}' is incomplete — write it as '{field} = value'"
            ))
        }
    };
    let value = parse_literal(p, "the condition")?;
    Ok(Expr::Cmp { field, op, value })
}

fn parse_spatial(p: &mut Parser, name: &str) -> Result<Expr, String> {
    let mut args: Vec<f64> = Vec::new();
    if !p.eat_sym(")") {
        loop {
            args.push(p.number(&format!("{name}(…)"))?);
            if p.eat_sym(",") {
                continue;
            }
            p.expect_sym(")", &format!("{name}(…)"))?;
            break;
        }
    }
    match name {
        "bbox" | "intersects" | "within" => {
            if args.len() != 4 {
                return Err(format!(
                    "{name}() takes 4 numbers: {name}(minLon, minLat, maxLon, maxLat) — got {}",
                    args.len()
                ));
            }
            let (min_lon, min_lat, max_lon, max_lat) =
                normalize_bbox(args[0], args[1], args[2], args[3]);
            Ok(Expr::Box4 {
                min_lon,
                min_lat,
                max_lon,
                max_lat,
                within: name == "within",
            })
        }
        "contains" => {
            if args.len() != 2 {
                return Err(format!(
                    "contains() takes 2 numbers: contains(lon, lat) — got {}",
                    args.len()
                ));
            }
            Ok(Expr::ContainsPoint {
                lon: args[0],
                lat: args[1],
            })
        }
        "near" => {
            if args.len() != 3 {
                return Err(format!(
                    "near() takes 3 numbers: near(lon, lat, km) — got {}",
                    args.len()
                ));
            }
            if args[2] < 0.0 {
                return Err("near() radius must be 0 or more kilometres".into());
            }
            Ok(Expr::Near {
                lon: args[0],
                lat: args[1],
                km: args[2],
            })
        }
        _ => unreachable!(),
    }
}

fn normalize_bbox(a: f64, b: f64, c: f64, d: f64) -> (f64, f64, f64, f64) {
    (a.min(c), b.min(d), a.max(c), b.max(d))
}

fn parse_literal(p: &mut Parser, ctx: &str) -> Result<Lit, String> {
    match p.next() {
        Some(Tok::Num(n)) => Ok(Lit::Num(n)),
        Some(Tok::Str(s)) => Ok(Lit::Str(s)),
        Some(Tok::Word(w)) => Ok(match w.to_ascii_lowercase().as_str() {
            "true" => Lit::Bool(true),
            "false" => Lit::Bool(false),
            "null" => Lit::Null,
            _ => Lit::Str(w),
        }),
        Some(t) => Err(format!("expected a value in {ctx}, found {}", t.describe())),
        None => Err(format!("expected a value in {ctx}")),
    }
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

fn geometry_type(geom: Option<&Value>) -> Option<String> {
    geom?.get("type")?.as_str().map(|s| s.to_string())
}

/// Walk every `[lon, lat, …]` position of a geometry.
fn for_each_position(geom: &Value, f: &mut impl FnMut(f64, f64)) {
    let Some(t) = geom.get("type").and_then(|t| t.as_str()) else {
        return;
    };
    if t == "GeometryCollection" {
        if let Some(list) = geom.get("geometries").and_then(|g| g.as_array()) {
            for g in list {
                for_each_position(g, f);
            }
        }
        return;
    }
    if let Some(coords) = geom.get("coordinates") {
        walk_coords(coords, f);
    }
}

fn walk_coords(v: &Value, f: &mut impl FnMut(f64, f64)) {
    let Some(arr) = v.as_array() else { return };
    if arr.len() >= 2 && arr[0].is_number() && arr[1].is_number() {
        if let (Some(x), Some(y)) = (arr[0].as_f64(), arr[1].as_f64()) {
            f(x, y);
        }
        return;
    }
    for item in arr {
        walk_coords(item, f);
    }
}

fn compute_bbox(geom: &Value) -> Option<[f64; 4]> {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut any = false;
    for_each_position(geom, &mut |x, y| {
        any = true;
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    });
    if any && b.iter().all(|v| v.is_finite()) {
        Some(b)
    } else {
        None
    }
}

fn boxes_intersect(a: &[f64; 4], q: &(f64, f64, f64, f64)) -> bool {
    a[0] <= q.2 && a[2] >= q.0 && a[1] <= q.3 && a[3] >= q.1
}

fn box_within(a: &[f64; 4], q: &(f64, f64, f64, f64)) -> bool {
    a[0] >= q.0 && a[2] <= q.2 && a[1] >= q.1 && a[3] <= q.3
}

/// Ray-casting point-in-ring test on the raw lon/lat plane.
fn point_in_ring(ring: &Value, lon: f64, lat: f64) -> bool {
    let Some(pts) = ring.as_array() else {
        return false;
    };
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    if pts.is_empty() {
        return false;
    }
    for i in 0..pts.len() {
        let (xi, yi) = match pair(&pts[i]) {
            Some(p) => p,
            None => return false,
        };
        let (xj, yj) = match pair(&pts[j]) {
            Some(p) => p,
            None => return false,
        };
        if (yi > lat) != (yj > lat) {
            let t = (lat - yi) / (yj - yi);
            if lon < xi + t * (xj - xi) {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn pair(v: &Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

fn polygon_contains(rings: &Value, lon: f64, lat: f64) -> bool {
    let Some(rings) = rings.as_array() else {
        return false;
    };
    let Some(outer) = rings.first() else {
        return false;
    };
    if !point_in_ring(outer, lon, lat) {
        return false;
    }
    !rings
        .iter()
        .skip(1)
        .any(|hole| point_in_ring(hole, lon, lat))
}

fn geometry_contains_point(geom: &Value, lon: f64, lat: f64) -> bool {
    let Some(t) = geom.get("type").and_then(|t| t.as_str()) else {
        return false;
    };
    match t {
        "Polygon" => geom
            .get("coordinates")
            .map(|c| polygon_contains(c, lon, lat))
            .unwrap_or(false),
        "MultiPolygon" => geom
            .get("coordinates")
            .and_then(|c| c.as_array())
            .map(|polys| polys.iter().any(|p| polygon_contains(p, lon, lat)))
            .unwrap_or(false),
        "Point" => geom
            .get("coordinates")
            .and_then(pair)
            .map(|(x, y)| x == lon && y == lat)
            .unwrap_or(false),
        "MultiPoint" => geom
            .get("coordinates")
            .and_then(|c| c.as_array())
            .map(|pts| {
                pts.iter()
                    .filter_map(pair)
                    .any(|(x, y)| x == lon && y == lat)
            })
            .unwrap_or(false),
        "GeometryCollection" => geom
            .get("geometries")
            .and_then(|g| g.as_array())
            .map(|list| list.iter().any(|g| geometry_contains_point(g, lon, lat)))
            .unwrap_or(false),
        _ => false,
    }
}

fn haversine_km(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().clamp(-1.0, 1.0).asin()
}

fn min_vertex_distance_km(geom: &Value, lon: f64, lat: f64) -> Option<f64> {
    let mut best = f64::INFINITY;
    let mut any = false;
    for_each_position(geom, &mut |x, y| {
        any = true;
        let d = haversine_km(lon, lat, x, y);
        if d < best {
            best = d;
        }
    });
    if any {
        Some(best)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Feature model
// ---------------------------------------------------------------------------

struct Feat {
    value: Value,
    gtype: Option<String>,
    bbox: Option<[f64; 4]>,
}

impl Feat {
    fn geometry(&self) -> Option<&Value> {
        self.value.get("geometry").filter(|g| !g.is_null())
    }
    fn properties(&self) -> Option<&Map<String, Value>> {
        self.value.get("properties").and_then(|p| p.as_object())
    }
    /// Resolve a query field to a feature value. `$type`/`$id` are the geometry
    /// type and feature id; every other name is a property, with `geometry_type`
    /// and `id` accepted as fallbacks when no such property exists. Explicit
    /// JSON `null` reads as missing, so `IS NULL` catches both.
    fn field(&self, name: &str) -> Option<Value> {
        if name.eq_ignore_ascii_case("$type") {
            return self.gtype.clone().map(Value::String);
        }
        if name.eq_ignore_ascii_case("$id") {
            return self.value.get("id").filter(|v| !v.is_null()).cloned();
        }
        if let Some(v) = self.properties().and_then(|p| p.get(name)) {
            return if v.is_null() { None } else { Some(v.clone()) };
        }
        if name.eq_ignore_ascii_case("geometry_type") {
            return self.gtype.clone().map(Value::String);
        }
        if name.eq_ignore_ascii_case("id") {
            return self.value.get("id").filter(|v| !v.is_null()).cloned();
        }
        None
    }
}

fn to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn as_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                t.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

fn lit_number(l: &Lit) -> Option<f64> {
    match l {
        Lit::Num(n) => Some(*n),
        Lit::Str(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn lit_text(l: &Lit) -> String {
    match l {
        Lit::Num(n) => trim_num(*n),
        Lit::Str(s) => s.clone(),
        Lit::Bool(b) => b.to_string(),
        Lit::Null => String::new(),
    }
}

/// SQL `LIKE` with `%` (any run) and `_` (one character), case-sensitive.
fn like_match(text: &str, pattern: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let (mut ti, mut pi) = (0usize, 0usize);
    let (mut star_p, mut star_t) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '_' || p[pi] == t[ti]) {
            ti += 1;
            pi += 1;
        } else if pi < p.len() && p[pi] == '%' {
            star_p = pi;
            star_t = ti;
            pi += 1;
        } else if star_p != usize::MAX {
            pi = star_p + 1;
            star_t += 1;
            ti = star_t;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '%' {
        pi += 1;
    }
    pi == p.len()
}

fn compare(value: Option<&Value>, op: Op, lit: &Lit) -> bool {
    let Some(v) = value else {
        // Missing / null: only `!= x` and `= null` are true.
        return match (op, lit) {
            (Op::Eq, Lit::Null) => true,
            (Op::Ne, Lit::Null) => false,
            (Op::Ne, _) => true,
            _ => false,
        };
    };
    if matches!(lit, Lit::Null) {
        return matches!(op, Op::Ne);
    }
    match op {
        Op::Like => return like_match(&to_text(v), &lit_text(lit)),
        Op::NotLike => return !like_match(&to_text(v), &lit_text(lit)),
        Op::Contains => {
            return to_text(v)
                .to_lowercase()
                .contains(&lit_text(lit).to_lowercase())
        }
        Op::StartsWith => {
            return to_text(v)
                .to_lowercase()
                .starts_with(&lit_text(lit).to_lowercase())
        }
        Op::EndsWith => {
            return to_text(v)
                .to_lowercase()
                .ends_with(&lit_text(lit).to_lowercase())
        }
        _ => {}
    }
    if let (Value::Bool(b), Lit::Bool(l)) = (v, lit) {
        return match op {
            Op::Eq => b == l,
            Op::Ne => b != l,
            _ => false,
        };
    }
    if let (Some(a), Some(b)) = (as_number(v), lit_number(lit)) {
        return match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            Op::Lt => a < b,
            Op::Le => a <= b,
            Op::Gt => a > b,
            Op::Ge => a >= b,
            _ => false,
        };
    }
    let (a, b) = (to_text(v), lit_text(lit));
    match op {
        Op::Eq => a == b,
        Op::Ne => a != b,
        Op::Lt => a < b,
        Op::Le => a <= b,
        Op::Gt => a > b,
        Op::Ge => a >= b,
        _ => false,
    }
}

fn eval(expr: &Expr, f: &Feat) -> bool {
    match expr {
        Expr::And(a, b) => eval(a, f) && eval(b, f),
        Expr::Or(a, b) => eval(a, f) || eval(b, f),
        Expr::Not(a) => !eval(a, f),
        Expr::Cmp { field, op, value } => compare(f.field(field).as_ref(), *op, value),
        Expr::In {
            field,
            values,
            negated,
        } => {
            let v = f.field(field);
            let hit = values.iter().any(|l| compare(v.as_ref(), Op::Eq, l));
            if *negated {
                v.is_some() && !hit
            } else {
                hit
            }
        }
        Expr::IsNull { field, negated } => {
            let present = f.field(field).is_some();
            if *negated {
                present
            } else {
                !present
            }
        }
        Expr::Box4 {
            min_lon,
            min_lat,
            max_lon,
            max_lat,
            within,
        } => {
            let q = (*min_lon, *min_lat, *max_lon, *max_lat);
            match &f.bbox {
                Some(b) => {
                    if *within {
                        box_within(b, &q)
                    } else {
                        boxes_intersect(b, &q)
                    }
                }
                None => false,
            }
        }
        Expr::ContainsPoint { lon, lat } => f
            .geometry()
            .map(|g| geometry_contains_point(g, *lon, *lat))
            .unwrap_or(false),
        Expr::Near { lon, lat, km } => match f.geometry() {
            Some(g) => {
                if geometry_contains_point(g, *lon, *lat) {
                    return true;
                }
                min_vertex_distance_km(g, *lon, *lat)
                    .map(|d| d <= *km)
                    .unwrap_or(false)
            }
            None => false,
        },
    }
}

fn needs_bbox(expr: &Expr) -> bool {
    match expr {
        Expr::And(a, b) | Expr::Or(a, b) => needs_bbox(a) || needs_bbox(b),
        Expr::Not(a) => needs_bbox(a),
        Expr::Box4 { .. } => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Input parsing
// ---------------------------------------------------------------------------

fn load_features(input: &str) -> Result<Vec<Feat>, String> {
    if input.trim().is_empty() {
        return Err(
            "GeoJSON input is empty — paste a FeatureCollection, Feature or geometry".into(),
        );
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "GeoJSON input is {:.1} MB — the limit is {} MB. Split the file or pre-filter it.",
            input.len() as f64 / 1_048_576.0,
            MAX_INPUT_BYTES / 1_048_576
        ));
    }
    let doc: Value =
        serde_json::from_str(input).map_err(|e| format!("GeoJSON parse error: {e}"))?;
    let raw: Vec<Value> = match doc.get("type").and_then(|t| t.as_str()) {
        Some("FeatureCollection") => match doc.get("features") {
            Some(Value::Array(a)) => a.clone(),
            _ => return Err("FeatureCollection is missing its \"features\" array".into()),
        },
        Some("Feature") => vec![doc.clone()],
        Some(t)
            if [
                "Point",
                "MultiPoint",
                "LineString",
                "MultiLineString",
                "Polygon",
                "MultiPolygon",
                "GeometryCollection",
            ]
            .contains(&t) =>
        {
            // Wrap a bare geometry so it can be queried like a one-feature collection.
            let mut m = Map::new();
            m.insert("type".into(), Value::String("Feature".into()));
            m.insert("geometry".into(), doc.clone());
            m.insert("properties".into(), Value::Object(Map::new()));
            vec![Value::Object(m)]
        }
        _ => match &doc {
            Value::Array(a) => a.clone(),
            _ => {
                return Err(
                    "input is not GeoJSON — expected a FeatureCollection, a Feature, a geometry, or an array of Features"
                        .into(),
                )
            }
        },
    };
    if raw.len() > MAX_FEATURES {
        return Err(format!(
            "input has {} features — the limit is {MAX_FEATURES}. Pre-filter the file first.",
            raw.len()
        ));
    }
    let mut out = Vec::with_capacity(raw.len());
    for (i, v) in raw.into_iter().enumerate() {
        if !v.is_object() {
            return Err(format!("feature #{} is not a JSON object", i + 1));
        }
        let gtype = geometry_type(v.get("geometry").filter(|g| !g.is_null()));
        out.push(Feat {
            value: v,
            gtype,
            bbox: None,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn to_json_string(v: &Value, indent: i64) -> String {
    if indent <= 0 {
        return v.to_string();
    }
    let pad = " ".repeat(indent.min(8) as usize);
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    if v.serialize(&mut ser).is_err() {
        return v.to_string();
    }
    String::from_utf8(buf).unwrap_or_else(|_| v.to_string())
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn rows_to_csv(columns: &[String], rows: &[Map<String, Value>]) -> String {
    let mut out = String::new();
    out.push_str(
        &columns
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for row in rows {
        let line: Vec<String> = columns
            .iter()
            .map(|c| csv_escape(&row.get(c).map(to_text).unwrap_or_default()))
            .collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}

fn number_value(n: f64) -> Value {
    if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
        return Value::Number(serde_json::Number::from(n as i64));
    }
    serde_json::Number::from_f64(n)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AggState {
    count: u64,
    sum: f64,
    numeric_count: u64,
    non_numeric: bool,
    min_num: Option<f64>,
    max_num: Option<f64>,
    min_str: Option<String>,
    max_str: Option<String>,
}

impl AggState {
    fn push(&mut self, v: Option<&Value>) {
        let Some(v) = v else { return };
        self.count += 1;
        match as_number(v) {
            Some(n) => {
                self.numeric_count += 1;
                self.sum += n;
                self.min_num = Some(self.min_num.map_or(n, |m| m.min(n)));
                self.max_num = Some(self.max_num.map_or(n, |m| m.max(n)));
            }
            None => self.non_numeric = true,
        }
        let t = to_text(v);
        if self.min_str.as_ref().map(|m| &t < m).unwrap_or(true) {
            self.min_str = Some(t.clone());
        }
        if self.max_str.as_ref().map(|m| &t > m).unwrap_or(true) {
            self.max_str = Some(t);
        }
    }
    /// `count(field)` counts only the rows where the field is present, matching
    /// SQL; `count(*)` never reaches here (the group's row count is used).
    fn finish(&self, func: AggFunc) -> Value {
        match func {
            AggFunc::Count => Value::Number(serde_json::Number::from(self.count)),
            AggFunc::Sum => {
                if self.numeric_count == 0 {
                    Value::Null
                } else {
                    number_value(self.sum)
                }
            }
            AggFunc::Avg => {
                if self.numeric_count == 0 {
                    Value::Null
                } else {
                    number_value(self.sum / self.numeric_count as f64)
                }
            }
            AggFunc::Min => self.extreme(true),
            AggFunc::Max => self.extreme(false),
        }
    }
    fn extreme(&self, min: bool) -> Value {
        if self.count == 0 {
            return Value::Null;
        }
        if !self.non_numeric {
            let n = if min { self.min_num } else { self.max_num };
            return n.map(number_value).unwrap_or(Value::Null);
        }
        let s = if min { &self.min_str } else { &self.max_str };
        s.clone().map(Value::String).unwrap_or(Value::Null)
    }
}

struct Group {
    keys: Vec<Option<Value>>,
    rows: u64,
    aggs: Vec<AggState>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run `query` over the GeoJSON in `input`.
///
/// * `bbox` — optional convenience filter `minLon,minLat,maxLon,maxLat`, ANDed
///   with the query's own `WHERE`.
/// * `format` — `geojson` (FeatureCollection), `json` (property rows) or `csv`.
///   Aggregate queries always emit rows, so `geojson` degrades to `json` there.
/// * `indent` — 0 for compact JSON, otherwise the number of spaces (max 8).
/// * `include_bbox` — add a computed `bbox` member to the output collection.
pub fn query(
    input: &str,
    query: &str,
    bbox: &str,
    format: &str,
    indent: i64,
    include_bbox: bool,
) -> Result<String, String> {
    let fmt = match format.trim().to_ascii_lowercase().as_str() {
        "" | "geojson" => "geojson",
        "json" => "json",
        "csv" => "csv",
        other => {
            return Err(format!(
                "unknown format '{other}' — use 'geojson', 'json' or 'csv'"
            ))
        }
    };
    if !(0..=8).contains(&indent) {
        return Err("indent must be between 0 and 8 spaces".into());
    }
    let q = parse_query(query)?;
    let bbox_filter = parse_bbox_param(bbox)?;
    let mut feats = load_features(input)?;

    let want_bbox =
        include_bbox || bbox_filter.is_some() || q.filter.as_ref().map(needs_bbox).unwrap_or(false);
    if want_bbox {
        for f in feats.iter_mut() {
            f.bbox = f.geometry().and_then(compute_bbox);
        }
    }

    // Filter.
    let mut kept: Vec<&Feat> = Vec::new();
    for f in feats.iter() {
        if let Some(q4) = &bbox_filter {
            match &f.bbox {
                Some(b) if boxes_intersect(b, q4) => {}
                _ => continue,
            }
        }
        if let Some(expr) = &q.filter {
            if !eval(expr, f) {
                continue;
            }
        }
        kept.push(f);
    }

    if q.is_aggregate() {
        aggregate_output(&q, &kept, fmt, indent)
    } else {
        feature_output(&q, kept, fmt, indent, include_bbox)
    }
}

fn parse_bbox_param(bbox: &str) -> Result<Option<(f64, f64, f64, f64)>, String> {
    let t = bbox.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = t
        .split([',', ' ', '\t', '\n'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 4 {
        return Err(format!(
            "bbox needs 4 numbers 'minLon,minLat,maxLon,maxLat' — got {} value(s)",
            parts.len()
        ));
    }
    let mut n = [0f64; 4];
    for (i, p) in parts.iter().enumerate() {
        n[i] = p
            .parse::<f64>()
            .map_err(|_| format!("bbox value '{p}' is not a number"))?;
    }
    Ok(Some(normalize_bbox(n[0], n[1], n[2], n[3])))
}

/// Sort key: numbers before strings, missing values always last.
fn sort_cmp(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => match (as_number(x), as_number(y)) {
            (Some(p), Some(q)) => p.partial_cmp(&q).unwrap_or(Ordering::Equal),
            _ => to_text(x).cmp(&to_text(y)),
        },
    }
}

fn apply_paging<T>(rows: &mut Vec<T>, q: &Query) {
    if q.offset > 0 {
        if q.offset >= rows.len() {
            rows.clear();
        } else {
            rows.drain(0..q.offset);
        }
    }
    if let Some(n) = q.limit {
        rows.truncate(n);
    }
}

fn feature_output(
    q: &Query,
    mut kept: Vec<&Feat>,
    fmt: &str,
    indent: i64,
    include_bbox: bool,
) -> Result<String, String> {
    if let Some((key, desc)) = &q.order {
        // ORDER BY may name an output alias; map it back to the source field.
        let field = q
            .items
            .iter()
            .find(|i| i.alias() == key)
            .and_then(|i| match i {
                SelItem::Field { name, .. } => Some(name.clone()),
                SelItem::Agg { .. } => None,
            })
            .unwrap_or_else(|| key.clone());
        kept.sort_by(|a, b| {
            let ord = sort_cmp(a.field(&field).as_ref(), b.field(&field).as_ref());
            // Missing values stay last in both directions.
            match (a.field(&field), b.field(&field)) {
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                _ if *desc => ord.reverse(),
                _ => ord,
            }
        });
    }
    apply_paging(&mut kept, q);

    match fmt {
        "geojson" => {
            let mut features = Vec::with_capacity(kept.len());
            for f in &kept {
                features.push(project_feature(q, f));
            }
            let mut root = Map::new();
            root.insert("type".into(), Value::String("FeatureCollection".into()));
            if include_bbox {
                if let Some(b) = collection_bbox(&kept) {
                    root.insert(
                        "bbox".into(),
                        Value::Array(b.iter().map(|v| number_value(*v)).collect()),
                    );
                }
            }
            root.insert("features".into(), Value::Array(features));
            Ok(to_json_string(&Value::Object(root), indent))
        }
        _ => {
            let rows: Vec<Map<String, Value>> = kept.iter().map(|f| property_row(q, f)).collect();
            if fmt == "csv" {
                let columns = row_columns(q, &rows);
                Ok(rows_to_csv(&columns, &rows))
            } else {
                let arr = Value::Array(rows.into_iter().map(Value::Object).collect());
                Ok(to_json_string(&arr, indent))
            }
        }
    }
}

fn collection_bbox(kept: &[&Feat]) -> Option<[f64; 4]> {
    let mut acc: Option<[f64; 4]> = None;
    for f in kept {
        let b = match &f.bbox {
            Some(b) => *b,
            None => match f.geometry().and_then(compute_bbox) {
                Some(b) => b,
                None => continue,
            },
        };
        acc = Some(match acc {
            None => b,
            Some(a) => [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ],
        });
    }
    acc
}

fn project_feature(q: &Query, f: &Feat) -> Value {
    if q.star {
        return f.value.clone();
    }
    let mut out = f.value.clone();
    let mut props = Map::new();
    for item in &q.items {
        if let SelItem::Field { name, alias } = item {
            if let Some(v) = f.field(name) {
                props.insert(alias.clone(), v);
            }
        }
    }
    if let Some(obj) = out.as_object_mut() {
        obj.insert("properties".into(), Value::Object(props));
    }
    out
}

fn property_row(q: &Query, f: &Feat) -> Map<String, Value> {
    if q.star {
        return f.properties().cloned().unwrap_or_default();
    }
    let mut row = Map::new();
    for item in &q.items {
        if let SelItem::Field { name, alias } = item {
            row.insert(alias.clone(), f.field(name).unwrap_or(Value::Null));
        }
    }
    row
}

/// CSV columns: the SELECT order when explicit, otherwise every key seen, in
/// first-appearance order.
fn row_columns(q: &Query, rows: &[Map<String, Value>]) -> Vec<String> {
    if !q.star {
        return q.items.iter().map(|i| i.alias().to_string()).collect();
    }
    let mut cols: Vec<String> = Vec::new();
    for row in rows {
        for k in row.keys() {
            if !cols.iter().any(|c| c == k) {
                cols.push(k.clone());
            }
        }
    }
    cols
}

fn aggregate_output(q: &Query, kept: &[&Feat], fmt: &str, indent: i64) -> Result<String, String> {
    let aggs: Vec<(usize, AggFunc, String)> = q
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| match it {
            SelItem::Agg { func, field, .. } => Some((i, *func, field.clone())),
            _ => None,
        })
        .collect();

    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Group> = std::collections::HashMap::new();

    for f in kept {
        let keys: Vec<Option<Value>> = q.group_by.iter().map(|g| f.field(g)).collect();
        let gk = keys
            .iter()
            .map(|k| match k {
                Some(v) => format!("s{}", to_text(v)),
                None => "n".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\u{1f}");
        let entry = groups.entry(gk.clone()).or_insert_with(|| {
            order.push(gk.clone());
            Group {
                keys: keys.clone(),
                rows: 0,
                aggs: aggs.iter().map(|_| AggState::default()).collect(),
            }
        });
        entry.rows += 1;
        for (slot, (_, func, field)) in aggs.iter().enumerate() {
            if *func == AggFunc::Count && field == "*" {
                continue;
            }
            let v = f.field(field);
            entry.aggs[slot].push(v.as_ref());
        }
    }

    // With no GROUP BY, an aggregate query always returns exactly one row.
    if q.group_by.is_empty() && groups.is_empty() {
        order.push(String::new());
        groups.insert(
            String::new(),
            Group {
                keys: Vec::new(),
                rows: 0,
                aggs: aggs.iter().map(|_| AggState::default()).collect(),
            },
        );
    }

    let mut rows: Vec<Map<String, Value>> = Vec::with_capacity(order.len());
    for gk in &order {
        let g = &groups[gk];
        let mut row = Map::new();
        let mut key_idx = 0usize;
        let mut agg_slot = 0usize;
        for item in &q.items {
            match item {
                SelItem::Field { name, alias } => {
                    let pos = q.group_by.iter().position(|x| x == name).unwrap_or(key_idx);
                    row.insert(
                        alias.clone(),
                        g.keys.get(pos).cloned().flatten().unwrap_or(Value::Null),
                    );
                    key_idx += 1;
                }
                SelItem::Agg { func, field, alias } => {
                    let value = if *func == AggFunc::Count && field == "*" {
                        Value::Number(serde_json::Number::from(g.rows))
                    } else {
                        g.aggs[agg_slot].finish(*func)
                    };
                    row.insert(alias.clone(), value);
                    agg_slot += 1;
                }
            }
        }
        rows.push(row);
    }

    if let Some((key, desc)) = &q.order {
        if !q.items.iter().any(|i| i.alias() == key) && !q.group_by.iter().any(|g| g == key) {
            return Err(format!(
                "ORDER BY '{key}' is not one of the selected output names — order by a selected field, alias or aggregate"
            ));
        }
        rows.sort_by(|a, b| {
            let ord = sort_cmp(a.get(key), b.get(key));
            if *desc {
                ord.reverse()
            } else {
                ord
            }
        });
    }
    apply_paging(&mut rows, q);

    if fmt == "csv" {
        let columns: Vec<String> = q.items.iter().map(|i| i.alias().to_string()).collect();
        Ok(rows_to_csv(&columns, &rows))
    } else {
        let arr = Value::Array(rows.into_iter().map(Value::Object).collect());
        Ok(to_json_string(&arr, indent))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = r#"{
      "type": "FeatureCollection",
      "features": [
        {"type":"Feature","id":"a","properties":{"name":"Springfield","country":"US","population":120000},
         "geometry":{"type":"Point","coordinates":[-89.65,39.78]}},
        {"type":"Feature","id":"b","properties":{"name":"Shelbyville","country":"US","population":30000},
         "geometry":{"type":"Point","coordinates":[-88.79,39.40]}},
        {"type":"Feature","id":"c","properties":{"name":"Ogdenville","country":"CA","population":250000},
         "geometry":{"type":"Point","coordinates":[-114.07,51.05]}},
        {"type":"Feature","id":"d","properties":{"name":"Zone","country":"US"},
         "geometry":{"type":"Polygon","coordinates":[[[-90,39],[-89,39],[-89,40],[-90,40],[-90,39]]]}}
      ]
    }"#;

    fn names(out: &str) -> Vec<String> {
        let v: Value = serde_json::from_str(out).unwrap();
        v["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["properties"]["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn select_star_returns_everything() {
        let out = query(FC, "SELECT *", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out).len(), 4);
        assert!(out.starts_with(r#"{"type":"FeatureCollection","features":["#));
    }

    #[test]
    fn numeric_where() {
        let out = query(
            FC,
            "SELECT * WHERE population > 100000",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Springfield", "Ogdenville"]);
    }

    #[test]
    fn implicit_select_star() {
        let out = query(FC, "WHERE country = US", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out).len(), 3);
    }

    #[test]
    fn and_or_not_and_parens() {
        let out = query(
            FC,
            "SELECT * WHERE (country = 'US' AND population >= 100000) OR country = 'CA'",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Springfield", "Ogdenville"]);
        let out = query(FC, "WHERE NOT country = 'US'", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Ogdenville"]);
    }

    #[test]
    fn in_like_and_text_predicates() {
        let out = query(
            FC,
            "WHERE name IN ('Shelbyville', 'Ogdenville')",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Shelbyville", "Ogdenville"]);
        let out = query(FC, "WHERE name LIKE 'S%e'", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Shelbyville"]);
        let out = query(FC, "WHERE name STARTS_WITH 'og'", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Ogdenville"]);
        let out = query(FC, "WHERE name CONTAINS 'field'", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Springfield"]);
        let out = query(FC, "WHERE name NOT LIKE '%ville'", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Springfield", "Zone"]);
    }

    #[test]
    fn is_null_finds_missing_properties() {
        let out = query(FC, "WHERE population IS NULL", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Zone"]);
        let out = query(FC, "WHERE population IS NOT NULL", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out).len(), 3);
    }

    #[test]
    fn geometry_type_field() {
        let out = query(FC, "WHERE $type = 'Polygon'", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Zone"]);
        let out = query(
            FC,
            "WHERE geometry_type != 'Polygon'",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out).len(), 3);
    }

    #[test]
    fn feature_id_field() {
        let out = query(FC, "WHERE $id = 'c'", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Ogdenville"]);
    }

    #[test]
    fn bbox_predicate_and_param() {
        let out = query(FC, "WHERE bbox(-90, 39, -88, 40)", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Springfield", "Shelbyville", "Zone"]);
        let out = query(FC, "SELECT *", "-90,39,-88,40", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Springfield", "Shelbyville", "Zone"]);
        // within() demands full containment, so the wide polygon drops out.
        let out = query(
            FC,
            "WHERE within(-89.9, 39.2, -88.5, 39.9)",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Springfield", "Shelbyville"]);
    }

    #[test]
    fn contains_point_and_near() {
        let out = query(FC, "WHERE contains(-89.5, 39.5)", "", "geojson", 0, false).unwrap();
        assert_eq!(names(&out), vec!["Zone"]);
        // ~48 km from Springfield, far from everything else.
        let out = query(
            FC,
            "WHERE near(-89.65, 39.35, 50) AND $type = 'Point'",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Springfield"]);
    }

    #[test]
    fn projection_keeps_geometry_and_drops_other_properties() {
        let out = query(
            FC,
            "SELECT name, population WHERE country = 'CA'",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let f = &v["features"][0];
        assert_eq!(f["properties"]["name"], "Ogdenville");
        assert!(f["properties"].get("country").is_none());
        assert_eq!(f["geometry"]["type"], "Point");
    }

    #[test]
    fn order_limit_offset() {
        let out = query(
            FC,
            "SELECT * ORDER BY population DESC LIMIT 2",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Ogdenville", "Springfield"]);
        let out = query(
            FC,
            "SELECT * ORDER BY population DESC LIMIT 1 OFFSET 1",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Springfield"]);
        // Missing values sort last in both directions.
        let out = query(
            FC,
            "SELECT * ORDER BY population ASC",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out).last().unwrap(), "Zone");
    }

    #[test]
    fn json_and_csv_output() {
        let out = query(
            FC,
            "SELECT name, population WHERE country = 'US' ORDER BY name",
            "",
            "json",
            0,
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[{"name":"Shelbyville","population":30000},{"name":"Springfield","population":120000},{"name":"Zone","population":null}]"#
        );
        let out = query(
            FC,
            "SELECT name, population WHERE country = 'CA'",
            "",
            "csv",
            0,
            false,
        )
        .unwrap();
        assert_eq!(out, "name,population\nOgdenville,250000\n");
    }

    #[test]
    fn aggregates_without_grouping() {
        let out = query(
            FC,
            "SELECT count(*), sum(population), avg(population), min(population), max(population)",
            "",
            "json",
            0,
            false,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["count(*)"], 4);
        assert_eq!(v[0]["sum(population)"], 400000.0);
        assert_eq!(v[0]["avg(population)"].as_f64().unwrap().round(), 133333.0);
        assert_eq!(v[0]["min(population)"], 30000.0);
        assert_eq!(v[0]["max(population)"], 250000.0);
    }

    #[test]
    fn group_by_with_alias_and_order() {
        let out = query(
            FC,
            "SELECT country, count(*) AS features, sum(population) AS people GROUP BY country ORDER BY people DESC",
            "",
            "json",
            0,
            false,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["country"], "CA");
        assert_eq!(v[0]["features"], 1);
        assert_eq!(v[0]["people"], 250000.0);
        assert_eq!(v[1]["country"], "US");
        assert_eq!(v[1]["features"], 3);
        assert_eq!(v[1]["people"], 150000.0);
    }

    #[test]
    fn group_by_to_csv() {
        let out = query(
            FC,
            "SELECT country, count(*) GROUP BY country ORDER BY country",
            "",
            "csv",
            0,
            false,
        )
        .unwrap();
        assert_eq!(out, "country,count(*)\nCA,1\nUS,3\n");
    }

    #[test]
    fn aggregate_over_empty_selection_is_zero() {
        let out = query(
            FC,
            "SELECT count(*) WHERE population > 999999999",
            "",
            "json",
            0,
            false,
        )
        .unwrap();
        assert_eq!(out, r#"[{"count(*)":0}]"#);
    }

    #[test]
    fn count_of_a_field_skips_missing() {
        let out = query(FC, "SELECT count(population) AS n", "", "json", 0, false).unwrap();
        assert_eq!(out, r#"[{"n":3}]"#);
    }

    #[test]
    fn indent_and_include_bbox() {
        let out = query(FC, "WHERE $id = 'b'", "", "geojson", 2, true).unwrap();
        assert!(out.contains("\n  \"type\": \"FeatureCollection\""));
        assert!(out.contains("\"bbox\": [\n    -88.79,\n    39.4,\n    -88.79,\n    39.4\n  ]"));
    }

    #[test]
    fn single_feature_and_bare_geometry_inputs() {
        let feature = r#"{"type":"Feature","properties":{"n":1},"geometry":{"type":"Point","coordinates":[1,2]}}"#;
        let out = query(feature, "SELECT * WHERE n = 1", "", "geojson", 0, false).unwrap();
        assert!(out.contains("\"coordinates\":[1,2]"));
        let geom = r#"{"type":"LineString","coordinates":[[0,0],[1,1]]}"#;
        let out = query(
            geom,
            "SELECT count(*) AS n WHERE $type = 'LineString'",
            "",
            "json",
            0,
            false,
        )
        .unwrap();
        assert_eq!(out, r#"[{"n":1}]"#);
    }

    #[test]
    fn from_clause_is_accepted() {
        let out = query(
            FC,
            "SELECT * FROM features WHERE country = 'CA'",
            "",
            "geojson",
            0,
            false,
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Ogdenville"]);
    }

    #[test]
    fn quoted_property_names_and_values_with_spaces() {
        let doc = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"my prop":"New York"},"geometry":null}]}"#;
        let out = query(
            doc,
            r#"SELECT * WHERE "my prop" = 'New York'"#,
            "",
            "json",
            0,
            false,
        )
        .unwrap();
        assert_eq!(out, r#"[{"my prop":"New York"}]"#);
    }

    #[test]
    fn numeric_strings_compare_numerically() {
        let doc = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"pop":"9"},"geometry":null},
          {"type":"Feature","properties":{"pop":"100"},"geometry":null}]}"#;
        let out = query(doc, "SELECT pop WHERE pop > 50", "", "json", 0, false).unwrap();
        assert_eq!(out, r#"[{"pop":"100"}]"#);
    }

    #[test]
    fn errors_are_actionable() {
        assert!(query("", "SELECT *", "", "geojson", 0, false)
            .unwrap_err()
            .contains("empty"));
        assert!(query(FC, "", "", "geojson", 0, false)
            .unwrap_err()
            .contains("empty"));
        assert!(query("not json", "SELECT *", "", "geojson", 0, false)
            .unwrap_err()
            .contains("parse error"));
        assert!(query(FC, "population > 5", "", "geojson", 0, false)
            .unwrap_err()
            .contains("must start with SELECT"));
        assert!(
            query(FC, "SELECT * WHERE population", "", "geojson", 0, false)
                .unwrap_err()
                .contains("incomplete")
        );
        assert!(
            query(FC, "SELECT * WHERE bbox(1, 2)", "", "geojson", 0, false)
                .unwrap_err()
                .contains("4 numbers")
        );
        assert!(
            query(FC, "SELECT * GROUP BY country", "", "geojson", 0, false)
                .unwrap_err()
                .contains("SELECT *")
        );
        assert!(query(
            FC,
            "SELECT name, count(*) GROUP BY country",
            "",
            "json",
            0,
            false
        )
        .unwrap_err()
        .contains("GROUP BY"));
        assert!(query(FC, "SELECT *", "1,2,3", "geojson", 0, false)
            .unwrap_err()
            .contains("4 numbers"));
        assert!(query(FC, "SELECT *", "", "xml", 0, false)
            .unwrap_err()
            .contains("unknown format"));
        assert!(query(FC, "SELECT *", "", "geojson", 9, false)
            .unwrap_err()
            .contains("indent"));
        assert!(query(FC, "SELECT median(population)", "", "json", 0, false)
            .unwrap_err()
            .contains("unknown function"));
        assert!(query(FC, "SELECT * WHERE name LIKE", "", "geojson", 0, false).is_err());
        assert!(query(FC, "SELECT * ORDER BY", "", "geojson", 0, false).is_err());
        assert!(query(FC, "SELECT * LIMIT two", "", "geojson", 0, false).is_err());
        assert!(query(
            FC,
            "SELECT * WHERE country = 'US' bogus",
            "",
            "geojson",
            0,
            false
        )
        .unwrap_err()
        .contains("unexpected"));
    }

    #[test]
    fn like_matcher_edge_cases() {
        assert!(like_match("abc", "a%"));
        assert!(like_match("abc", "%c"));
        assert!(like_match("abc", "a_c"));
        assert!(like_match("abc", "%"));
        assert!(!like_match("abc", "a_"));
        assert!(!like_match("abc", "b%"));
        assert!(like_match("aXbXc", "a%b%c"));
    }

    #[test]
    fn caps_are_enforced() {
        let big = format!("{}{}", " ".repeat(MAX_INPUT_BYTES), "{}");
        assert!(query(&big, "SELECT *", "", "geojson", 0, false)
            .unwrap_err()
            .contains("limit is 8 MB"));
    }
}
