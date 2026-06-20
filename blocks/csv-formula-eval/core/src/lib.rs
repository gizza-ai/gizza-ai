//! gizza-ai/csv-formula-eval core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Evaluates spreadsheet-style
//! arithmetic formulas to add or transform CSV columns, referencing other columns
//! by their (identifier-like) header name. Math via `meval`.


fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); }
        }
    })
}

fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "".to_string();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

struct Formula {
    target: String,
    expr: meval::Expr,
}

/// Parse `target = expr` formulas (separated by `;` or newlines).
fn parse_formulas(spec: &str) -> Result<Vec<Formula>, String> {
    let mut out = Vec::new();
    for part in spec.split([';', '\n']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (target, rhs) = part.split_once('=').ok_or_else(|| {
            format!("formula '{part}' must be '<new_column> = <expression>'")
        })?;
        let target = target.trim().to_string();
        if target.is_empty() {
            return Err(format!("formula '{part}' is missing a target column name"));
        }
        let expr: meval::Expr = rhs.trim().parse().map_err(|e| format!("invalid expression in '{part}': {e}"))?;
        out.push(Formula { target, expr });
    }
    if out.is_empty() {
        return Err("no formulas given (use '<new_column> = <expression>')".into());
    }
    Ok(out)
}

/// Apply formulas to a CSV with a header row. Columns are referenced in
/// expressions by their header name (must be a valid identifier — letters,
/// digits, `_`, not starting with a digit). A formula targeting an existing
/// column replaces it; a new target appends a column. Cells that aren't numbers
/// are treated as unavailable variables (an expression using them errors for that
/// row, yielding a blank cell).
pub fn eval(data: &str, formulas_spec: &str, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let formulas = parse_formulas(formulas_spec)?;
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_, _>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let mut header: Vec<String> = records[0].iter().map(|s| s.to_string()).collect();
    // Index of each target column (append if new) — fixed up once against header.
    let mut target_idx: Vec<usize> = Vec::with_capacity(formulas.len());
    for f in &formulas {
        match header.iter().position(|h| h == &f.target) {
            Some(i) => target_idx.push(i),
            None => { header.push(f.target.clone()); target_idx.push(header.len() - 1); }
        }
    }

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).flexible(true).from_writer(vec![]);
    wtr.write_record(&header).map_err(|e| format!("CSV write error: {e}"))?;

    for rec in records.iter().skip(1) {
        let mut row: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        row.resize(header.len(), String::new());
        for (f, &ti) in formulas.iter().zip(target_idx.iter()) {
            // Build the variable context from current numeric cells.
            let mut ctx = meval::Context::new();
            for (ci, name) in header.iter().enumerate() {
                if let Some(cell) = row.get(ci) {
                    if let Ok(n) = cell.trim().parse::<f64>() {
                        ctx.var(name.clone(), n);
                    }
                }
            }
            let val = f.expr.eval_with_context(&ctx);
            row[ti] = match val { Ok(n) => fmt_num(n), Err(_) => String::new() };
        }
        wtr.write_record(&row).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_computed_column() {
        let d = "price,qty\n10,3\n5,4";
        assert_eq!(eval(d, "total = price * qty", ",").unwrap(), "price,qty,total\n10,3,30\n5,4,20\n");
    }

    #[test]
    fn transforms_existing_column() {
        let d = "price\n10\n20";
        // replace price with price*1.1 (tax)
        assert_eq!(eval(d, "price = price * 1.1", ",").unwrap(), "price\n11\n22\n");
    }

    #[test]
    fn chained_formulas_see_earlier_results() {
        let d = "a,b\n2,3";
        // sum then double it
        assert_eq!(eval(d, "sum = a + b; dbl = sum * 2", ",").unwrap(), "a,b,sum,dbl\n2,3,5,10\n");
    }

    #[test]
    fn math_functions() {
        let d = "x\n9";
        assert_eq!(eval(d, "r = sqrt(x)", ",").unwrap(), "x,r\n9,3\n");
    }

    #[test]
    fn non_numeric_cell_yields_blank() {
        let d = "x\nfoo";
        // x isn't a number → expression can't bind x → blank result
        assert_eq!(eval(d, "y = x + 1", ",").unwrap(), "x,y\nfoo,\n");
    }

    #[test]
    fn errors() {
        assert!(eval("  ", "y = 1", ",").is_err());
        assert!(eval("a\n1", "no_equals", ",").is_err());
        assert!(eval("a\n1", "y = +*", ",").is_err());   // invalid expr
        assert!(eval("a\n1", "= 1", ",").is_err());       // missing target
    }
}
