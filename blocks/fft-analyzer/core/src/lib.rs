//! fft-analyzer — discrete Fourier transform of a pasted sample list.
//!
//! Pure, deterministic, dependency-light: a hand-rolled iterative radix-2 FFT for
//! power-of-two lengths and a direct O(N²) DFT for exact non-power-of-two lengths
//! (both instantiate cleanly under wasmi and wasm32-unknown-unknown). Emits a
//! frequency/magnitude/phase table plus peak detection in text, CSV, JSON, or a
//! Unicode bar chart.

use std::f64::consts::PI;
use std::fmt::Write as _;

/// Largest accepted input sample count. Above this the FFT buffers and the
/// rendered table stop being reasonable for an in-browser tool.
pub const MAX_SAMPLES: usize = 65_536;
/// Direct-DFT cap. `pad = none` with a non-power-of-two length is O(N²).
pub const MAX_DIRECT_DFT: usize = 4_096;
/// Rows printed by the `text`/`chart` formats before truncating with a note.
const MAX_TEXT_ROWS: usize = 1_024;
/// Bars drawn by the `chart` format before truncating with a note.
const MAX_CHART_ROWS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cx {
    re: f64,
    im: f64,
}

impl Cx {
    fn new(re: f64, im: f64) -> Self {
        Cx { re, im }
    }
    fn abs(&self) -> f64 {
        self.re.hypot(self.im)
    }
}

/// Windows, in the order they appear in the descriptor enum.
#[derive(Clone, Copy, PartialEq)]
enum Window {
    Rectangular,
    Hann,
    Hamming,
    Blackman,
    BlackmanHarris,
    FlatTop,
}

impl Window {
    fn parse(s: &str) -> Result<Window, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "rectangular" | "rect" | "none" => Ok(Window::Rectangular),
            "hann" | "hanning" => Ok(Window::Hann),
            "hamming" => Ok(Window::Hamming),
            "blackman" => Ok(Window::Blackman),
            "blackman-harris" | "blackmanharris" => Ok(Window::BlackmanHarris),
            "flattop" | "flat-top" => Ok(Window::FlatTop),
            other => Err(format!(
                "window must be one of rectangular, hann, hamming, blackman, blackman-harris, flattop (got '{other}')"
            )),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Window::Rectangular => "rectangular",
            Window::Hann => "hann",
            Window::Hamming => "hamming",
            Window::Blackman => "blackman",
            Window::BlackmanHarris => "blackman-harris",
            Window::FlatTop => "flattop",
        }
    }

    /// Periodic (DFT-even) window coefficients — the correct convention for
    /// spectral analysis, as opposed to the symmetric filter-design form.
    fn coefficients(&self, n: usize) -> Vec<f64> {
        if n == 0 {
            return Vec::new();
        }
        if *self == Window::Rectangular {
            return vec![1.0; n];
        }
        let nf = n as f64;
        (0..n)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / nf;
                match self {
                    Window::Rectangular => 1.0,
                    Window::Hann => 0.5 - 0.5 * t.cos(),
                    Window::Hamming => 0.54 - 0.46 * t.cos(),
                    Window::Blackman => 0.42 - 0.5 * t.cos() + 0.08 * (2.0 * t).cos(),
                    Window::BlackmanHarris => {
                        0.35875 - 0.48829 * t.cos() + 0.14128 * (2.0 * t).cos()
                            - 0.01168 * (3.0 * t).cos()
                    }
                    Window::FlatTop => {
                        0.21557895 - 0.41663158 * t.cos() + 0.277263158 * (2.0 * t).cos()
                            - 0.083578947 * (3.0 * t).cos()
                            + 0.006947368 * (4.0 * t).cos()
                    }
                }
            })
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Scale {
    Amplitude,
    Magnitude,
    Normalized,
    Db,
    Power,
}

impl Scale {
    fn parse(s: &str) -> Result<Scale, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "amplitude" => Ok(Scale::Amplitude),
            "magnitude" => Ok(Scale::Magnitude),
            "normalized" | "normalised" => Ok(Scale::Normalized),
            "db" => Ok(Scale::Db),
            "power" => Ok(Scale::Power),
            other => Err(format!(
                "scale must be one of amplitude, magnitude, normalized, db, power (got '{other}')"
            )),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Scale::Amplitude => "amplitude",
            Scale::Magnitude => "magnitude",
            Scale::Normalized => "normalized",
            Scale::Db => "db",
            Scale::Power => "power",
        }
    }

    fn column(&self) -> &'static str {
        match self {
            Scale::Amplitude => "amplitude",
            Scale::Magnitude => "magnitude",
            Scale::Normalized => "normalized",
            Scale::Db => "dB",
            Scale::Power => "power",
        }
    }
}

