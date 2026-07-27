//! data-clusterer core — cluster the numeric columns of a tabular (CSV) dataset
//! with KMeans, DBSCAN, or hierarchical (agglomerative) clustering, then render
//! the result as a self-contained SVG scatter plot, a labeled CSV, or a JSON
//! report. Pure-Rust (hand-built SVG, hand-rolled algorithms + PCA, no plotting
//! or ML deps), so it runs on every backend including the chat Service Worker.
//! No wafer/wasm-bindgen deps here.
//!
//! Determinism: KMeans uses a deterministic farthest-point (k-means++ style, no
//! RNG) seeding and PCA uses deterministic power iteration, so the same input
//! always yields the same clusters/plot — required by the page's
//! recompute-on-input model and by the tests.

/// Palette for cluster colours (DBSCAN noise is drawn separately in grey).
const PALETTE: [&str; 10] = [
    "#4e79a7", "#f28e2b", "#59a14f", "#e15759", "#76b7b2", "#edc948", "#b07aa1", "#ff9da7",
    "#9c755f", "#bab0ac",
];
const NOISE_COLOR: &str = "#b0b0b0";

/// Caps to keep runtime + memory inside the 64 MiB wasm sandbox.
const MAX_ROWS: usize = 50_000;
const MAX_POINTS: usize = 10_000; // kmeans / dbscan
const MAX_HIER: usize = 1_500; // hierarchical is O(n^2) memory
const MAX_FEATURES: usize = 50;
const MAX_SILHOUETTE_N: usize = 4_000; // silhouette is O(n^2); skip above this
const KMEANS_MAX_ITER: usize = 100;
const PCA_ITERS: usize = 200;

/// Options resolved from the tool params.
pub struct Options {
    /// One of "kmeans" | "dbscan" | "hierarchical".
    pub method: String,
    /// Number of clusters k (KMeans) or target cluster count (hierarchical).
    /// Ignored for DBSCAN, which discovers the count from `eps`/`min_samples`.
    pub clusters: u32,
    /// DBSCAN neighbourhood radius (in the clustering feature space — standardized
    /// units when `normalize` is on). Ignored for the other methods.
    pub eps: f64,
    /// DBSCAN minimum neighbourhood size (including the point itself) for a core
    /// point. Ignored for the other methods.
    pub min_samples: u32,
    /// Hierarchical linkage: "average" | "complete" | "single" | "ward".
    pub linkage: String,
    /// Comma-separated feature columns: header names or 1-based indices. Blank =
    /// use every fully-numeric column.
    pub columns: String,
    /// Standardize each feature to zero mean / unit variance before clustering so
    /// columns on different scales contribute comparably.
    pub normalize: bool,
    /// Output: "chart" (SVG scatter plot), "csv" (rows + a cluster column), or
    /// "json" (clusters, sizes, centroids, silhouette).
    pub output: String,
    /// Optional chart title.
    pub title: String,
    /// SVG width in pixels.
    pub width: u32,
    /// SVG height in pixels.
    pub height: u32,
}

/// A clustered result, produced once and then rendered to the chosen format.
struct Clustered {
    /// Header names for the selected feature columns (synthesised as "Column N"
    /// when the input has no header row).
    feature_names: Vec<String>,
    /// One entry per data row (in input order): the raw string cells.
    rows: Vec<Vec<String>>,
    /// For each data row, the index into `points`/`labels` if it was clustered,
    /// or None if it was skipped (non-numeric / blank in a selected column).
    row_point: Vec<Option<usize>>,
    /// The clustered points, in the (possibly standardized) space used for
    /// distances — parallel to `labels`.
    points: Vec<Vec<f64>>,
    /// The clustered points in ORIGINAL units (for centroids + the plot).
    points_raw: Vec<Vec<f64>>,
    /// Cluster label per point: 0..k for a cluster, -1 for DBSCAN noise.
    labels: Vec<i32>,
    /// Distinct non-noise cluster ids in ascending order.
    cluster_ids: Vec<i32>,
    method: String,
}

// ---- CSV parsing -----------------------------------------------------------

/// Parse CSV text into rows of trimmed cells. Quoted fields with embedded
/// commas/newlines and doubled-quote escapes are handled by the `csv` crate.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records().flatten() {
        rows.push(rec.iter().map(|c| c.trim().to_string()).collect());
        if rows.len() > MAX_ROWS {
            break;
        }
    }
    rows.retain(|r| !r.iter().all(|c| c.is_empty()));
    rows
}

/// A row is a header if it is non-empty and not entirely numeric.
fn is_header(row: &[String]) -> bool {
    !row.is_empty() && !row.iter().all(|c| c.trim().parse::<f64>().is_ok())
}

/// Resolve a column spec (header name, case-insensitive, or 1-based index) to a
/// 0-based column index.
fn resolve_col(spec: &str, header: Option<&Vec<String>>, ncols: usize) -> Result<usize, String> {
    let spec = spec.trim();
    if let Some(h) = header {
        if let Some(i) = h.iter().position(|c| c.eq_ignore_ascii_case(spec)) {
            return Ok(i);
        }
    }
    if let Ok(n) = spec.parse::<usize>() {
        if n >= 1 && n <= ncols {
            return Ok(n - 1);
        }
        return Err(format!(
            "column '{spec}' is out of range (the table has {ncols} columns)"
        ));
    }
    match header {
        Some(h) => Err(format!(
            "column '{spec}' not found — available: {}",
            h.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(", ")
        )),
        None => Err(format!(
            "column '{spec}' not found — the table has no header row, so use a 1-based index (1..{ncols})"
        )),
    }
}

