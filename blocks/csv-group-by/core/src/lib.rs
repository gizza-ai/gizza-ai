//! gizza-ai/csv-group-by core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Groups CSV rows by one or more
//! columns and aggregates other columns (count/sum/avg/min/max). Groups are
//! emitted in first-seen order.

use std::collections::HashMap;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => { let b = other.as_bytes(); if b.len()==1 { b[0] } else { return Err(format!("delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'")); } }
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Func { Count, Sum, Avg, Min, Max }

impl Func {
    fn parse(s: &str) -> Result<Func, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "count" => Ok(Func::Count),
            "sum" => Ok(Func::Sum),
            "avg" | "mean" => Ok(Func::Avg),
            "min" => Ok(Func::Min),
            "max" => Ok(Func::Max),
            other => Err(format!("unknown aggregate '{other}' (use count/sum/avg/min/max)")),
        }
    }
    fn label(self) -> &'static str {
        match self { Func::Count => "count", Func::Sum => "sum", Func::Avg => "avg", Func::Min => "min", Func::Max => "max" }
    }
}

fn resolve(name: &str, header: &csv::StringRecord) -> Result<usize, String> {
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 { return Err("column index is 1-based (>= 1)".into()); }
        return Ok(n - 1);
    }
    header.iter().position(|c| c == name).ok_or_else(|| format!("column '{name}' not found in the header"))
}

struct Agg { col: usize, col_name: String, func: Func }

struct Acc { sum: f64, count: u64, min: f64, max: f64, n_numeric: u64 }
impl Acc {
    fn new() -> Self { Acc { sum: 0.0, count: 0, min: f64::INFINITY, max: f64::NEG_INFINITY, n_numeric: 0 } }
    fn push(&mut self, cell: &str) {
        self.count += 1;
        if let Ok(v) = cell.trim().parse::<f64>() {
            self.sum += v; self.min = self.min.min(v); self.max = self.max.max(v); self.n_numeric += 1;
        }
    }
    fn value(&self, f: Func) -> String {
        let fmt = |v: f64| { if v.fract()==0.0 && v.abs()<1e15 { format!("{}", v as i64) } else { let s=format!("{v:.6}"); s.trim_end_matches('0').trim_end_matches('.').to_string() } };
        match f {
            Func::Count => self.count.to_string(),
            Func::Sum => fmt(self.sum),
            Func::Avg => if self.n_numeric>0 { fmt(self.sum / self.n_numeric as f64) } else { String::new() },
            Func::Min => if self.n_numeric>0 { fmt(self.min) } else { String::new() },
            Func::Max => if self.n_numeric>0 { fmt(self.max) } else { String::new() },
        }
    }
}

/// Group `data` by `group_cols` (comma-separated names/indices) and aggregate per
/// `aggs` (comma/semicolon list of `col:func`, or bare `count`). Header required.
pub fn group_by(data: &str, group_cols: &str, aggs: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() { return Err("input is empty".into()); }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_,_>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() { return Err("no rows found".into()); }
    if !has_header { return Err("csv-group-by requires a header row (header=true)".into()); }
    let header = &records[0];

    let gcols: Vec<usize> = {
        let toks: Vec<&str> = group_cols.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if toks.is_empty() { return Err("group_by: at least one column is required".into()); }
        toks.iter().map(|t| resolve(t, header)).collect::<Result<_,_>>()?
    };
    let gnames: Vec<String> = gcols.iter().map(|&c| header.get(c).unwrap_or("").to_string()).collect();

    let mut aggspecs: Vec<Agg> = Vec::new();
    for tok in aggs.split([',', ';']).map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if tok.eq_ignore_ascii_case("count") {
            aggspecs.push(Agg { col: usize::MAX, col_name: String::new(), func: Func::Count });
            continue;
        }
        let (col, func) = tok.split_once(':').ok_or_else(|| format!("aggregate '{tok}' must be 'column:func' (or 'count')"))?;
        let ci = resolve(col.trim(), header)?;
        aggspecs.push(Agg { col: ci, col_name: header.get(ci).unwrap_or("").to_string(), func: Func::parse(func)? });
    }
    if aggspecs.is_empty() { return Err("at least one aggregation is required (e.g. 'amount:sum' or 'count')".into()); }

    // group key -> (order, group values, per-agg accumulators)
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, (Vec<String>, Vec<Acc>)> = HashMap::new();
    for rec in records.iter().skip(1) {
        let key = gcols.iter().map(|&c| rec.get(c).unwrap_or("")).collect::<Vec<_>>().join("\u{1}");
        let entry = groups.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            (gcols.iter().map(|&c| rec.get(c).unwrap_or("").to_string()).collect(), aggspecs.iter().map(|_| Acc::new()).collect())
        });
        for (i, a) in aggspecs.iter().enumerate() {
            let cell = if a.func == Func::Count { "" } else { rec.get(a.col).unwrap_or("") };
            entry.1[i].push(cell);
        }
    }

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    let mut out_header: Vec<String> = gnames.clone();
    for a in &aggspecs {
        out_header.push(if a.func == Func::Count && a.col == usize::MAX { "count".to_string() } else { format!("{}_{}", a.func.label(), a.col_name) });
    }
    wtr.write_record(&out_header).map_err(|e| format!("CSV write error: {e}"))?;
    for key in &order {
        let (gvals, accs) = &groups[key];
        let mut row = gvals.clone();
        for (i, a) in aggspecs.iter().enumerate() { row.push(accs[i].value(a.func)); }
        wtr.write_record(&row).map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_and_count_per_group() {
        let d = "dept,amount\nA,10\nB,5\nA,20\nB,15\nA,30";
        let out = group_by(d, "dept", "amount:sum, count", true, ",").unwrap();
        assert_eq!(out, "dept,sum_amount,count\nA,60,3\nB,20,2\n");
    }

    #[test]
    fn avg_min_max() {
        let d = "g,v\nx,2\nx,4\ny,10";
        let out = group_by(d, "g", "v:avg, v:min, v:max", true, ",").unwrap();
        assert_eq!(out, "g,avg_v,min_v,max_v\nx,3,2,4\ny,10,10,10\n");
    }

    #[test]
    fn multi_column_group() {
        let d = "a,b,v\nx,1,5\nx,1,5\nx,2,1";
        let out = group_by(d, "a,b", "v:sum", true, ",").unwrap();
        assert_eq!(out, "a,b,sum_v\nx,1,10\nx,2,1\n");
    }

    #[test]
    fn errors() {
        assert!(group_by("  ", "a", "count", true, ",").is_err());
        assert!(group_by("a,b\n1,2", "nope", "count", true, ",").is_err());
        assert!(group_by("a,b\n1,2", "a", "", true, ",").is_err());
        assert!(group_by("a,b\n1,2", "a", "b:bogus", true, ",").is_err());
        assert!(group_by("a,b\n1,2", "a", "count", false, ",").is_err());
    }
}