/// Parsed request, shared by every output format.
struct Spectrum {
    input_len: usize,
    fft_len: usize,
    sample_rate: f64,
    window: Window,
    scale: Scale,
    phase_degrees: bool,
    complex_input: bool,
    one_sided: bool,
    removed_dc: f64,
    coherent_gain: f64,
    rows: Vec<Row>,
    peaks: Vec<usize>,
    nyquist_warning: bool,
}

struct Row {
    bin: usize,
    freq: f64,
    value: f64,
    phase: f64,
    re: f64,
    im: f64,
}

/// Entry point shared by the chat block, the CLI, and the browser page.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    data: &str,
    sample_rate: f64,
    window: &str,
    pad: &str,
    spectrum: &str,
    scale: &str,
    phase_unit: &str,
    remove_dc: bool,
    peaks: i64,
    decimals: i64,
    format: &str,
) -> Result<String, String> {
    let window = Window::parse(window)?;
    let scale = Scale::parse(scale)?;
    let pad = match pad.trim().to_ascii_lowercase().as_str() {
        "" | "pow2" | "power-of-two" => "pow2",
        "none" | "exact" => "none",
        other => return Err(format!("pad must be pow2 or none (got '{other}')")),
    };
    let view = match spectrum.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "" | "auto" => "auto",
        "one-sided" | "onesided" | "half" => "one-sided",
        "two-sided" | "twosided" | "full" => "two-sided",
        other => {
            return Err(format!(
                "spectrum must be auto, one-sided, or two-sided (got '{other}')"
            ))
        }
    };
    let phase_degrees = match phase_unit.trim().to_ascii_lowercase().as_str() {
        "" | "degrees" | "deg" => true,
        "radians" | "rad" => false,
        other => return Err(format!("phase_unit must be degrees or radians (got '{other}')")),
    };
    let format = match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => "text",
        "csv" => "csv",
        "json" => "json",
        "chart" => "chart",
        other => {
            return Err(format!(
                "format must be text, csv, json, or chart (got '{other}')"
            ))
        }
    };
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return Err(format!(
            "sample_rate must be a positive number of samples per second (got '{sample_rate}')"
        ));
    }
    let peaks = peaks.clamp(0, 20) as usize;
    let decimals = decimals.clamp(0, 12) as usize;

    let (mut samples, complex_input) = parse_samples(data)?;
    let input_len = samples.len();

    let removed_dc = if remove_dc {
        let mean_re = samples.iter().map(|c| c.re).sum::<f64>() / input_len as f64;
        let mean_im = samples.iter().map(|c| c.im).sum::<f64>() / input_len as f64;
        for c in samples.iter_mut() {
            c.re -= mean_re;
            c.im -= mean_im;
        }
        mean_re
    } else {
        0.0
    };

    // Window is applied over the REAL samples only, before any zero padding —
    // padding must not be tapered or the effective window changes shape.
    let coeffs = window.coefficients(input_len);
    let coherent_gain = coeffs.iter().sum::<f64>() / input_len as f64;
    for (c, w) in samples.iter_mut().zip(coeffs.iter()) {
        c.re *= *w;
        c.im *= *w;
    }

    let fft_len = if pad == "pow2" {
        next_power_of_two(input_len)
    } else {
        input_len
    };
    if fft_len > MAX_SAMPLES * 2 {
        return Err(format!(
            "transform length {fft_len} is too large; keep the input at or under {MAX_SAMPLES} samples"
        ));
    }
    if pad == "none" && !fft_len.is_power_of_two() && fft_len > MAX_DIRECT_DFT {
        return Err(format!(
            "pad=none with {fft_len} samples uses a direct O(N^2) DFT, which is capped at {MAX_DIRECT_DFT} samples; use pad=pow2 or trim the input"
        ));
    }
    samples.resize(fft_len, Cx::new(0.0, 0.0));

    let bins = if fft_len.is_power_of_two() {
        fft_radix2(&samples)
    } else {
        dft_direct(&samples)
    };

    let one_sided = match view {
        "one-sided" => true,
        "two-sided" => false,
        _ => !complex_input,
    };
    let shown = if one_sided { fft_len / 2 + 1 } else { fft_len };

    // Amplitude scaling: a real tone of amplitude A splits its energy between the
    // +f and -f bins, so the one-sided view doubles every bin except DC and (for
    // even N) Nyquist. Windowing lowers the peak by the coherent gain; divide it
    // back out so a Hann-windowed tone still reads its true amplitude.
    let gain = if coherent_gain.abs() < 1e-12 { 1.0 } else { coherent_gain };
    let mut rows = Vec::with_capacity(shown);
    let peak_ref = bins.iter().map(|c| c.abs()).fold(0.0_f64, f64::max);
    for k in 0..shown {
        let c = bins[k];
        let mag = c.abs();
        let two_sided_bin = k == 0 || (fft_len % 2 == 0 && k == fft_len / 2);
        let amp_factor = if one_sided && !two_sided_bin { 2.0 } else { 1.0 };
        let normalized = mag / input_len as f64 / gain;
        let value = match scale {
            Scale::Amplitude => normalized * amp_factor,
            Scale::Magnitude => mag,
            Scale::Normalized => normalized,
            Scale::Db => {
                let a = normalized * amp_factor;
                if a <= 0.0 {
                    f64::NEG_INFINITY
                } else {
                    20.0 * a.log10()
                }
            }
            Scale::Power => (normalized * amp_factor).powi(2),
        };
        // Phase of a numerically-zero bin is arbitrary floating-point noise;
        // report 0 rather than a random angle from rounding dust.
        let phase_rad = if peak_ref > 0.0 && mag <= peak_ref * 1e-10 {
            0.0
        } else {
            c.im.atan2(c.re)
        };
        let phase = if phase_degrees {
            phase_rad.to_degrees()
        } else {
            phase_rad
        };
        // Two-sided bins above N/2 are the negative frequencies.
        let signed_k = if !one_sided && k > fft_len / 2 {
            k as f64 - fft_len as f64
        } else {
            k as f64
        };
        rows.push(Row {
            bin: k,
            freq: signed_k * sample_rate / fft_len as f64,
            value,
            phase,
            re: c.re,
            im: c.im,
        });
    }

    let peak_idx = find_peaks(&rows, peaks, scale);
    let nyquist_warning = peak_idx
        .iter()
        .any(|&i| one_sided && rows[i].bin > 0 && rows[i].bin == fft_len / 2);

    let s = Spectrum {
        input_len,
        fft_len,
        sample_rate,
        window,
        scale,
        phase_degrees,
        complex_input,
        one_sided,
        removed_dc,
        coherent_gain,
        rows,
        peaks: peak_idx,
        nyquist_warning,
    };

    Ok(match format {
        "csv" => render_csv(&s, decimals),
        "json" => render_json(&s, decimals),
        "chart" => render_chart(&s, decimals),
        _ => render_text(&s, decimals),
    })
}