// ---- number / string formatting -------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Format a number: integer if whole, else up to 3 decimals (trailing zeros
/// trimmed).
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if (v.round() - v).abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Quote a CSV field if it contains a comma, quote, or newline.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn euclidean(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

fn sq_euclidean(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>()
}

// ---- feature extraction ----------------------------------------------------

/// Build the clustered dataset: parse CSV, pick feature columns, extract the
/// numeric matrix, standardize if requested, then run the chosen algorithm.
fn cluster(data: &str, opts: &Options) -> Result<Clustered, String> {
    let all = parse_csv(data);
    if all.is_empty() {
        return Err("no data — paste a CSV table with numeric columns".into());
    }
    let ncols = all.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncols == 0 {
        return Err("no columns found in the data".into());
    }

    // Header detection: a non-numeric first row (with >=2 rows) is a header.
    let has_header = all.len() >= 2 && is_header(&all[0]);
    let header: Option<&Vec<String>> = if has_header { Some(&all[0]) } else { None };
    let body: &[Vec<String>] = if has_header { &all[1..] } else { &all[..] };
    if body.is_empty() {
        return Err("the data has a header but no rows to cluster".into());
    }

    // Select feature columns.
    let cols: Vec<usize> = if opts.columns.trim().is_empty() {
        // Every column whose body cells are all non-empty and numeric.
        (0..ncols)
            .filter(|&c| {
                body.iter().all(|r| {
                    r.get(c).map(|v| !v.is_empty() && v.parse::<f64>().is_ok()).unwrap_or(false)
                })
            })
            .collect()
    } else {
        let mut out = Vec::new();
        for spec in opts.columns.split(',').filter(|s| !s.trim().is_empty()) {
            let idx = resolve_col(spec, header, ncols)?;
            if !out.contains(&idx) {
                out.push(idx);
            }
        }
        out
    };
    if cols.is_empty() {
        return Err(
            "no numeric feature columns found — provide a CSV with numeric columns, or name them in `columns`".into(),
        );
    }
    if cols.len() > MAX_FEATURES {
        return Err(format!(
            "too many feature columns ({}) — cap is {MAX_FEATURES}; list the ones to cluster in `columns`",
            cols.len()
        ));
    }

    let feature_names: Vec<String> = cols
        .iter()
        .map(|&c| match header {
            Some(h) => h.get(c).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("Column {}", c + 1)),
            None => format!("Column {}", c + 1),
        })
        .collect();

    // Extract the numeric matrix (rows with any non-numeric selected cell are
    // skipped from clustering but retained for the CSV output).
    let rows: Vec<Vec<String>> = body.to_vec();
    let mut row_point: Vec<Option<usize>> = Vec::with_capacity(rows.len());
    let mut points_raw: Vec<Vec<f64>> = Vec::new();
    for r in &rows {
        let mut vals = Vec::with_capacity(cols.len());
        let mut ok = true;
        for &c in &cols {
            match r.get(c).and_then(|v| v.parse::<f64>().ok()) {
                Some(v) if v.is_finite() => vals.push(v),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            row_point.push(Some(points_raw.len()));
            points_raw.push(vals);
        } else {
            row_point.push(None);
        }
    }

    let n = points_raw.len();
    if n < 2 {
        return Err(format!(
            "only {n} fully-numeric row(s) found — need at least 2 to cluster"
        ));
    }

    // Standardize (z-score) per feature for the distance space.
    let dims = cols.len();
    let mut points: Vec<Vec<f64>> = points_raw.clone();
    if opts.normalize {
        for d in 0..dims {
            let mean = points_raw.iter().map(|p| p[d]).sum::<f64>() / n as f64;
            let var = points_raw.iter().map(|p| (p[d] - mean).powi(2)).sum::<f64>() / n as f64;
            let sd = var.sqrt();
            if sd > 1e-12 {
                for p in &mut points {
                    p[d] = (p[d] - mean) / sd;
                }
            } else {
                for p in &mut points {
                    p[d] = 0.0;
                }
            }
        }
    }

    let method = opts.method.trim().to_lowercase();
    let labels = match method.as_str() {
        "kmeans" | "k-means" | "" => {
            if n > MAX_POINTS {
                return Err(format!("{n} rows exceeds the KMeans cap of {MAX_POINTS}"));
            }
            let k = opts.clusters.max(1) as usize;
            if k > n {
                return Err(format!(
                    "k={k} is larger than the {n} data points — reduce the cluster count"
                ));
            }
            kmeans(&points, k)
        }
        "dbscan" => {
            if n > MAX_POINTS {
                return Err(format!("{n} rows exceeds the DBSCAN cap of {MAX_POINTS}"));
            }
            if !(opts.eps > 0.0 && opts.eps.is_finite()) {
                return Err("dbscan needs eps > 0 (the neighbourhood radius)".into());
            }
            dbscan(&points, opts.eps, opts.min_samples.max(1) as usize)
        }
        "hierarchical" | "agglomerative" => {
            if n > MAX_HIER {
                return Err(format!(
                    "{n} rows exceeds the hierarchical cap of {MAX_HIER} (it needs an n×n distance matrix) — use KMeans/DBSCAN for larger data"
                ));
            }
            let k = opts.clusters.max(1) as usize;
            if k > n {
                return Err(format!(
                    "target cluster count {k} is larger than the {n} data points"
                ));
            }
            let link = parse_linkage(&opts.linkage)?;
            hierarchical(&points, k, link)
        }
        other => {
            return Err(format!(
                "unknown method '{other}' — use kmeans, dbscan, or hierarchical"
            ))
        }
    };

    let mut cluster_ids: Vec<i32> = labels.iter().copied().filter(|&l| l >= 0).collect();
    cluster_ids.sort_unstable();
    cluster_ids.dedup();

    Ok(Clustered {
        feature_names,
        rows,
        row_point,
        points,
        points_raw,
        labels,
        cluster_ids,
        method,
    })
}

// ---- KMeans (deterministic) -----------------------------------------------

/// Lloyd's algorithm with deterministic farthest-point (k-means++ style, no RNG)
/// seeding: the first centre is point 0, each next centre is the point with the
/// greatest distance to its nearest chosen centre (ties → lowest index).
fn kmeans(points: &[Vec<f64>], k: usize) -> Vec<i32> {
    let n = points.len();
    let dims = points[0].len();

    // Seed centres.
    let mut centres: Vec<Vec<f64>> = Vec::with_capacity(k);
    centres.push(points[0].clone());
    while centres.len() < k {
        let mut best_i = 0usize;
        let mut best_d = -1.0f64;
        for (i, p) in points.iter().enumerate() {
            let nearest = centres
                .iter()
                .map(|c| euclidean(p, c))
                .fold(f64::INFINITY, f64::min);
            if nearest > best_d {
                best_d = nearest;
                best_i = i;
            }
        }
        centres.push(points[best_i].clone());
    }

    let mut labels = vec![0i32; n];
    for _ in 0..KMEANS_MAX_ITER {
        // Assign.
        let mut changed = false;
        for (i, p) in points.iter().enumerate() {
            let mut best = 0usize;
            let mut best_d = f64::INFINITY;
            for (ci, c) in centres.iter().enumerate() {
                let d = euclidean(p, c);
                if d < best_d {
                    best_d = d;
                    best = ci;
                }
            }
            if labels[i] != best as i32 {
                labels[i] = best as i32;
                changed = true;
            }
        }
        // Update centres (empty cluster keeps its previous centre).
        let mut sums = vec![vec![0.0f64; dims]; k];
        let mut counts = vec![0usize; k];
        for (i, p) in points.iter().enumerate() {
            let c = labels[i] as usize;
            counts[c] += 1;
            for d in 0..dims {
                sums[c][d] += p[d];
            }
        }
        for c in 0..k {
            if counts[c] > 0 {
                for d in 0..dims {
                    centres[c][d] = sums[c][d] / counts[c] as f64;
                }
            }
        }
        if !changed {
            break;
        }
    }
    labels
}

// ---- DBSCAN ----------------------------------------------------------------

/// Density-based clustering. A point is a core point if its eps-neighbourhood
/// (including itself) has at least `min_samples` points; clusters grow from core
/// points, and points reachable from no core point are noise (label -1).
fn dbscan(points: &[Vec<f64>], eps: f64, min_samples: usize) -> Vec<i32> {
    let n = points.len();
    let neighbours = |i: usize| -> Vec<usize> {
        (0..n).filter(|&j| euclidean(&points[i], &points[j]) <= eps).collect()
    };
    let mut labels = vec![-2i32; n]; // -2 = unvisited, -1 = noise, >=0 = cluster
    let mut cid = 0i32;
    for i in 0..n {
        if labels[i] != -2 {
            continue;
        }
        let nbrs = neighbours(i);
        if nbrs.len() < min_samples {
            labels[i] = -1; // provisional noise (may be claimed as a border point)
            continue;
        }
        labels[i] = cid;
        let mut seeds = nbrs;
        let mut qi = 0;
        while qi < seeds.len() {
            let j = seeds[qi];
            qi += 1;
            if labels[j] == -1 {
                labels[j] = cid; // border point
            }
            if labels[j] != -2 {
                continue;
            }
            labels[j] = cid;
            let jn = neighbours(j);
            if jn.len() >= min_samples {
                for &x in &jn {
                    if !seeds.contains(&x) {
                        seeds.push(x);
                    }
                }
            }
        }
        cid += 1;
    }
    labels
}

// ---- Hierarchical (agglomerative, Lance–Williams) --------------------------

#[derive(Clone, Copy)]
enum Linkage {
    Average,
    Complete,
    Single,
    Ward,
}

fn parse_linkage(s: &str) -> Result<Linkage, String> {
    match s.trim().to_lowercase().as_str() {
        "average" | "upgma" | "" => Ok(Linkage::Average),
        "complete" | "max" => Ok(Linkage::Complete),
        "single" | "min" => Ok(Linkage::Single),
        "ward" => Ok(Linkage::Ward),
        other => Err(format!(
            "unknown linkage '{other}' — use average, complete, single, or ward"
        )),
    }
}

/// Bottom-up agglomerative clustering, merging the closest pair until `k`
/// clusters remain, using a Lance–Williams distance update for the chosen
/// linkage. Ward operates on squared Euclidean distances (only the merge order
/// matters for the flat labels). Cluster ids are renumbered 0..k in order of
/// their smallest original member index for stable, deterministic output.
fn hierarchical(points: &[Vec<f64>], k: usize, link: Linkage) -> Vec<i32> {
    let n = points.len();
    let ward = matches!(link, Linkage::Ward);
    // Pairwise cluster-distance matrix (starts as point distances).
    let mut d = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let dist = if ward {
                sq_euclidean(&points[i], &points[j])
            } else {
                euclidean(&points[i], &points[j])
            };
            d[i][j] = dist;
            d[j][i] = dist;
        }
    }
    let mut active: Vec<bool> = vec![true; n];
    let mut sizes: Vec<usize> = vec![1; n];
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut remaining = n;

    while remaining > k {
        // Find the closest active pair.
        let mut bi = 0;
        let mut bj = 0;
        let mut best = f64::INFINITY;
        for i in 0..n {
            if !active[i] {
                continue;
            }
            for j in (i + 1)..n {
                if active[j] && d[i][j] < best {
                    best = d[i][j];
                    bi = i;
                    bj = j;
                }
            }
        }
        // Merge bj into bi (Lance–Williams update against every other cluster).
        let dij = d[bi][bj];
        let (ni, nj) = (sizes[bi] as f64, sizes[bj] as f64);
        for m in 0..n {
            if !active[m] || m == bi || m == bj {
                continue;
            }
            let (dik, djk, nk) = (d[bi][m], d[bj][m], sizes[m] as f64);
            let nd = match link {
                Linkage::Single => dik.min(djk),
                Linkage::Complete => dik.max(djk),
                Linkage::Average => (ni * dik + nj * djk) / (ni + nj),
                Linkage::Ward => {
                    ((ni + nk) * dik + (nj + nk) * djk - nk * dij) / (ni + nj + nk)
                }
            };
            d[bi][m] = nd;
            d[m][bi] = nd;
        }
        let moved = std::mem::take(&mut members[bj]);
        members[bi].extend(moved);
        sizes[bi] += sizes[bj];
        active[bj] = false;
        remaining -= 1;
    }

    // Renumber remaining clusters by smallest member index for stable ids.
    let mut clusters: Vec<(usize, usize)> = (0..n)
        .filter(|&i| active[i])
        .map(|i| (*members[i].iter().min().unwrap(), i))
        .collect();
    clusters.sort_unstable();
    let mut labels = vec![0i32; n];
    for (cid, &(_, ci)) in clusters.iter().enumerate() {
        for &m in &members[ci] {
            labels[m] = cid as i32;
        }
    }
    labels
}

