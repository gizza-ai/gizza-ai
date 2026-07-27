//! gizza-ai/csv-window-functions core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps. Computes SQL-style window
//! functions over CSV rows WITHOUT collapsing them: a running total (cumulative
//! sum), a trailing moving average, lag/lead, and rank/dense_rank/row_number —
//! each evaluated within partitions and (optionally) an in-partition order. One
//! result column is appended to every input row.

use std::collections::HashMap;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

#[derive(Clone, Copy, PartialEq)]
enum WFunc {
    RunningTotal,
    MovingAverage,
    Lag,
    Lead,
    Rank,
    DenseRank,
    RowNumber,
}

impl WFunc {
    fn parse(s: &str) -> Result<WFunc, String> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "running_total" | "runningtotal" | "cumsum" | "cumulative_sum" => Ok(WFunc::RunningTotal),
            "moving_average" | "movingaverage" | "moving_avg" | "rolling_average" | "rolling_mean" => {
                Ok(WFunc::MovingAverage)
            }
            "lag" => Ok(WFunc::Lag),
            "lead" => Ok(WFunc::Lead),
            "rank" => Ok(WFunc::Rank),
            "dense_rank" | "denserank" => Ok(WFunc::DenseRank),
            "row_number" | "rownumber" | "rownum" => Ok(WFunc::RowNumber),
            other => Err(format!(
                "unknown function '{other}' (use running_total/moving_average/lag/lead/rank/dense_rank/row_number)"
            )),
        }
    }
    /// Whether this function reads a value `column`.
    fn needs_column(self) -> bool {
        !matches!(self, WFunc::RowNumber)
    }
    fn default_out(self, col: &str) -> String {
        match self {
            WFunc::RunningTotal => format!("running_total_{col}"),
            WFunc::MovingAverage => format!("moving_avg_{col}"),
            WFunc::Lag => format!("lag_{col}"),
            WFunc::Lead => format!("lead_{col}"),
            WFunc::Rank => "rank".to_string(),
            WFunc::DenseRank => "dense_rank".to_string(),
            WFunc::RowNumber => "row_number".to_string(),
        }
    }
}

fn resolve(name: &str, header: &csv::StringRecord) -> Result<usize, String> {
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("column index is 1-based (>= 1)".into());
        }
        return Ok(n - 1);
    }
    header
        .iter()
        .position(|c| c == name)
        .ok_or_else(|| format!("column '{name}' not found in the header"))
}

fn resolve_list(spec: &str, header: &csv::StringRecord) -> Result<Vec<usize>, String> {
    spec.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|t| resolve(t, header))
        .collect()
}

fn num(cell: &str) -> Option<f64> {
    let t = cell.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<f64>().ok()
    }
}

fn fmt(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// A total-order sort key for a cell: numbers sort before text, numerically /
/// lexically within their kind. Used for `order_by` sorting and rank ties.
#[derive(PartialEq)]
enum Key {
    Num(f64),
    Text(String),
}
impl Key {
    fn of(cell: &str) -> Key {
        match num(cell) {
            Some(n) => Key::Num(n),
            None => Key::Text(cell.to_string()),
        }
    }
    fn cmp(&self, other: &Key) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (self, other) {
            (Key::Num(a), Key::Num(b)) => a.partial_cmp(b).unwrap_or(Equal),
            (Key::Text(a), Key::Text(b)) => a.cmp(b),
            (Key::Num(_), Key::Text(_)) => Less,
            (Key::Text(_), Key::Num(_)) => Greater,
        }
    }
}

/// Sort a partition's row indices by `ocols` (stable; input order for ties).
fn order_partition(idx: &mut [usize], rows: &[&csv::StringRecord], ocols: &[usize], descending: bool) {
    if ocols.is_empty() {
        return;
    }
    idx.sort_by(|&a, &b| {
        for &c in ocols {
            let ka = Key::of(rows[a].get(c).unwrap_or(""));
            let kb = Key::of(rows[b].get(c).unwrap_or(""));
            let ord = ka.cmp(&kb);
            if ord != std::cmp::Ordering::Equal {
                return if descending { ord.reverse() } else { ord };
            }
        }
        std::cmp::Ordering::Equal
    });
}

