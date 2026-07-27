//! critical-path-calculator core — pure Critical Path Method (CPM) scheduler.
//!
//! Given a task graph (each task has a duration and a list of predecessors),
//! runs a forward pass (earliest start/finish) and a backward pass (latest
//! start/finish), then derives total float, free float, the critical path, and
//! the total project duration. No wafer/wasm-bindgen deps — shared by the chat
//! skill block and the web page.

use std::collections::BTreeMap;

/// One parsed task: label, duration (already reduced from a PERT estimate if
/// one was given), and its immediate predecessors (in input order).
struct Task {
    name: String,
    duration: f64,
    preds: Vec<String>,
}

/// The computed schedule for one task.
pub struct TaskResult {
    pub name: String,
    pub duration: f64,
    pub earliest_start: f64,
    pub earliest_finish: f64,
    pub latest_start: f64,
    pub latest_finish: f64,
    pub total_float: f64,
    pub free_float: f64,
    pub critical: bool,
}

/// Full CPM result.
pub struct Schedule {
    pub project_duration: f64,
    pub critical_path: Vec<String>,
    pub tasks: Vec<TaskResult>,
}

// Floats that differ by less than this are treated as equal (guards against
// fractional-duration rounding when deciding what is "critical", i.e. zero
// total float).
const EPS: f64 = 1e-9;