// ---- silhouette ------------------------------------------------------------

/// Mean silhouette coefficient over the non-noise points (in the clustering
/// space). Returns None when it is undefined (fewer than 2 clusters, or n above
/// the cap). Points in a singleton cluster contribute 0.
fn silhouette(points: &[Vec<f64>], labels: &[i32], cluster_ids: &[i32]) -> Option<f64> {
    if cluster_ids.len() < 2 {
        return None;
    }
    let idx: Vec<usize> = (0..points.len()).filter(|&i| labels[i] >= 0).collect();
    if idx.len() < 2 || idx.len() > MAX_SILHOUETTE_N {
        return None;
    }
    let mut total = 0.0;
    for &i in &idx {
        let li = labels[i];
        // Mean distance to each cluster.
        let mut sum = std::collections::BTreeMap::<i32, (f64, usize)>::new();
        for &j in &idx {
            if i == j {
                continue;
            }
            let e = sum.entry(labels[j]).or_insert((0.0, 0));
            e.0 += euclidean(&points[i], &points[j]);
            e.1 += 1;
        }
        let a = sum.get(&li).map(|&(s, c)| if c > 0 { s / c as f64 } else { 0.0 }).unwrap_or(0.0);
        let b = sum
            .iter()
            .filter(|(&l, _)| l != li)
            .map(|(_, &(s, c))| s / c as f64)
            .fold(f64::INFINITY, f64::min);
        let s = if !b.is_finite() || a.max(b) < 1e-12 {
            0.0
        } else {
            (b - a) / a.max(b)
        };
        total += s;
    }
    Some(total / idx.len() as f64)
}

