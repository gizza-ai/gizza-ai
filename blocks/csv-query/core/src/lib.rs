//! gizza-ai/csv-query core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Runs a small SQL-ish query over CSV:
//!   SELECT <cols|*> [WHERE <col op value>] [ORDER BY <col> [ASC|DESC]] [LIMIT n]
//! Columns are header names or 1-based indices. Aggregates / GROUP BY are out of
//! scope here (use csv-group-by / csv-pivot). Numeric-aware comparisons.

#[derive(Clone, Copy, PartialEq)]
enum Op { Eq, Ne, Lt, Le, Gt, Ge, Contains }

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => { let b = other.as_bytes(); if b.len()==1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); } }
    })
}

fn resolve(name: &str, header: &csv::StringRecord) -> Result<usize, String> {
    if let Ok(n) = name.parse::<usize>() { if n==0 { return Err("column index is 1-based (>= 1)".into()); } return Ok(n-1); }
    header.iter().position(|c| c == name).ok_or_else(|| format!("column '{name}' not found in the header"))
}

/// Case-insensitive search for a top-level keyword (surrounded by spaces) and
/// return the byte index of the keyword start.
fn find_kw(q: &str, kw: &str) -> Option<usize> {
    let lq = q.to_ascii_lowercase();
    let needle = format!(" {} ", kw.to_ascii_lowercase());
    lq.find(&needle).map(|i| i + 1)
}

fn parse_condition(c: &str) -> Result<(String, Op, String), String> {
    let c = c.trim();
    if let Some(i) = c.to_ascii_lowercase().find(" contains ") {
        return Ok((c[..i].trim().to_string(), Op::Contains, c[i+10..].trim().to_string()));
    }
    let bytes = c.as_bytes();
    for i in 0..bytes.len() {
        let two = if i+2<=bytes.len() { &c[i..i+2] } else { "" };
        let op = match two { "=="=>Some((Op::Eq,2)), "!="=>Some((Op::Ne,2)), "<="=>Some((Op::Le,2)), ">="=>Some((Op::Ge,2)),
            _ => match &c[i..i+1] { ">"=>Some((Op::Gt,1)), "<"=>Some((Op::Lt,1)), "="=>Some((Op::Eq,1)), _=>None } };
        if let Some((op,len)) = op {
            let col = c[..i].trim().to_string();
            if col.is_empty() { return Err("WHERE condition is missing a column".into()); }
            return Ok((col, op, c[i+len..].trim().to_string()));
        }
    }
    Err("WHERE must be '<column> <op> <value>' (op: == != < <= > >= contains)".into())
}

fn matches(cell: &str, op: Op, value: &str) -> bool {
    if op == Op::Contains { return cell.to_lowercase().contains(&value.to_lowercase()); }
    match (cell.trim().parse::<f64>(), value.trim().parse::<f64>()) {
        (Ok(a), Ok(b)) => match op { Op::Eq=>a==b, Op::Ne=>a!=b, Op::Lt=>a<b, Op::Le=>a<=b, Op::Gt=>a>b, Op::Ge=>a>=b, Op::Contains=>unreachable!() },
        _ => match op { Op::Eq=>cell==value, Op::Ne=>cell!=value, Op::Lt=>cell<value, Op::Le=>cell<=value, Op::Gt=>cell>value, Op::Ge=>cell>=value, Op::Contains=>unreachable!() },
    }
}