/// Format an f64 without a trailing `.0` (so integer durations print cleanly)
/// while keeping fractional PERT results readable.
fn fmt_num(v: f64) -> String {
    let r = (v * 1e6).round() / 1e6;
    if (r - r.round()).abs() < EPS {
        format!("{}", r.round() as i64)
    } else {
        let s = format!("{r:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Parse a duration token: either a single non-negative number (`5`) or a PERT
/// three-point estimate `optimistic/most-likely/pessimistic` (`2/4/9`), which
/// reduces to the expected time (o + 4·m + p) / 6.
fn parse_duration(tok: &str) -> Result<f64, String> {
    let tok = tok.trim();
    if tok.is_empty() {
        return Err("missing duration".into());
    }
    if tok.contains('/') {
        let parts: Vec<&str> = tok.split('/').map(|p| p.trim()).collect();
        if parts.len() != 3 {
            return Err(format!(
                "PERT duration '{tok}' must be optimistic/most-likely/pessimistic (three values)"
            ));
        }
        let mut vals = [0.0f64; 3];
        for (i, p) in parts.iter().enumerate() {
            vals[i] = p
                .parse::<f64>()
                .map_err(|_| format!("duration '{tok}' has a non-numeric value '{p}'"))?;
            if vals[i] < 0.0 || !vals[i].is_finite() {
                return Err(format!("duration values must be finite and >= 0 (got '{p}')"));
            }
        }
        Ok((vals[0] + 4.0 * vals[1] + vals[2]) / 6.0)
    } else {
        let v = tok
            .parse::<f64>()
            .map_err(|_| format!("duration '{tok}' is not a number"))?;
        if v < 0.0 || !v.is_finite() {
            return Err(format!("duration must be finite and >= 0 (got '{tok}')"));
        }
        Ok(v)
    }
}

/// Parse the task list. One task per line:
/// `name, duration[, pred1, pred2, ...]`. Blank lines and `#` comments ignored.
fn parse(input: &str) -> Result<Vec<Task>, String> {
    let mut tasks: Vec<Task> = Vec::new();
    for (lineno, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|f| f.trim()).collect();
        let name = fields[0].trim();
        if name.is_empty() {
            return Err(format!("line {}: missing task name", lineno + 1));
        }
        if fields.len() < 2 || fields[1].is_empty() {
            return Err(format!(
                "line {}: task '{name}' needs a duration (e.g. `{name}, 5`)",
                lineno + 1
            ));
        }
        let duration = parse_duration(fields[1])
            .map_err(|e| format!("line {}: task '{name}': {e}", lineno + 1))?;
        // Remaining non-empty fields are predecessors (allows a trailing comma).
        let preds: Vec<String> = fields[2..]
            .iter()
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect();
        tasks.push(Task {
            name: name.to_string(),
            duration,
            preds,
        });
    }
    if tasks.is_empty() {
        return Err("no tasks provided — add at least one line `name, duration`".into());
    }
    // Unique names.
    let mut seen = BTreeMap::new();
    for t in &tasks {
        if seen.insert(t.name.clone(), ()).is_some() {
            return Err(format!("duplicate task name '{}'", t.name));
        }
    }
    // Every predecessor must be a defined task, and self-references are invalid.
    for t in &tasks {
        for p in &t.preds {
            if !seen.contains_key(p) {
                return Err(format!(
                    "task '{}' lists unknown predecessor '{p}'",
                    t.name
                ));
            }
            if p == &t.name {
                return Err(format!("task '{}' cannot depend on itself", t.name));
            }
        }
    }
    Ok(tasks)
}

/// Compute the full CPM schedule from a task list.
pub fn compute(input: &str) -> Result<Schedule, String> {
    let tasks = parse(input)?;
    let n = tasks.len();
    let index: BTreeMap<String, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.clone(), i))
        .collect();

    // Successor lists (adjacency) + in-degree for Kahn topological sort.
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    for (i, t) in tasks.iter().enumerate() {
        for p in &t.preds {
            let pi = index[p];
            succ[pi].push(i);
            indeg[i] += 1;
        }
    }

    // Kahn's algorithm → topological order (also detects cycles).
    let mut queue: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut topo: Vec<usize> = Vec::with_capacity(n);
    let mut deg = indeg.clone();
    let mut head = 0;
    while head < queue.len() {
        let u = queue[head];
        head += 1;
        topo.push(u);
        for &v in &succ[u] {
            deg[v] -= 1;
            if deg[v] == 0 {
                queue.push(v);
            }
        }
    }
    if topo.len() != n {
        let mut stuck: Vec<&str> = (0..n)
            .filter(|&i| deg[i] > 0)
            .map(|i| tasks[i].name.as_str())
            .collect();
        stuck.sort_unstable();
        return Err(format!(
            "dependencies form a cycle (cannot schedule); tasks involved: {}",
            stuck.join(", ")
        ));
    }

    // Forward pass: earliest start/finish.
    let mut es = vec![0.0f64; n];
    let mut ef = vec![0.0f64; n];
    for &u in &topo {
        let start = tasks[u]
            .preds
            .iter()
            .map(|p| ef[index[p]])
            .fold(0.0f64, f64::max);
        es[u] = start;
        ef[u] = start + tasks[u].duration;
    }
    let project_duration = ef.iter().cloned().fold(0.0f64, f64::max);

    // Backward pass: latest finish/start (reverse topological order).
    let mut lf = vec![project_duration; n];
    let mut ls = vec![0.0f64; n];
    for &u in topo.iter().rev() {
        let finish = if succ[u].is_empty() {
            project_duration
        } else {
            succ[u].iter().map(|&v| ls[v]).fold(f64::INFINITY, f64::min)
        };
        lf[u] = finish;
        ls[u] = finish - tasks[u].duration;
    }

    // Floats + criticality, in input order.
    let mut results = Vec::with_capacity(n);
    for (i, t) in tasks.iter().enumerate() {
        let total_float = ls[i] - es[i];
        // Free float = (min ES of successors) − EF; terminal tasks use the
        // project finish, which reduces free float to total float there.
        let free_float = if succ[i].is_empty() {
            project_duration - ef[i]
        } else {
            succ[i].iter().map(|&v| es[v]).fold(f64::INFINITY, f64::min) - ef[i]
        };
        results.push(TaskResult {
            name: t.name.clone(),
            duration: t.duration,
            earliest_start: es[i],
            earliest_finish: ef[i],
            latest_start: ls[i],
            latest_finish: lf[i],
            total_float: total_float.max(0.0),
            free_float: free_float.max(0.0),
            critical: total_float.abs() < EPS,
        });
    }

    // Build one representative critical path: walk from a critical start task
    // (ES = 0) forward through critical successors whose ES continues the chain.
    let critical: Vec<bool> = results.iter().map(|r| r.critical).collect();
    let mut path: Vec<String> = Vec::new();
    if let Some(start) = (0..n).find(|&i| critical[i] && es[i].abs() < EPS) {
        let mut cur = start;
        loop {
            path.push(tasks[cur].name.clone());
            let next = succ[cur].iter().cloned().find(|&v| {
                critical[v] && (es[v] - ef[cur]).abs() < EPS
            });
            match next {
                Some(v) => cur = v,
                None => break,
            }
        }
    }

    Ok(Schedule {
        project_duration,
        critical_path: path,
        tasks: results,
    })
}