// ---- centroids -------------------------------------------------------------

/// Per-cluster centroid in ORIGINAL feature units (for the JSON report).
fn centroid_raw(cl: &Clustered, cid: i32) -> Vec<f64> {
    let dims = cl.feature_names.len();
    let mut sum = vec![0.0f64; dims];
    let mut count = 0usize;
    for (i, &l) in cl.labels.iter().enumerate() {
        if l == cid {
            count += 1;
            for d in 0..dims {
                sum[d] += cl.points_raw[i][d];
            }
        }
    }
    if count > 0 {
        for d in 0..dims {
            sum[d] /= count as f64;
        }
    }
    sum
}

fn cluster_size(cl: &Clustered, cid: i32) -> usize {
    cl.labels.iter().filter(|&&l| l == cid).count()
}

fn noise_count(cl: &Clustered) -> usize {
    cl.labels.iter().filter(|&&l| l == -1).count()
}

// ---- PCA (deterministic power iteration) -----------------------------------

fn mat_vec(cov: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
    cov.iter().map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum()).collect()
}

fn normalize_vec(v: &mut [f64]) -> f64 {
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-12 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    norm
}

/// Deterministic power iteration for the top eigenvector of a symmetric matrix,
/// starting from a fixed seed vector (so the result is reproducible).
fn top_eigenvector(cov: &[Vec<f64>]) -> (Vec<f64>, f64) {
    let d = cov.len();
    // Fixed non-degenerate seed: a linear ramp, then normalized.
    let mut v: Vec<f64> = (0..d).map(|i| 1.0 + i as f64).collect();
    normalize_vec(&mut v);
    for _ in 0..PCA_ITERS {
        let mut nv = mat_vec(cov, &v);
        if normalize_vec(&mut nv) < 1e-18 {
            break;
        }
        v = nv;
    }
    let av = mat_vec(cov, &v);
    let eig = v.iter().zip(&av).map(|(a, b)| a * b).sum::<f64>();
    // Deterministic sign: make the largest-magnitude loading positive.
    let mut max_i = 0;
    for i in 1..d {
        if v[i].abs() > v[max_i].abs() {
            max_i = i;
        }
    }
    if v[max_i] < 0.0 {
        for x in v.iter_mut() {
            *x = -*x;
        }
    }
    (v, eig)
}