fn next_power_of_two(n: usize) -> usize {
    let mut p = 1usize;
    while p < n {
        p <<= 1;
    }
    p.max(1)
}

/// Parse a sample list. Accepts real (`1`, `-2.5`, `2.7e3`) and complex
/// (`3+4i`, `-2i`, `1.25-0.5j`) values separated by commas, semicolons,
/// whitespace, or newlines. Returns the samples plus whether any was complex.
fn parse_samples(data: &str) -> Result<(Vec<Cx>, bool), String> {
    let mut out = Vec::new();
    let mut any_complex = false;
    for (idx, tok) in data
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|t| !t.trim().is_empty())
        .enumerate()
    {
        if out.len() >= MAX_SAMPLES {
            return Err(format!(
                "too many samples: this tool accepts at most {MAX_SAMPLES} values"
            ));
        }
        let (c, is_complex) = parse_one(tok.trim(), idx + 1)?;
        any_complex |= is_complex;
        out.push(c);
    }
    if out.is_empty() {
        return Err(
            "no samples found: paste numbers separated by commas, spaces, semicolons, or newlines (for example 0, 1, 0, -1)"
                .to_string(),
        );
    }
    if out.len() < 2 {
        return Err(format!(
            "need at least 2 samples to compute a spectrum (got {})",
            out.len()
        ));
    }
    Ok((out, any_complex))
}

