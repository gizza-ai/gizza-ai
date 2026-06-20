//! gizza-ai/csv-pivot core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Builds a pivot table: rows are grouped
//! by one or more `rows` columns, the distinct values of a `columns` field become
//! output columns, and each cell aggregates a `values` column (sum/count/avg/min/
//! max). Row keys and pivot columns are emitted in first-seen order.

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
            "" | "sum" => Ok(Func::Sum), "count" => Ok(Func::Count),
            "avg" | "mean" => Ok(Func::Avg), "min" => Ok(Func::Min), "max" => Ok(Func::Max),
            other => Err(format!("unknown aggregate '{other}' (use sum/count/avg/min/max)")),
        }
    }
}

fn resolve(name: &str, header: &csv::StringRecord) -> Result<usize, String> {
    if let Ok(n) = name.parse::<usize>() { if n==0 { return Err("column index is 1-based (>= 1)".into()); } return Ok(n-1); }
    header.iter().position(|c| c == name).ok_or_else(|| format!("column '{name}' not found in the header"))
}

struct Acc { sum: f64, count: u64, min: f64, max: f64, n: u64 }
impl Acc {
    fn new() -> Self { Acc { sum:0.0, count:0, min:f64::INFINITY, max:f64::NEG_INFINITY, n:0 } }
    fn push(&mut self, cell: &str) { self.count+=1; if let Ok(v)=cell.trim().parse::<f64>(){ self.sum+=v; self.min=self.min.min(v); self.max=self.max.max(v); self.n+=1; } }
    fn value(&self, f: Func) -> String {
        let fmt=|v:f64|{ if v.fract()==0.0 && v.abs()<1e15 { format!("{}", v as i64) } else { let s=format!("{v:.6}"); s.trim_end_matches('0').trim_end_matches('.').to_string() } };
        match f { Func::Count=>self.count.to_string(), Func::Sum=>fmt(self.sum),
            Func::Avg=> if self.n>0 {fmt(self.sum/self.n as f64)} else {String::new()},
            Func::Min=> if self.n>0 {fmt(self.min)} else {String::new()},
            Func::Max=> if self.n>0 {fmt(self.max)} else {String::new()} }
    }
}

/// Build the pivot table. See module docs.
pub fn pivot(data: &str, rows: &str, columns: &str, values: &str, agg: &str, has_header: bool, delimiter: &str) -> Result<String, String> {
    if data.trim().is_empty() { return Err("input is empty".into()); }
    if !has_header { return Err("csv-pivot requires a header row (header=true)".into()); }
    let func = Func::parse(agg)?;
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr.records().collect::<Result<_,_>>().map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() { return Err("no rows found".into()); }
    let header = &records[0];

    let rcols: Vec<usize> = { let t: Vec<&str> = rows.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if t.is_empty() { return Err("rows: at least one column is required".into()); }
        t.iter().map(|x| resolve(x, header)).collect::<Result<_,_>>()? };
    let ccol = resolve(columns.trim(), header)?;
    let vcol = resolve(values.trim(), header)?;
    let rnames: Vec<String> = rcols.iter().map(|&c| header.get(c).unwrap_or("").to_string()).collect();

    let mut row_order: Vec<String> = Vec::new();
    let mut col_order: Vec<String> = Vec::new();
    let mut row_vals: HashMap<String, Vec<String>> = HashMap::new();
    let mut cells: HashMap<(String, String), Acc> = HashMap::new();

    for rec in records.iter().skip(1) {
        let rkey = rcols.iter().map(|&c| rec.get(c).unwrap_or("")).collect::<Vec<_>>().join("\u{1}");
        if !row_vals.contains_key(&rkey) {
            row_order.push(rkey.clone());
            row_vals.insert(rkey.clone(), rcols.iter().map(|&c| rec.get(c).unwrap_or("").to_string()).collect());
        }
        let cval = rec.get(ccol).unwrap_or("").to_string();
        if !col_order.iter().any(|c| c == &cval) { col_order.push(cval.clone()); }
        cells.entry((rkey, cval)).or_insert_with(Acc::new).push(rec.get(vcol).unwrap_or(""));
    }
    if row_order.is_empty() { return Err("no data rows to pivot".into()); }

    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    let mut out_header = rnames.clone();
    out_header.extend(col_order.iter().cloned());
    wtr.write_record(&out_header).map_err(|e| format!("CSV write error: {e}"))?;
    for rkey in &row_order {
        let mut row = row_vals[rkey].clone();
        for cval in &col_order {
            row.push(match cells.get(&(rkey.clone(), cval.clone())) { Some(a) => a.value(func), None => String::new() });
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
    fn basic_sum_pivot() {
        // rows=region, columns=product, values=sales, sum
        let d = "region,product,sales\nN,A,10\nN,B,5\nS,A,7\nN,A,3";
        let out = pivot(d, "region", "product", "sales", "sum", true, ",").unwrap();
        assert_eq!(out, "region,A,B\nN,13,5\nS,7,\n");
    }

    #[test]
    fn count_pivot() {
        let d = "r,c,v\nx,p,1\nx,p,2\nx,q,9";
        let out = pivot(d, "r", "c", "v", "count", true, ",").unwrap();
        assert_eq!(out, "r,p,q\nx,2,1\n");
    }

    #[test]
    fn errors() {
        assert!(pivot("  ", "r","c","v","sum", true, ",").is_err());
        assert!(pivot("a,b,c\n1,2,3", "nope","b","c","sum", true, ",").is_err());
        assert!(pivot("a,b,c\n1,2,3", "a","b","c","bogus", true, ",").is_err());
        assert!(pivot("a,b,c\n1,2,3", "a","b","c","sum", false, ",").is_err());
    }
}