/// Project points to their top-2 principal components (mean-centred). Returns the
/// per-point (pc1, pc2). Used for the scatter plot when there are >2 features.
fn pca_2d(points: &[Vec<f64>]) -> Vec<(f64, f64)> {
    let n = points.len();
    let d = points[0].len();
    // Mean-centre.
    let mean: Vec<f64> = (0..d).map(|c| points.iter().map(|p| p[c]).sum::<f64>() / n as f64).collect();
    let centred: Vec<Vec<f64>> =
        points.iter().map(|p| (0..d).map(|c| p[c] - mean[c]).collect()).collect();
    // Covariance (d×d, symmetric).
    let mut cov = vec![vec![0.0f64; d]; d];
    for p in &centred {
        for a in 0..d {
            for b in a..d {
                cov[a][b] += p[a] * p[b];
            }
        }
    }
    for a in 0..d {
        for b in a..d {
            cov[a][b] /= n as f64;
            cov[b][a] = cov[a][b];
        }
    }
    let (v1, eig1) = top_eigenvector(&cov);
    // Deflate and get the second component.
    let mut cov2 = cov.clone();
    for a in 0..d {
        for b in 0..d {
            cov2[a][b] -= eig1 * v1[a] * v1[b];
        }
    }
    let (v2, _) = top_eigenvector(&cov2);
    centred
        .iter()
        .map(|p| {
            let pc1 = p.iter().zip(&v1).map(|(a, b)| a * b).sum::<f64>();
            let pc2 = p.iter().zip(&v2).map(|(a, b)| a * b).sum::<f64>();
            (pc1, pc2)
        })
        .collect()
}

// ---- rendering -------------------------------------------------------------

/// Cluster the data and render it to the requested output format.
pub fn run(data: &str, opts: &Options) -> Result<String, String> {
    let cl = cluster(data, opts)?;
    match opts.output.trim().to_lowercase().as_str() {
        "csv" => Ok(render_csv(&cl)),
        "json" => Ok(render_json(&cl, opts)),
        "chart" | "svg" | "" => render_chart(&cl, opts),
        other => Err(format!(
            "unknown output '{other}' — use chart, csv, or json"
        )),
    }
}

/// A short human label for a point's cluster.
fn label_text(l: i32) -> String {
    if l == -1 {
        "noise".to_string()
    } else {
        format!("cluster {}", l + 1)
    }
}

fn render_csv(cl: &Clustered) -> String {
    let mut out = String::new();
    // Header: original feature names + a cluster column.
    let mut head: Vec<String> = cl.feature_names.iter().map(|s| csv_field(s)).collect();
    head.push("cluster".to_string());
    out.push_str(&head.join(","));
    out.push('\n');
    for (ri, row) in cl.rows.iter().enumerate() {
        let cluster = match cl.row_point[ri] {
            Some(pi) => label_text(cl.labels[pi]),
            None => "skipped".to_string(),
        };
        // Emit the selected feature cells followed by the cluster label so the
        // mapping is unambiguous.
        let mut cells: Vec<String> = Vec::new();
        match cl.row_point[ri] {
            Some(pi) => {
                for d in 0..cl.feature_names.len() {
                    cells.push(fmt_num(cl.points_raw[pi][d]));
                }
            }
            None => {
                // Skipped rows still show their first cells.
                for d in 0..cl.feature_names.len() {
                    cells.push(csv_field(row.get(d).map(|s| s.as_str()).unwrap_or("")));
                }
            }
        }
        cells.push(csv_field(&cluster));
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

fn json_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn render_json(cl: &Clustered, opts: &Options) -> String {
    let sil = silhouette(&cl.points, &cl.labels, &cl.cluster_ids);
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"method\": {},\n", json_str(&cl.method)));
    s.push_str(&format!("  \"points\": {},\n", cl.points.len()));
    s.push_str(&format!("  \"clusters\": {},\n", cl.cluster_ids.len()));
    s.push_str(&format!("  \"noise\": {},\n", noise_count(cl)));
    s.push_str(&format!("  \"normalized\": {},\n", opts.normalize));
    s.push_str("  \"features\": [");
    s.push_str(&cl.feature_names.iter().map(|n| json_str(n)).collect::<Vec<_>>().join(", "));
    s.push_str("],\n");
    match sil {
        Some(v) => s.push_str(&format!("  \"silhouette\": {},\n", fmt_json_num(v))),
        None => s.push_str("  \"silhouette\": null,\n"),
    }
    s.push_str("  \"cluster_detail\": [\n");
    for (i, &cid) in cl.cluster_ids.iter().enumerate() {
        let cent = centroid_raw(cl, cid);
        let cent_str = cent.iter().map(|v| fmt_json_num(*v)).collect::<Vec<_>>().join(", ");
        s.push_str(&format!(
            "    {{ \"cluster\": {}, \"size\": {}, \"centroid\": [{}] }}{}\n",
            cid + 1,
            cluster_size(cl, cid),
            cent_str,
            if i + 1 < cl.cluster_ids.len() { "," } else { "" }
        ));
    }
    s.push_str("  ]\n");
    s.push_str("}\n");
    s
}