/// Render the schedule as a human-readable aligned report or as JSON.
/// `format`: `"report"` (default / blank) or `"json"`.
pub fn analyze(input: &str, format: &str) -> Result<String, String> {
    let s = compute(input)?;
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "report" => Ok(render_report(&s)),
        "json" => Ok(render_json(&s)),
        other => Err(format!(
            "unknown format '{other}' — use 'report' or 'json'"
        )),
    }
}

fn render_report(s: &Schedule) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Project duration: {}\n",
        fmt_num(s.project_duration)
    ));
    if s.critical_path.is_empty() {
        out.push_str("Critical path: (none)\n");
    } else {
        out.push_str(&format!(
            "Critical path: {}\n",
            s.critical_path.join(" -> ")
        ));
    }
    out.push('\n');

    let headers = [
        "Task", "Dur", "ES", "EF", "LS", "LF", "Total float", "Free float", "Critical",
    ];
    // Each column: header + every cell.
    let mut cols: Vec<Vec<String>> = headers.iter().map(|h| vec![h.to_string()]).collect();
    for t in &s.tasks {
        let row = [
            t.name.clone(),
            fmt_num(t.duration),
            fmt_num(t.earliest_start),
            fmt_num(t.earliest_finish),
            fmt_num(t.latest_start),
            fmt_num(t.latest_finish),
            fmt_num(t.total_float),
            fmt_num(t.free_float),
            if t.critical { "yes".into() } else { "no".into() },
        ];
        for (c, cell) in row.into_iter().enumerate() {
            cols[c].push(cell);
        }
    }
    let widths: Vec<usize> = cols
        .iter()
        .map(|c| c.iter().map(|s| s.len()).max().unwrap_or(0))
        .collect();
    let nrows = s.tasks.len() + 1;
    for r in 0..nrows {
        let mut line = String::new();
        for c in 0..cols.len() {
            if c > 0 {
                line.push_str("  ");
            }
            line.push_str(&format!("{:<width$}", cols[c][r], width = widths[c]));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn render_json(s: &Schedule) -> String {
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"project_duration\": {},\n",
        fmt_num(s.project_duration)
    ));
    let path: Vec<String> = s
        .critical_path
        .iter()
        .map(|n| format!("\"{}\"", esc(n)))
        .collect();
    out.push_str(&format!(
        "  \"critical_path\": [{}],\n",
        path.join(", ")
    ));
    out.push_str("  \"tasks\": [\n");
    let items: Vec<String> = s
        .tasks
        .iter()
        .map(|t| {
            format!(
                "    {{\"name\": \"{}\", \"duration\": {}, \"earliest_start\": {}, \"earliest_finish\": {}, \"latest_start\": {}, \"latest_finish\": {}, \"total_float\": {}, \"free_float\": {}, \"critical\": {}}}",
                esc(&t.name),
                fmt_num(t.duration),
                fmt_num(t.earliest_start),
                fmt_num(t.earliest_finish),
                fmt_num(t.latest_start),
                fmt_num(t.latest_finish),
                fmt_num(t.total_float),
                fmt_num(t.free_float),
                t.critical
            )
        })
        .collect();
    out.push_str(&items.join(",\n"));
    out.push_str("\n  ]\n}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "A, 3\nB, 4, A\nC, 2, A\nD, 5, B, C\nE, 1, D";

    #[test]
    fn happy_path_report() {
        let out = analyze(SAMPLE, "report").unwrap();
        assert!(out.contains("Project duration: 13"), "got: {out}");
        assert!(out.contains("Critical path: A -> B -> D -> E"), "got: {out}");
        // C is the only non-critical task, with total float 2.
        assert!(out.lines().any(|l| l.starts_with("C ") && l.contains("no")));
    }

    #[test]
    fn json_output_is_exact() {
        let out = analyze("A, 3\nB, 4, A\nC, 2, A\nD, 5, B, C\nE, 1, D", "json").unwrap();
        let expected = "{\n  \"project_duration\": 13,\n  \"critical_path\": [\"A\", \"B\", \"D\", \"E\"],\n  \"tasks\": [\n    {\"name\": \"A\", \"duration\": 3, \"earliest_start\": 0, \"earliest_finish\": 3, \"latest_start\": 0, \"latest_finish\": 3, \"total_float\": 0, \"free_float\": 0, \"critical\": true},\n    {\"name\": \"B\", \"duration\": 4, \"earliest_start\": 3, \"earliest_finish\": 7, \"latest_start\": 3, \"latest_finish\": 7, \"total_float\": 0, \"free_float\": 0, \"critical\": true},\n    {\"name\": \"C\", \"duration\": 2, \"earliest_start\": 3, \"earliest_finish\": 5, \"latest_start\": 5, \"latest_finish\": 7, \"total_float\": 2, \"free_float\": 2, \"critical\": false},\n    {\"name\": \"D\", \"duration\": 5, \"earliest_start\": 7, \"earliest_finish\": 12, \"latest_start\": 7, \"latest_finish\": 12, \"total_float\": 0, \"free_float\": 0, \"critical\": true},\n    {\"name\": \"E\", \"duration\": 1, \"earliest_start\": 12, \"earliest_finish\": 13, \"latest_start\": 12, \"latest_finish\": 13, \"total_float\": 0, \"free_float\": 0, \"critical\": true}\n  ]\n}";
        assert_eq!(out, expected);
    }

    #[test]
    fn pert_three_point_estimate() {
        // Single task, PERT estimate (2 + 4*4 + 9)/6 = 4.5.
        let out = analyze("A, 2/4/9", "json").unwrap();
        assert!(out.contains("\"project_duration\": 4.5"), "got: {out}");
        assert!(out.contains("\"duration\": 4.5"), "got: {out}");
    }

    #[test]
    fn parallel_paths_pick_the_longest() {
        // Two parallel chains from A to D; the B chain (6) is longer than C (2).
        let s = compute("A, 1\nB, 6, A\nC, 2, A\nD, 1, B, C").unwrap();
        assert_eq!(s.project_duration, 8.0);
        assert_eq!(s.critical_path, vec!["A", "B", "D"]);
    }

    #[test]
    fn err_on_cycle() {
        let e = analyze("A, 1, C\nB, 1, A\nC, 1, B", "report").unwrap_err();
        assert!(e.contains("cycle"), "got: {e}");
    }

    #[test]
    fn err_on_unknown_predecessor() {
        let e = analyze("A, 1, Z", "report").unwrap_err();
        assert!(e.contains("unknown predecessor 'Z'"), "got: {e}");
    }

    #[test]
    fn err_on_missing_duration() {
        let e = analyze("A", "report").unwrap_err();
        assert!(e.contains("needs a duration"), "got: {e}");
    }

    #[test]
    fn err_on_empty() {
        assert!(analyze("   \n # comment", "report").is_err());
    }

    #[test]
    fn err_on_bad_format() {
        assert!(analyze("A, 1", "xml").unwrap_err().contains("unknown format"));
    }
}