/// Run the query over `data`. Requires a header row.
pub fn query(data: &str, q: &str, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() { return Err("input is empty".into()); }
    if q.trim().is_empty() { return Err("query is empty".into()); }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_,_>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() { return Err("no rows found".into()); }
    let header = records[0].clone();

    // Carve clauses by keyword position: SELECT < .. > [WHERE ..] [ORDER BY ..] [LIMIT ..]
    let qpad = format!(" {} ", q.trim());
    let lq = qpad.to_ascii_lowercase();
    if !lq.trim_start().starts_with("select") { return Err("query must start with SELECT".into()); }
    let sel_start = lq.find("select").unwrap() + "select".len();
    let where_i = find_kw(&qpad, "where");
    let order_i = find_kw(&qpad, "order by");
    let limit_i = find_kw(&qpad, "limit");
    let bound = |after: usize| [where_i, order_i, limit_i].into_iter().flatten().filter(|&x| x > after).min().unwrap_or(qpad.len());

    let select_clause = qpad[sel_start..bound(sel_start)].trim().to_string();
    let where_clause = where_i.map(|i| qpad[i+5..bound(i)].trim().to_string());
    let order_clause = order_i.map(|i| qpad[i+8..bound(i)].trim().to_string());
    let limit_clause = limit_i.map(|i| qpad[i+5..bound(i)].trim().to_string());

    // SELECT columns.
    let proj: Option<Vec<usize>> = if select_clause == "*" { None } else {
        let cols: Vec<&str> = select_clause.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if cols.is_empty() { return Err("SELECT needs columns or '*'".into()); }
        Some(cols.iter().map(|c| resolve(c, &header)).collect::<Result<_,_>>()?)
    };

    // WHERE.
    let cond = match where_clause { Some(c) => { let (col, op, val) = parse_condition(&c)?; Some((resolve(&col, &header)?, op, val)) }, None => None };

    // Filter rows.
    let mut rows: Vec<csv::StringRecord> = records.iter().skip(1).filter(|rec| match &cond {
        Some((ci, op, val)) => matches(rec.get(*ci).unwrap_or(""), *op, val),
        None => true,
    }).cloned().collect();

    // ORDER BY.
    if let Some(oc) = order_clause {
        let mut parts = oc.split_whitespace();
        let col = parts.next().ok_or("ORDER BY needs a column")?;
        let desc = matches!(parts.next().map(|s| s.to_ascii_lowercase()).as_deref(), Some("desc"));
        let ci = resolve(col, &header)?;
        rows.sort_by(|a, b| {
            let (x, y) = (a.get(ci).unwrap_or(""), b.get(ci).unwrap_or(""));
            let ord = match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
                (Ok(p), Ok(qn)) => p.partial_cmp(&qn).unwrap_or(std::cmp::Ordering::Equal),
                _ => x.cmp(y),
            };
            if desc { ord.reverse() } else { ord }
        });
    }

    // LIMIT.
    if let Some(lc) = limit_clause {
        let n: usize = lc.trim().parse().map_err(|_| format!("LIMIT must be a number, got '{lc}'"))?;
        rows.truncate(n);
    }

    // Emit.
    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    let project = |rec: &csv::StringRecord| -> Vec<String> {
        match &proj { Some(cols) => cols.iter().map(|&c| rec.get(c).unwrap_or("").to_string()).collect(), None => rec.iter().map(|s| s.to_string()).collect() }
    };
    wtr.write_record(&project(&header)).map_err(|e| format!("CSV write error: {e}"))?;
    for rec in &rows { wtr.write_record(&project(rec)).map_err(|e| format!("CSV write error: {e}"))?; }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: &str = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCarol,40,NYC\nDan,35,SF";

    #[test]
    fn select_star() {
        assert_eq!(query(D, "SELECT *", ",").unwrap(), format!("{D}\n"));
    }

    #[test]
    fn project_columns() {
        assert_eq!(query(D, "SELECT name, city", ",").unwrap(), "name,city\nAlice,NYC\nBob,LA\nCarol,NYC\nDan,SF\n");
    }

    #[test]
    fn where_numeric() {
        assert_eq!(query(D, "SELECT name WHERE age >= 35", ",").unwrap(), "name\nCarol\nDan\n");
    }

    #[test]
    fn where_contains_and_order_and_limit() {
        let out = query(D, "SELECT name, age WHERE city == NYC ORDER BY age DESC LIMIT 1", ",").unwrap();
        assert_eq!(out, "name,age\nCarol,40\n");
    }

    #[test]
    fn order_by_string_asc() {
        let out = query(D, "SELECT name ORDER BY name", ",").unwrap();
        assert_eq!(out, "name\nAlice\nBob\nCarol\nDan\n");
    }

    #[test]
    fn errors() {
        assert!(query("  ", "SELECT *", ",").is_err());
        assert!(query(D, "", ",").is_err());
        assert!(query(D, "name", ",").is_err());              // no SELECT
        assert!(query(D, "SELECT nope", ",").is_err());        // unknown column
        assert!(query(D, "SELECT * WHERE age", ",").is_err()); // bad condition
        assert!(query(D, "SELECT * LIMIT x", ",").is_err());   // bad limit
    }
}