fn fmt_json_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if (v.round() - v).abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Compute the (x, y) plot coordinates plus axis names. Two features → the raw
/// columns; one feature → value vs row index; more than two → a PCA projection
/// (PC1/PC2) of the clustering space so high-dimensional data still plots.
fn plot_coords(cl: &Clustered) -> (Vec<f64>, Vec<f64>, String, String) {
    let dims = cl.feature_names.len();
    if dims == 1 {
        let xs: Vec<f64> = cl.points_raw.iter().map(|p| p[0]).collect();
        let ys: Vec<f64> = (1..=cl.points_raw.len()).map(|i| i as f64).collect();
        (xs, ys, cl.feature_names[0].clone(), "row".to_string())
    } else if dims == 2 {
        let xs: Vec<f64> = cl.points_raw.iter().map(|p| p[0]).collect();
        let ys: Vec<f64> = cl.points_raw.iter().map(|p| p[1]).collect();
        (xs, ys, cl.feature_names[0].clone(), cl.feature_names[1].clone())
    } else {
        let proj = pca_2d(&cl.points);
        let xs: Vec<f64> = proj.iter().map(|p| p.0).collect();
        let ys: Vec<f64> = proj.iter().map(|p| p.1).collect();
        (xs, ys, "PC1".to_string(), "PC2".to_string())
    }
}