fn parse_one(tok: &str, pos: usize) -> Result<(Cx, bool), String> {
    let lower = tok.to_ascii_lowercase();
    let bad = |what: &str| {
        format!("sample {pos} ('{tok}') is not a valid {what}; use forms like 1.5, -2, 2.7e3, 3+4i, or -2i")
    };
    if !lower.ends_with('i') && !lower.ends_with('j') {
        let v: f64 = lower.parse().map_err(|_| bad("number"))?;
        if !v.is_finite() {
            return Err(bad("finite number"));
        }
        return Ok((Cx::new(v, 0.0), false));
    }
    // Complex: strip the trailing i/j, then split off an optional real part at
    // the last +/- that is not an exponent sign.
    let body = &lower[..lower.len() - 1];
    let mut split = None;
    for (i, ch) in body.char_indices().skip(1) {
        if (ch == '+' || ch == '-')
            && !matches!(body.as_bytes()[i - 1], b'e' | b'E')
        {
            split = Some(i);
        }
    }
    let (re_str, im_str) = match split {
        Some(i) => (&body[..i], &body[i..]),
        None => ("0", body),
    };
    let re: f64 = if re_str.is_empty() {
        0.0
    } else {
        re_str.parse().map_err(|_| bad("complex number"))?
    };
    let im: f64 = match im_str {
        "" | "+" => 1.0,
        "-" => -1.0,
        s => s.parse().map_err(|_| bad("complex number"))?,
    };
    if !re.is_finite() || !im.is_finite() {
        return Err(bad("finite complex number"));
    }
    Ok((Cx::new(re, im), true))
}

/// Iterative in-place radix-2 decimation-in-time FFT. Twiddles are recomputed
/// per stage from f64 trig — no tables, no allocation beyond the output buffer.
fn fft_radix2(input: &[Cx]) -> Vec<Cx> {
    let n = input.len();
    let mut a = input.to_vec();
    if n < 2 {
        return a;
    }
    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            a.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let t = ang * k as f64;
                let (ws, wc) = t.sin_cos();
                let u = a[start + k];
                let v = a[start + k + len / 2];
                let vr = v.re * wc - v.im * ws;
                let vi = v.re * ws + v.im * wc;
                a[start + k] = Cx::new(u.re + vr, u.im + vi);
                a[start + k + len / 2] = Cx::new(u.re - vr, u.im - vi);
            }
        }
        len <<= 1;
    }
    a
}

/// Direct DFT for exact non-power-of-two lengths (`pad = none`).
fn dft_direct(input: &[Cx]) -> Vec<Cx> {
    let n = input.len();
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let mut re = 0.0;
        let mut im = 0.0;
        for (t, x) in input.iter().enumerate() {
            let ang = -2.0 * PI * (k as f64) * (t as f64) / n as f64;
            let (s, c) = ang.sin_cos();
            re += x.re * c - x.im * s;
            im += x.re * s + x.im * c;
        }
        out.push(Cx::new(re, im));
    }
    out
}

/// Rank local maxima by magnitude. Peaks are strictly-greater-than-or-equal
/// local maxima so a flat two-bin ridge still yields one entry.
fn find_peaks(rows: &[Row], want: usize, scale: Scale) -> Vec<usize> {
    if want == 0 || rows.is_empty() {
        return Vec::new();
    }
    // dB values go negative; rank on a monotone-equivalent magnitude instead.
    let key = |r: &Row| -> f64 {
        if scale == Scale::Db {
            r.re.hypot(r.im)
        } else {
            r.value.abs()
        }
    };
    let mut candidates: Vec<usize> = (0..rows.len())
        .filter(|&i| {
            let v = key(&rows[i]);
            if v <= 0.0 {
                return false;
            }
            let left = i == 0 || key(&rows[i - 1]) <= v;
            let right = i + 1 == rows.len() || key(&rows[i + 1]) < v;
            left && right
        })
        .collect();
    candidates.sort_by(|&a, &b| {
        key(&rows[b])
            .partial_cmp(&key(&rows[a]))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    candidates.truncate(want);
    candidates
}

fn fmt(v: f64, d: usize) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    let s = format!("{v:.d$}", d = d);
    // Avoid "-0.0000" in output tables.
    if s.chars().all(|c| matches!(c, '-' | '0' | '.')) && s.starts_with('-') {
        s[1..].to_string()
    } else {
        s
    }
}

fn header_lines(s: &Spectrum, d: usize) -> Vec<(String, String)> {
    let resolution = s.sample_rate / s.fft_len as f64;
    let mut v = vec![
        ("samples".into(), s.input_len.to_string()),
        ("transform length".into(), s.fft_len.to_string()),
        ("sample rate".into(), format!("{} /s", fmt(s.sample_rate, d))),
        ("resolution".into(), format!("{} per bin", fmt(resolution, d))),
        ("nyquist".into(), fmt(s.sample_rate / 2.0, d)),
        ("window".into(), s.window.label().to_string()),
        ("scale".into(), s.scale.label().to_string()),
        (
            "spectrum".into(),
            if s.one_sided { "one-sided" } else { "two-sided" }.to_string(),
        ),
    ];
    if s.complex_input {
        v.push(("input".into(), "complex".into()));
    }
    if s.removed_dc != 0.0 {
        v.push(("removed dc".into(), fmt(s.removed_dc, d)));
    }
    if s.window != Window::Rectangular {
        v.push(("coherent gain".into(), fmt(s.coherent_gain, d)));
    }
    v
}

fn phase_label(s: &Spectrum) -> &'static str {
    if s.phase_degrees {
        "phase(deg)"
    } else {
        "phase(rad)"
    }
}