/// Compute a window function over `data`, appending one result column to every row.
///
/// * `function` — running_total | moving_average | lag | lead | rank | dense_rank | row_number
/// * `column` — the value column (name or 1-based index); required for every function except row_number
/// * `partition_by` — comma-separated columns; each distinct combination is an independent window (empty = one window over all rows)
/// * `order_by` — comma-separated columns to sort rows within each partition before computing (empty = input order)
/// * `window` — trailing frame size for moving_average (rows, >= 1)
/// * `offset` — row distance for lag/lead (>= 0)
/// * `output_column` — name of the appended column (empty = a sensible default)
/// * `descending` — reverse the `order_by` sort, and rank largest-first
///
/// Output rows are grouped by partition (first-seen order of the partition key) and,
/// within each partition, follow the `order_by` order (or input order).
#[allow(clippy::too_many_arguments)]
pub fn window(
    data: &str,
    function: &str,
    column: &str,
    partition_by: &str,
    order_by: &str,
    window: i64,
    offset: i64,
    output_column: &str,
    descending: bool,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if !has_header {
        return Err("csv-window-functions requires a header row (header=true)".into());
    }
    let func = WFunc::parse(function)?;
    if window < 1 {
        return Err("window must be >= 1".into());
    }
    if offset < 0 {
        return Err("offset must be >= 0".into());
    }
    let win = window as usize;
    let off = offset as usize;

    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let header = &records[0];

    let col: Option<usize> = if func.needs_column() {
        if column.trim().is_empty() {
            return Err(format!(
                "function '{function}' needs a value column (set `column`)"
            ));
        }
        Some(resolve(column.trim(), header)?)
    } else {
        None
    };
    let pcols = resolve_list(partition_by, header)?;
    let ocols = resolve_list(order_by, header)?;

    let rows: Vec<&csv::StringRecord> = records.iter().skip(1).collect();

    // Group row indices by partition key, preserving first-seen partition order.
    let mut order: Vec<String> = Vec::new();
    let mut parts: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, rec) in rows.iter().enumerate() {
        let key = pcols
            .iter()
            .map(|&c| rec.get(c).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\u{1}");
        parts.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Vec::new()
        });
        parts.get_mut(&key).unwrap().push(i);
    }

    // Compute the appended value for every original row index.
    let mut out_val: Vec<String> = vec![String::new(); rows.len()];
    let out_name = if output_column.trim().is_empty() {
        func.default_out(column.trim())
    } else {
        output_column.trim().to_string()
    };

    for key in &order {
        let mut idx = parts[key].clone();
        order_partition(&mut idx, &rows, &ocols, descending);

        match func {
            WFunc::RunningTotal => {
                let c = col.unwrap();
                let mut acc = 0.0;
                for &r in &idx {
                    if let Some(v) = num(rows[r].get(c).unwrap_or("")) {
                        acc += v;
                    }
                    out_val[r] = fmt(acc);
                }
            }
            WFunc::MovingAverage => {
                let c = col.unwrap();
                for (pos, &r) in idx.iter().enumerate() {
                    let start = pos + 1 - win.min(pos + 1);
                    let mut sum = 0.0;
                    let mut n = 0u64;
                    for &rr in &idx[start..=pos] {
                        if let Some(v) = num(rows[rr].get(c).unwrap_or("")) {
                            sum += v;
                            n += 1;
                        }
                    }
                    out_val[r] = if n > 0 { fmt(sum / n as f64) } else { String::new() };
                }
            }
            WFunc::Lag => {
                let c = col.unwrap();
                for (pos, &r) in idx.iter().enumerate() {
                    out_val[r] = if pos >= off {
                        rows[idx[pos - off]].get(c).unwrap_or("").to_string()
                    } else {
                        String::new()
                    };
                }
            }
            WFunc::Lead => {
                let c = col.unwrap();
                for (pos, &r) in idx.iter().enumerate() {
                    out_val[r] = if pos + off < idx.len() {
                        rows[idx[pos + off]].get(c).unwrap_or("").to_string()
                    } else {
                        String::new()
                    };
                }
            }
            WFunc::Rank | WFunc::DenseRank => {
                let c = col.unwrap();
                // Rank by the value column within the partition (descending = largest first).
                let mut sorted: Vec<usize> = idx.clone();
                sorted.sort_by(|&a, &b| {
                    let ka = Key::of(rows[a].get(c).unwrap_or(""));
                    let kb = Key::of(rows[b].get(c).unwrap_or(""));
                    let ord = ka.cmp(&kb);
                    if descending {
                        ord.reverse()
                    } else {
                        ord
                    }
                });
                let mut rank = 0u64;
                let mut dense = 0u64;
                let mut prev: Option<Key> = None;
                let mut ranks: HashMap<usize, u64> = HashMap::new();
                for (i, &r) in sorted.iter().enumerate() {
                    let k = Key::of(rows[r].get(c).unwrap_or(""));
                    let tie = prev
                        .as_ref()
                        .map(|p| p.cmp(&k) == std::cmp::Ordering::Equal)
                        .unwrap_or(false);
                    if !tie {
                        rank = i as u64 + 1;
                        dense += 1;
                    }
                    ranks.insert(r, if func == WFunc::Rank { rank } else { dense });
                    prev = Some(k);
                }
                for &r in &idx {
                    out_val[r] = ranks[&r].to_string();
                }
            }
            WFunc::RowNumber => {
                for (pos, &r) in idx.iter().enumerate() {
                    out_val[r] = (pos + 1).to_string();
                }
            }
        }
    }

    // Emit: header + one appended column; rows grouped by partition then in-partition order.
    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    let mut out_header: Vec<String> = header.iter().map(|s| s.to_string()).collect();
    out_header.push(out_name);
    wtr.write_record(&out_header)
        .map_err(|e| format!("CSV write error: {e}"))?;
    for key in &order {
        let mut idx = parts[key].clone();
        order_partition(&mut idx, &rows, &ocols, descending);
        for &r in &idx {
            let mut row: Vec<String> = rows[r].iter().map(|s| s.to_string()).collect();
            row.push(out_val[r].clone());
            wtr.write_record(&row)
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
    }
    let bytes = wtr.into_inner().map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_total_over_all_rows() {
        let d = "day,sales\n1,10\n2,5\n3,20";
        let out = window(d, "running_total", "sales", "", "", 3, 1, "", false, true, ",").unwrap();
        assert_eq!(out, "day,sales,running_total_sales\n1,10,10\n2,5,15\n3,20,35\n");
    }

    #[test]
    fn running_total_partitioned_ordered() {
        let d = "region,day,sales\nW,2,5\nE,1,10\nW,1,3\nE,2,20";
        let out = window(d, "running_total", "sales", "region", "day", 3, 1, "", false, true, ",").unwrap();
        // W partition first-seen; within each, ordered by day, cumulative sales.
        assert_eq!(
            out,
            "region,day,sales,running_total_sales\nW,1,3,3\nW,2,5,8\nE,1,10,10\nE,2,20,30\n"
        );
    }

    #[test]
    fn moving_average_trailing_window() {
        let d = "v\n2\n4\n6\n8";
        let out = window(d, "moving_average", "v", "", "", 2, 1, "avg2", false, true, ",").unwrap();
        // trailing 2: [2]->2, [2,4]->3, [4,6]->5, [6,8]->7
        assert_eq!(out, "v,avg2\n2,2\n4,3\n6,5\n8,7\n");
    }

    #[test]
    fn lag_and_lead() {
        let d = "v\na\nb\nc";
        let lag = window(d, "lag", "v", "", "", 3, 1, "", false, true, ",").unwrap();
        assert_eq!(lag, "v,lag_v\na,\nb,a\nc,b\n");
        let lead = window(d, "lead", "v", "", "", 3, 1, "", false, true, ",").unwrap();
        assert_eq!(lead, "v,lead_v\na,b\nb,c\nc,\n");
    }

    #[test]
    fn rank_and_dense_rank_with_ties() {
        let d = "name,score\nA,90\nB,90\nC,80\nD,70";
        let r = window(d, "rank", "score", "", "", 3, 1, "", true, true, ",").unwrap();
        // desc: 90,90 -> rank 1,1 ; 80 -> 3 ; 70 -> 4 (gap after tie)
        assert_eq!(r, "name,score,rank\nA,90,1\nB,90,1\nC,80,3\nD,70,4\n");
        let dr = window(d, "dense_rank", "score", "", "", 3, 1, "", true, true, ",").unwrap();
        assert_eq!(dr, "name,score,dense_rank\nA,90,1\nB,90,1\nC,80,2\nD,70,3\n");
    }

    #[test]
    fn row_number_needs_no_column() {
        let d = "region,v\nW,x\nW,y\nE,z";
        let out = window(d, "row_number", "", "region", "", 3, 1, "", false, true, ",").unwrap();
        assert_eq!(out, "region,v,row_number\nW,x,1\nW,y,2\nE,z,1\n");
    }

    #[test]
    fn errors() {
        assert!(window("  ", "rank", "a", "", "", 3, 1, "", false, true, ",").is_err());
        assert!(window("a,b\n1,2", "bogus", "a", "", "", 3, 1, "", false, true, ",").is_err());
        assert!(window("a,b\n1,2", "rank", "nope", "", "", 3, 1, "", false, true, ",").is_err());
        assert!(window("a,b\n1,2", "running_total", "", "", "", 3, 1, "", false, true, ",").is_err());
        assert!(window("a,b\n1,2", "moving_average", "a", "", "", 0, 1, "", false, true, ",").is_err());
        assert!(window("a,b\n1,2", "rank", "a", "", "", 3, 1, "", false, false, ",").is_err());
    }
}