/// Render an SVG scatter plot with points coloured by cluster, centroid markers,
/// and a legend of cluster sizes.
fn render_chart(cl: &Clustered, opts: &Options) -> Result<String, String> {
    let width = (opts.width.clamp(200, 4000)) as f64;
    let height = (opts.height.clamp(150, 4000)) as f64;

    let (xs, ys, x_name, y_name) = plot_coords(cl);

    // Legend entries: every cluster (with size) plus noise if present.
    let mut legend: Vec<(String, String)> = Vec::new();
    for &cid in &cl.cluster_ids {
        legend.push((
            format!("Cluster {} (n={})", cid + 1, cluster_size(cl, cid)),
            PALETTE[cid as usize % PALETTE.len()].to_string(),
        ));
    }
    let noise = noise_count(cl);
    if noise > 0 {
        legend.push((format!("Noise (n={noise})"), NOISE_COLOR.to_string()));
    }
    let has_legend = !legend.is_empty();

    let m_left = 62.0;
    let m_bottom = 46.0;
    let title = opts.title.trim();
    let m_top = if title.is_empty() { 26.0 } else { 50.0 };
    let m_right = if has_legend { 180.0 } else { 26.0 };
    let plot_w = (width - m_left - m_right).max(20.0);
    let plot_h = (height - m_top - m_bottom).max(20.0);

    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    for &x in &xs {
        xmin = xmin.min(x);
        xmax = xmax.max(x);
    }
    for &y in &ys {
        ymin = ymin.min(y);
        ymax = ymax.max(y);
    }
    if (xmax - xmin).abs() < 1e-12 {
        xmin -= 1.0;
        xmax += 1.0;
    }
    if (ymax - ymin).abs() < 1e-12 {
        ymin -= 1.0;
        ymax += 1.0;
    }
    let xpad = (xmax - xmin) * 0.05;
    let ypad = (ymax - ymin) * 0.05;
    let (xlo, xhi) = (xmin - xpad, xmax + xpad);
    let (ylo, yhi) = (ymin - ypad, ymax + ypad);
    let sx = |x: f64| m_left + (x - xlo) / (xhi - xlo) * plot_w;
    let sy = |y: f64| m_top + plot_h - (y - ylo) / (yhi - ylo) * plot_h;

    let color = |l: i32| -> String {
        if l < 0 {
            NOISE_COLOR.to_string()
        } else {
            PALETTE[l as usize % PALETTE.len()].to_string()
        }
    };

    let mut s = String::new();
    s.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif">"##,
        w = width as i64,
        h = height as i64
    ));
    s.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="#ffffff"/>"##,
        width as i64, height as i64
    ));
    if !title.is_empty() {
        s.push_str(&format!(
            r##"<text x="{x}" y="30" text-anchor="middle" font-size="18" font-weight="bold" fill="#222">{t}</text>"##,
            x = (m_left + plot_w / 2.0) as i64,
            t = esc(title)
        ));
    }

    // Axes.
    let ax_x0 = m_left;
    let ax_y0 = m_top + plot_h;
    s.push_str(&format!(
        r##"<line x1="{x0}" y1="{y0}" x2="{x1}" y2="{y0}" stroke="#888" stroke-width="1"/>"##,
        x0 = ax_x0 as i64,
        y0 = ax_y0 as i64,
        x1 = (m_left + plot_w) as i64
    ));
    s.push_str(&format!(
        r##"<line x1="{x0}" y1="{y0}" x2="{x0}" y2="{y1}" stroke="#888" stroke-width="1"/>"##,
        x0 = ax_x0 as i64,
        y0 = ax_y0 as i64,
        y1 = m_top as i64
    ));

    // Gridlines + tick labels (min / mid / max per axis).
    for f in [0.0_f64, 0.5, 1.0] {
        let xv = xlo + (xhi - xlo) * f;
        let px = sx(xv);
        s.push_str(&format!(
            r##"<line x1="{px}" y1="{y0}" x2="{px}" y2="{y1}" stroke="#eee" stroke-width="1"/>"##,
            px = px as i64,
            y0 = ax_y0 as i64,
            y1 = m_top as i64
        ));
        s.push_str(&format!(
            r##"<text x="{px}" y="{ty}" text-anchor="middle" font-size="11" fill="#555">{lbl}</text>"##,
            px = px as i64,
            ty = (ax_y0 + 16.0) as i64,
            lbl = esc(&fmt_num(xv))
        ));
        let yv = ylo + (yhi - ylo) * f;
        let py = sy(yv);
        s.push_str(&format!(
            r##"<line x1="{x0}" y1="{py}" x2="{x1}" y2="{py}" stroke="#eee" stroke-width="1"/>"##,
            x0 = ax_x0 as i64,
            x1 = (m_left + plot_w) as i64,
            py = py as i64
        ));
        s.push_str(&format!(
            r##"<text x="{tx}" y="{py}" text-anchor="end" font-size="11" fill="#555" dy="4">{lbl}</text>"##,
            tx = (ax_x0 - 8.0) as i64,
            py = py as i64,
            lbl = esc(&fmt_num(yv))
        ));
    }

    // Axis titles.
    s.push_str(&format!(
        r##"<text x="{x}" y="{y}" text-anchor="middle" font-size="12" fill="#333">{lbl}</text>"##,
        x = (m_left + plot_w / 2.0) as i64,
        y = (ax_y0 + 38.0) as i64,
        lbl = esc(&x_name)
    ));
    s.push_str(&format!(
        r##"<text x="{x}" y="{y}" text-anchor="middle" font-size="12" fill="#333" transform="rotate(-90 {x} {y})">{lbl}</text>"##,
        x = 16,
        y = (m_top + plot_h / 2.0) as i64,
        lbl = esc(&y_name)
    ));

    // Points, coloured by cluster.
    for i in 0..xs.len() {
        s.push_str(&format!(
            r##"<circle cx="{cx:.1}" cy="{cy:.1}" r="4.2" fill="{c}" fill-opacity="0.75" stroke="#ffffff" stroke-width="0.6"/>"##,
            cx = sx(xs[i]),
            cy = sy(ys[i]),
            c = color(cl.labels[i])
        ));
    }

    // Centroid markers (diamond) per cluster — mean of the plotted coordinates.
    for &cid in &cl.cluster_ids {
        let members: Vec<usize> = (0..cl.labels.len()).filter(|&i| cl.labels[i] == cid).collect();
        if members.is_empty() {
            continue;
        }
        let mx = members.iter().map(|&i| xs[i]).sum::<f64>() / members.len() as f64;
        let my = members.iter().map(|&i| ys[i]).sum::<f64>() / members.len() as f64;
        let (cx, cy) = (sx(mx), sy(my));
        let r = 7.0;
        s.push_str(&format!(
            r##"<path class="centroid" d="M {x0:.1} {y0:.1} L {x1:.1} {y1:.1} L {x2:.1} {y2:.1} L {x3:.1} {y3:.1} Z" fill="{c}" stroke="#222" stroke-width="1.6"/>"##,
            x0 = cx,
            y0 = cy - r,
            x1 = cx + r,
            y1 = cy,
            x2 = cx,
            y2 = cy + r,
            x3 = cx - r,
            y3 = cy,
            c = PALETTE[cid as usize % PALETTE.len()]
        ));
    }

    // Legend.
    if has_legend {
        let lx = m_left + plot_w + 16.0;
        let mut ly = m_top + 8.0;
        for (lbl, col) in &legend {
            s.push_str(&format!(
                r##"<circle cx="{cx}" cy="{cy}" r="6" fill="{col}"/>"##,
                cx = (lx + 6.0) as i64,
                cy = ly as i64
            ));
            s.push_str(&format!(
                r##"<text x="{tx}" y="{ty}" font-size="12" fill="#333">{lbl}</text>"##,
                tx = (lx + 18.0) as i64,
                ty = (ly + 4.0) as i64,
                lbl = esc(lbl)
            ));
            ly += 20.0;
        }
    }

    s.push_str("</svg>");
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(method: &str, output: &str) -> Options {
        Options {
            method: method.into(),
            clusters: 2,
            eps: 1.0,
            min_samples: 2,
            linkage: "average".into(),
            columns: String::new(),
            normalize: true,
            output: output.into(),
            title: String::new(),
            width: 700,
            height: 500,
        }
    }

    // Two well-separated blobs in 2D.
    const BLOBS: &str = "x,y\n0,0\n0.2,0.1\n0.1,0.2\n10,10\n10.1,9.9\n9.9,10.2";

    #[test]
    fn kmeans_separates_two_blobs() {
        let o = opts("kmeans", "json");
        let out = run(BLOBS, &o).unwrap();
        assert!(out.contains("\"clusters\": 2"), "{out}");
        assert!(out.contains("\"method\": \"kmeans\""));
        // Each blob has 3 points.
        assert_eq!(out.matches("\"size\": 3").count(), 2, "{out}");
    }

    #[test]
    fn kmeans_is_deterministic() {
        let o = opts("kmeans", "csv");
        let a = run(BLOBS, &o).unwrap();
        let b = run(BLOBS, &o).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn chart_output_is_svg_coloured_by_cluster() {
        let mut o = opts("kmeans", "chart");
        o.title = "My clusters".into();
        let svg = run(BLOBS, &o).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("My clusters"));
        // 6 data points + 2 legend swatches = 8 circles.
        assert_eq!(svg.matches("<circle").count(), 8, "{svg}");
        // one centroid marker per cluster
        assert_eq!(svg.matches("class=\"centroid\"").count(), 2, "{svg}");
        assert!(svg.contains("Cluster 1 (n=3)"));
        assert!(svg.contains("Cluster 2 (n=3)"));
        // axis titles from the header
        assert!(svg.contains(">x<"));
        assert!(svg.contains(">y<"));
        // two distinct palette colours
        assert!(svg.contains(PALETTE[0]));
        assert!(svg.contains(PALETTE[1]));
    }

    #[test]
    fn csv_output_maps_rows_to_clusters() {
        let o = opts("kmeans", "csv");
        let out = run(BLOBS, &o).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "x,y,cluster");
        assert_eq!(lines.len(), 7); // header + 6 rows
        assert!(lines[1].ends_with(",cluster 1") || lines[1].ends_with(",cluster 2"));
    }

    #[test]
    fn dbscan_finds_clusters_and_noise() {
        // Two dense blobs + one far outlier that should be noise.
        let data = "x,y\n0,0\n0.1,0\n0,0.1\n5,5\n5.1,5\n5,5.1\n50,50";
        let mut o = opts("dbscan", "json");
        o.eps = 0.5;
        o.min_samples = 2;
        o.normalize = false; // use raw distances so eps is interpretable
        let out = run(data, &o).unwrap();
        assert!(out.contains("\"method\": \"dbscan\""));
        assert!(out.contains("\"noise\": 1"), "{out}");
        assert!(out.contains("\"clusters\": 2"), "{out}");
    }

    #[test]
    fn hierarchical_separates_blobs() {
        let o = opts("hierarchical", "json");
        let out = run(BLOBS, &o).unwrap();
        assert!(out.contains("\"method\": \"hierarchical\""));
        assert!(out.contains("\"clusters\": 2"));
        assert_eq!(out.matches("\"size\": 3").count(), 2, "{out}");
    }

    #[test]
    fn hierarchical_ward_and_complete_linkage_work() {
        for link in ["ward", "complete", "single"] {
            let mut o = opts("hierarchical", "json");
            o.linkage = link.into();
            let out = run(BLOBS, &o).unwrap();
            assert!(out.contains("\"clusters\": 2"), "linkage {link}: {out}");
            assert_eq!(out.matches("\"size\": 3").count(), 2, "linkage {link}: {out}");
        }
        // Bad linkage errors clearly.
        let mut o = opts("hierarchical", "json");
        o.linkage = "banana".into();
        assert!(run(BLOBS, &o).unwrap_err().contains("linkage"));
    }

    #[test]
    fn column_selection_by_name() {
        // The `label` column is non-numeric and must be ignorable via `columns`.
        let data = "a,b,label\n0,0,foo\n0.1,0.1,bar\n9,9,baz\n9.1,9.1,qux";
        let mut o = opts("kmeans", "json");
        o.columns = "a,b".into();
        let out = run(data, &o).unwrap();
        assert!(out.contains("\"features\": [\"a\", \"b\"]"), "{out}");
        assert!(out.contains("\"clusters\": 2"));
    }

    #[test]
    fn auto_selects_numeric_columns_only() {
        let data = "name,age,score\nAda,30,90\nBob,31,88\nCy,80,10\nDot,81,12";
        let o = opts("kmeans", "json");
        let out = run(data, &o).unwrap();
        // `name` is dropped; age + score kept.
        assert!(out.contains("\"features\": [\"age\", \"score\"]"), "{out}");
    }

    #[test]
    fn silhouette_reported_for_separated_blobs() {
        let o = opts("kmeans", "json");
        let out = run(BLOBS, &o).unwrap();
        // Well-separated blobs → silhouette present and non-null.
        assert!(out.contains("\"silhouette\":"));
        assert!(!out.contains("\"silhouette\": null"), "{out}");
    }

    #[test]
    fn high_dim_data_projects_to_pca_axes() {
        // 4 features → chart uses PC1/PC2.
        let data = "a,b,c,d\n0,0,0,0\n0.1,0.1,0,0.1\n0,0.1,0.1,0\n9,9,9,9\n9.1,8.9,9,9.1\n8.9,9.1,9,9";
        let o = opts("kmeans", "chart");
        let svg = run(data, &o).unwrap();
        assert!(svg.contains(">PC1<"), "{svg}");
        assert!(svg.contains(">PC2<"), "{svg}");
        // Still coloured into two clusters.
        assert!(svg.contains(PALETTE[0]));
        assert!(svg.contains(PALETTE[1]));
    }

    #[test]
    fn pca_is_deterministic() {
        let data = "a,b,c,d\n0,0,0,0\n0.1,0.1,0,0.1\n0,0.1,0.1,0\n9,9,9,9\n9.1,8.9,9,9.1\n8.9,9.1,9,9";
        let o = opts("kmeans", "chart");
        assert_eq!(run(data, &o).unwrap(), run(data, &o).unwrap());
    }

    #[test]
    fn errors_are_actionable() {
        // Empty input.
        assert!(run("", &opts("kmeans", "chart")).is_err());
        // k larger than the data.
        let mut o = opts("kmeans", "json");
        o.clusters = 99;
        assert!(run(BLOBS, &o).unwrap_err().contains("larger than"));
        // No numeric columns.
        assert!(run("a,b\nfoo,bar\nbaz,qux", &opts("kmeans", "chart"))
            .unwrap_err()
            .contains("numeric"));
        // Unknown method.
        assert!(run(BLOBS, &opts("banana", "json")).unwrap_err().contains("unknown method"));
        // dbscan with eps <= 0.
        let mut o = opts("dbscan", "json");
        o.eps = 0.0;
        assert!(run(BLOBS, &o).unwrap_err().contains("eps"));
        // Unknown output.
        assert!(run(BLOBS, &opts("kmeans", "yaml")).unwrap_err().contains("unknown output"));
    }

    #[test]
    fn single_feature_plots_against_row_index() {
        let data = "v\n1\n2\n3\n20\n21\n22";
        let o = opts("kmeans", "chart");
        let svg = run(data, &o).unwrap();
        assert!(svg.contains(">row<"), "{svg}");
        assert!(svg.contains(">v<"));
    }
}