fn render_text(s: &Spectrum, d: usize) -> String {
    let mut o = String::new();
    o.push_str("Spectrum\n");
    for (k, v) in header_lines(s, d) {
        let _ = writeln!(o, "  {k:<17} {v}");
    }
    if !s.peaks.is_empty() {
        o.push_str("\nPeaks\n");
        for (rank, &i) in s.peaks.iter().enumerate() {
            let r = &s.rows[i];
            let _ = writeln!(
                o,
                "  {}. bin {}  freq {}  {} {}  {} {}",
                rank + 1,
                r.bin,
                fmt(r.freq, d),
                s.scale.column(),
                fmt(r.value, d),
                phase_label(s),
                fmt(r.phase, d)
            );
        }
    }
    if s.nyquist_warning {
        o.push_str(
            "\nNote: a top peak sits in the Nyquist bin, which usually means the signal is aliased. Raise the sample rate.\n",
        );
    }
    o.push_str("\nBins\n");
    let _ = writeln!(
        o,
        "  bin  frequency  {}  {}",
        s.scale.column(),
        phase_label(s)
    );
    for r in s.rows.iter().take(MAX_TEXT_ROWS) {
        let _ = writeln!(
            o,
            "  {}  {}  {}  {}",
            r.bin,
            fmt(r.freq, d),
            fmt(r.value, d),
            fmt(r.phase, d)
        );
    }
    if s.rows.len() > MAX_TEXT_ROWS {
        let _ = writeln!(
            o,
            "  … {} more bins not shown — use format=csv or format=json for the full table",
            s.rows.len() - MAX_TEXT_ROWS
        );
    }
    o
}

fn render_chart(s: &Spectrum, d: usize) -> String {
    const WIDTH: usize = 40;
    let mut o = String::new();
    o.push_str("Spectrum\n");
    for (k, v) in header_lines(s, d) {
        let _ = writeln!(o, "  {k:<17} {v}");
    }
    let shown = s.rows.len().min(MAX_CHART_ROWS);
    // dB is negative-going; chart the linear magnitude so bars stay meaningful.
    let bar_value = |r: &Row| -> f64 {
        if s.scale == Scale::Db {
            r.re.hypot(r.im)
        } else {
            r.value.abs()
        }
    };
    let max = s.rows[..shown]
        .iter()
        .map(bar_value)
        .fold(0.0_f64, f64::max);
    let width = s.rows[..shown]
        .iter()
        .map(|r| fmt(r.freq, d).len())
        .max()
        .unwrap_or(4);
    let _ = writeln!(o, "\n{} by frequency\n", s.scale.column());
    for r in s.rows[..shown].iter() {
        let filled = if max > 0.0 {
            ((bar_value(r) / max) * WIDTH as f64).round() as usize
        } else {
            0
        };
        let _ = writeln!(
            o,
            "  {:>width$}  {}{}  {}",
            fmt(r.freq, d),
            "█".repeat(filled),
            "·".repeat(WIDTH - filled),
            fmt(r.value, d),
            width = width
        );
    }
    if s.rows.len() > shown {
        let _ = writeln!(
            o,
            "  … {} more bins not shown — use format=csv or format=json for the full table",
            s.rows.len() - shown
        );
    }
    if s.nyquist_warning {
        o.push_str(
            "\nNote: a top peak sits in the Nyquist bin, which usually means the signal is aliased. Raise the sample rate.\n",
        );
    }
    o
}

fn render_csv(s: &Spectrum, d: usize) -> String {
    let mut o = String::new();
    o.push_str("section,key,value\n");
    for (k, v) in header_lines(s, d) {
        let _ = writeln!(o, "meta,{k},{v}");
    }
    for (rank, &i) in s.peaks.iter().enumerate() {
        let r = &s.rows[i];
        let _ = writeln!(
            o,
            "peak,{},bin={} frequency={} {}={}",
            rank + 1,
            r.bin,
            fmt(r.freq, d),
            s.scale.column(),
            fmt(r.value, d)
        );
    }
    let _ = writeln!(
        o,
        "\nbin,frequency,{},{},real,imaginary",
        s.scale.column(),
        phase_label(s)
    );
    for r in s.rows.iter() {
        let _ = writeln!(
            o,
            "{},{},{},{},{},{}",
            r.bin,
            fmt(r.freq, d),
            fmt(r.value, d),
            fmt(r.phase, d),
            fmt(r.re, d),
            fmt(r.im, d)
        );
    }
    o
}

fn json_num(v: f64, d: usize) -> String {
    if v.is_finite() {
        fmt(v, d)
    } else {
        "null".to_string()
    }
}

fn render_json(s: &Spectrum, d: usize) -> String {
    let mut o = String::new();
    o.push_str("{\n");
    let _ = writeln!(o, "  \"samples\": {},", s.input_len);
    let _ = writeln!(o, "  \"transform_length\": {},", s.fft_len);
    let _ = writeln!(o, "  \"sample_rate\": {},", json_num(s.sample_rate, d));
    let _ = writeln!(
        o,
        "  \"resolution\": {},",
        json_num(s.sample_rate / s.fft_len as f64, d)
    );
    let _ = writeln!(o, "  \"nyquist\": {},", json_num(s.sample_rate / 2.0, d));
    let _ = writeln!(o, "  \"window\": \"{}\",", s.window.label());
    let _ = writeln!(o, "  \"scale\": \"{}\",", s.scale.label());
    let _ = writeln!(
        o,
        "  \"spectrum\": \"{}\",",
        if s.one_sided { "one-sided" } else { "two-sided" }
    );
    let _ = writeln!(o, "  \"complex_input\": {},", s.complex_input);
    let _ = writeln!(
        o,
        "  \"phase_unit\": \"{}\",",
        if s.phase_degrees { "degrees" } else { "radians" }
    );
    let _ = writeln!(o, "  \"removed_dc\": {},", json_num(s.removed_dc, d));
    let _ = writeln!(o, "  \"aliasing_warning\": {},", s.nyquist_warning);
    o.push_str("  \"peaks\": [");
    for (n, &i) in s.peaks.iter().enumerate() {
        let r = &s.rows[i];
        if n > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "\n    {{ \"bin\": {}, \"frequency\": {}, \"{}\": {}, \"phase\": {} }}",
            r.bin,
            json_num(r.freq, d),
            s.scale.column(),
            json_num(r.value, d),
            json_num(r.phase, d)
        );
    }
    o.push_str(if s.peaks.is_empty() { "],\n" } else { "\n  ],\n" });
    o.push_str("  \"bins\": [");
    for (n, r) in s.rows.iter().enumerate() {
        if n > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "\n    {{ \"bin\": {}, \"frequency\": {}, \"{}\": {}, \"phase\": {}, \"real\": {}, \"imaginary\": {} }}",
            r.bin,
            json_num(r.freq, d),
            s.scale.column(),
            json_num(r.value, d),
            json_num(r.phase, d),
            json_num(r.re, d),
            json_num(r.im, d)
        );
    }
    o.push_str("\n  ]\n}\n");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_default(data: &str) -> Result<String, String> {
        analyze(data, 1.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 5, 4, "text")
    }

    #[test]
    fn recovers_a_pure_tone_amplitude_and_frequency() {
        // 1 Hz cosine, unit amplitude, sampled 8x per second for one second.
        let samples: Vec<String> = (0..8)
            .map(|n| format!("{}", (2.0 * PI * n as f64 / 8.0).cos()))
            .collect();
        let out = analyze(
            &samples.join(","), 8.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false,
            3, 4, "text",
        )
        .unwrap();
        assert!(out.contains("1. bin 1  freq 1.0000  amplitude 1.0000"), "{out}");
        assert!(out.contains("resolution        1.0000 per bin"), "{out}");
        assert!(out.contains("nyquist           4.0000"), "{out}");
    }

    #[test]
    fn dc_signal_puts_all_energy_in_bin_zero() {
        let out = analyze("2,2,2,2", 4.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 1, 4, "text").unwrap();
        assert!(out.contains("1. bin 0  freq 0.0000  amplitude 2.0000"), "{out}");
        // Every other bin is empty.
        assert!(out.contains("  1  1.0000  0.0000  0.0000"), "{out}");
    }

    #[test]
    fn remove_dc_strips_the_offset_bin() {
        let out = analyze("5,5,5,5", 4.0, "rectangular", "pow2", "auto", "amplitude", "degrees", true, 2, 4, "json").unwrap();
        assert!(out.contains("\"removed_dc\": 5.0000"), "{out}");
        assert!(out.contains("\"peaks\": []"), "{out}");
    }

    #[test]
    fn two_tone_signal_reports_both_peaks() {
        let n = 64;
        let samples: Vec<String> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64;
                format!(
                    "{}",
                    (2.0 * PI * 4.0 * t).sin() + 0.5 * (2.0 * PI * 10.0 * t).sin()
                )
            })
            .collect();
        let out = analyze(&samples.join(" "), n as f64, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 2, 3, "text").unwrap();
        assert!(out.contains("1. bin 4  freq 4.000  amplitude 1.000"), "{out}");
        assert!(out.contains("2. bin 10  freq 10.000  amplitude 0.500"), "{out}");
    }

    #[test]
    fn zero_pads_to_the_next_power_of_two() {
        let out = run_default("1,2,3,4,5,6").unwrap();
        assert!(out.contains("samples           6"), "{out}");
        assert!(out.contains("transform length  8"), "{out}");
    }

    #[test]
    fn pad_none_uses_the_exact_length() {
        let out = analyze("1,2,3,4,5,6", 6.0, "rectangular", "none", "auto", "amplitude", "degrees", false, 1, 4, "text").unwrap();
        assert!(out.contains("transform length  6"), "{out}");
    }

    #[test]
    fn direct_dft_matches_the_radix2_fft_on_a_shared_length() {
        let data = "1,4,-2,7,0,3,9,-5";
        let a = analyze(data, 8.0, "rectangular", "pow2", "auto", "magnitude", "radians", false, 0, 6, "csv").unwrap();
        let b = analyze(data, 8.0, "rectangular", "none", "auto", "magnitude", "radians", false, 0, 6, "csv").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn complex_input_selects_the_two_sided_spectrum() {
        let out = analyze("1,i,-1,-i", 4.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 1, 4, "text").unwrap();
        assert!(out.contains("input             complex"), "{out}");
        assert!(out.contains("spectrum          two-sided"), "{out}");
        // A unit phasor rotating one turn per 4 samples lands entirely in bin 1.
        assert!(out.contains("1. bin 1  freq 1.0000  amplitude 1.0000"), "{out}");
    }

    #[test]
    fn two_sided_view_labels_negative_frequencies() {
        let out = analyze("1,0,-1,0", 4.0, "rectangular", "pow2", "two-sided", "magnitude", "degrees", false, 0, 1, "text").unwrap();
        assert!(out.contains("  3  -1.0"), "{out}");
    }

    #[test]
    fn windowing_preserves_measured_tone_amplitude() {
        let n = 32;
        let samples: Vec<String> = (0..n)
            .map(|i| format!("{}", 3.0 * (2.0 * PI * 4.0 * i as f64 / n as f64).cos()))
            .collect();
        let out = analyze(&samples.join(","), n as f64, "hann", "pow2", "auto", "amplitude", "degrees", false, 1, 3, "text").unwrap();
        // Coherent-gain correction keeps the Hann-windowed peak at the true amplitude.
        assert!(out.contains("1. bin 4  freq 4.000  amplitude 3.000"), "{out}");
        assert!(out.contains("coherent gain     0.500"), "{out}");
    }

    #[test]
    fn db_scale_reports_zero_db_for_a_unit_tone() {
        let n = 16;
        let samples: Vec<String> = (0..n)
            .map(|i| format!("{}", (2.0 * PI * 2.0 * i as f64 / n as f64).cos()))
            .collect();
        let out = analyze(&samples.join(","), n as f64, "rectangular", "pow2", "auto", "db", "degrees", false, 1, 2, "text").unwrap();
        assert!(out.contains("1. bin 2  freq 2.00  dB 0.00"), "{out}");
    }

    #[test]
    fn chart_format_draws_bars() {
        let out = analyze("1,0,-1,0,1,0,-1,0", 8.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 1, 2, "chart").unwrap();
        assert!(out.contains("█"), "{out}");
        assert!(out.contains("amplitude by frequency"), "{out}");
    }

    #[test]
    fn csv_carries_real_and_imaginary_columns() {
        let out = analyze("1,0,0,0", 4.0, "rectangular", "pow2", "auto", "magnitude", "degrees", false, 0, 2, "csv").unwrap();
        assert!(out.contains("bin,frequency,magnitude,phase(deg),real,imaginary"), "{out}");
        assert!(out.contains("0,0.00,1.00,0.00,1.00,0.00"), "{out}");
    }

    #[test]
    fn json_is_parseable() {
        let out = analyze("1,2,3,4", 4.0, "rectangular", "pow2", "auto", "amplitude", "radians", false, 2, 4, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["transform_length"], 4);
        assert_eq!(v["phase_unit"], "radians");
        assert_eq!(v["bins"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn scientific_notation_and_mixed_separators_parse() {
        let out = run_default("1e1; 2E1\n-3.5, 4").unwrap();
        assert!(out.contains("samples           4"), "{out}");
    }

    #[test]
    fn nyquist_peak_raises_an_aliasing_note() {
        let out = analyze("1,-1,1,-1,1,-1,1,-1", 8.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 1, 2, "text").unwrap();
        assert!(out.contains("aliased"), "{out}");
    }

    // ---- error paths ----

    #[test]
    fn rejects_empty_input() {
        let err = run_default("   ").unwrap_err();
        assert!(err.contains("no samples found"), "{err}");
    }

    #[test]
    fn rejects_a_single_sample() {
        let err = run_default("42").unwrap_err();
        assert!(err.contains("at least 2 samples"), "{err}");
    }

    #[test]
    fn rejects_a_non_numeric_sample_with_its_position() {
        let err = run_default("1,2,abc,4").unwrap_err();
        assert!(err.contains("sample 3 ('abc')"), "{err}");
        assert!(err.contains("3+4i"), "{err}");
    }

    #[test]
    fn rejects_a_non_positive_sample_rate() {
        let err = analyze("1,2,3,4", 0.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 5, 4, "text").unwrap_err();
        assert!(err.contains("sample_rate must be a positive"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_window() {
        let err = analyze("1,2,3,4", 1.0, "kaiser", "pow2", "auto", "amplitude", "degrees", false, 5, 4, "text").unwrap_err();
        assert!(err.contains("window must be one of"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_format() {
        let err = analyze("1,2,3,4", 1.0, "hann", "pow2", "auto", "amplitude", "degrees", false, 5, 4, "xml").unwrap_err();
        assert!(err.contains("format must be text, csv, json, or chart"), "{err}");
    }

    #[test]
    fn rejects_too_many_samples() {
        let data = vec!["1"; MAX_SAMPLES + 1].join(",");
        let err = run_default(&data).unwrap_err();
        assert!(err.contains("too many samples"), "{err}");
    }

    #[test]
    fn rejects_an_oversized_direct_dft() {
        // 4097 samples is not a power of two, so pad=none takes the O(N^2) path.
        let data = vec!["1"; MAX_DIRECT_DFT + 1].join(",");
        let err = analyze(&data, 1.0, "rectangular", "none", "auto", "amplitude", "degrees", false, 0, 2, "text").unwrap_err();
        assert!(err.contains("direct O(N^2) DFT"), "{err}");
    }

    #[test]
    fn accepts_the_exact_sample_cap() {
        let data = vec!["1"; MAX_SAMPLES].join(",");
        let out = analyze(&data, 1.0, "rectangular", "pow2", "auto", "amplitude", "degrees", false, 1, 2, "text").unwrap();
        assert!(out.contains(&format!("samples           {MAX_SAMPLES}")), "header missing");
        assert!(out.contains("more bins not shown"), "large tables truncate with a note");
    }
}
