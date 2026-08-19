//! gizza-ai/spectral-eq-match core — pure compute shared by the chat skill
//! block, the CLI and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Derives a CORRECTIVE EQ curve from a measured/target spectrum pair. The user
//! supplies their track's per-band levels (whatever a spectrum analyser, RTA or
//! meter reported) plus a reference — either pasted reference band levels or a
//! built-in target curve — and the tool returns the per-band corrective gains, a
//! ready-to-paste ffmpeg `equalizer`/`firequalizer` filter chain and, when the
//! two integrated-loudness figures are supplied, the broadband loudness offset
//! in dB.
//!
//! Everything here is arithmetic on numbers the user already measured: no audio
//! is decoded, no FFT is run. That keeps the tool inside gizza's single-media-
//! input model (a two-audio-file matcher has no viable ffmpeg surface) while
//! still producing the artifact people actually want — the filter chain.

use std::fmt::Write as _;

/// Maximum number of bands accepted in either curve. 64 comfortably covers a
/// 31-band third-octave RTA plus headroom; beyond that the "EQ" is a convolution
/// job, not a filter chain.
pub const MAX_BANDS: usize = 64;

/// Gains smaller than this (in dB) are dropped from the `equalizer` chain — a
/// 0.04 dB peaking filter is a no-op that only costs a processing stage.
pub const GAIN_EPSILON_DB: f64 = 0.05;

/// A single measured band: centre frequency in Hz and its level in dB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub hz: f64,
    pub db: f64,
}

/// Which target the track is matched against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetCurve {
    /// Use the pasted reference band levels.
    Reference,
    /// 0 dB at every band (equal level per band).
    Flat,
    /// −3 dB per octave referenced to 1 kHz (equal energy per octave).
    Pink,
    /// −2 dB per octave: a shallower tilt, relatively more top end.
    Bright,
    /// −4 dB per octave: a steeper tilt, relatively more low end.
    Warm,
    /// Pink tilt plus a low-end rolloff, a presence lift and an air cut.
    Speech,
}

impl TargetCurve {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "reference" => Ok(TargetCurve::Reference),
            "flat" => Ok(TargetCurve::Flat),
            "pink" => Ok(TargetCurve::Pink),
            "bright" => Ok(TargetCurve::Bright),
            "warm" => Ok(TargetCurve::Warm),
            "speech" => Ok(TargetCurve::Speech),
            other => Err(format!(
                "unknown target_curve '{other}' (expected reference, flat, pink, bright, warm or speech)"
            )),
        }
    }

    /// Human-readable description used in the report header.
    fn label(self) -> &'static str {
        match self {
            TargetCurve::Reference => "pasted reference",
            TargetCurve::Flat => "flat (0 dB at every band)",
            TargetCurve::Pink => "pink (-3.0 dB/octave from 1 kHz)",
            TargetCurve::Bright => "bright (-2.0 dB/octave from 1 kHz)",
            TargetCurve::Warm => "warm (-4.0 dB/octave from 1 kHz)",
            TargetCurve::Speech => {
                "speech (pink tilt, rolloff below 150 Hz, +3 dB presence at 3 kHz, air cut above 10 kHz)"
            }
        }
    }

    /// Preset target level in dB at `hz`, referenced to 0 dB at 1 kHz.
    fn preset_db(self, hz: f64) -> f64 {
        let oct = (hz / 1000.0).log2();
        match self {
            TargetCurve::Reference => 0.0,
            TargetCurve::Flat => 0.0,
            TargetCurve::Pink => -3.0 * oct,
            TargetCurve::Bright => -2.0 * oct,
            TargetCurve::Warm => -4.0 * oct,
            TargetCurve::Speech => {
                let mut db = -3.0 * oct;
                if hz < 150.0 {
                    db -= 6.0 * (150.0 / hz).log2();
                }
                let p = (hz / 3000.0).log2();
                db += 3.0 * (-(p * p)).exp();
                if hz > 10_000.0 {
                    db -= 3.0 * (hz / 10_000.0).log2();
                }
                db
            }
        }
    }
}

/// What the tool returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Full per-band table plus settings, loudness offset and the ffmpeg command.
    Report,
    /// A single ready-to-paste `ffmpeg -af "…"` command using peaking `equalizer` stages.
    Ffmpeg,
    /// A single ready-to-paste `ffmpeg -af "…"` command using one `firequalizer` stage.
    Firequalizer,
    /// Machine-readable per-band CSV.
    Csv,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Ok(Output::Report),
            "ffmpeg" => Ok(Output::Ffmpeg),
            "firequalizer" => Ok(Output::Firequalizer),
            "csv" => Ok(Output::Csv),
            other => Err(format!(
                "unknown output '{other}' (expected report, ffmpeg, firequalizer or csv)"
            )),
        }
    }
}

/// Parse one numeric token. Frequencies accept a `k`/`K` multiplier and a `Hz`
/// suffix (`1k`, `1.6kHz`, `12500`); levels accept a `dB`/`dBFS` suffix.
fn parse_token(tok: &str, is_freq: bool) -> Result<f64, String> {
    let raw = tok.trim();
    let mut s = raw.to_ascii_lowercase();
    for suffix in ["dbfs", "dbr", "db", "hz"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.trim().to_string();
        }
    }
    let mut scale = 1.0_f64;
    if is_freq {
        if let Some(stripped) = s.strip_suffix('k') {
            scale = 1000.0;
            s = stripped.trim().to_string();
        }
    }
    let v: f64 = s
        .parse()
        .map_err(|_| format!("'{raw}' is not a number (expected a frequency/level pair)"))?;
    if !v.is_finite() {
        return Err(format!("'{raw}' is not a finite number"));
    }
    Ok(v * scale)
}

/// Parse a band list. Frequency/level pairs may be separated by newlines,
/// commas, semicolons, colons, equals signs or plain whitespace — every
/// delimiter is treated alike and the tokens are paired up in order.
pub fn parse_bands(input: &str, what: &str) -> Result<Vec<Band>, String> {
    let toks: Vec<&str> = input
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':' | '=' | '|'))
        .filter(|t| !t.is_empty())
        .collect();
    if toks.is_empty() {
        return Err(format!("{what} band levels are empty"));
    }
    if toks.len() % 2 != 0 {
        return Err(format!(
            "{what} band levels have {} values; expected pairs of frequency and level (for example '1k -3.5')",
            toks.len()
        ));
    }
    let count = toks.len() / 2;
    if count > MAX_BANDS {
        return Err(format!(
            "{what} band levels have {count} bands; the limit is {MAX_BANDS}"
        ));
    }
    let mut bands = Vec::with_capacity(count);
    for pair in toks.chunks(2) {
        let hz = parse_token(pair[0], true)?;
        let db = parse_token(pair[1], false)?;
        if !(hz > 0.0 && hz <= 200_000.0) {
            return Err(format!(
                "{what} band frequency {hz} Hz is out of range (expected 0 < Hz <= 200000)"
            ));
        }
        if !(-200.0..=200.0).contains(&db) {
            return Err(format!(
                "{what} band level {db} dB is out of range (expected -200..200 dB)"
            ));
        }
        bands.push(Band { hz, db });
    }
    bands.sort_by(|a, b| a.hz.partial_cmp(&b.hz).unwrap());
    for w in bands.windows(2) {
        if (w[0].hz - w[1].hz).abs() < 1e-9 {
            return Err(format!(
                "{what} band levels list {} Hz twice; give each band one level",
                fmt_num(w[0].hz)
            ));
        }
    }
    Ok(bands)
}

/// Linear-in-dB interpolation over log frequency, holding the endpoint values
/// outside the measured range.
fn interp_log(points: &[Band], hz: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    if hz <= points[0].hz {
        return points[0].db;
    }
    let last = points[points.len() - 1];
    if hz >= last.hz {
        return last.db;
    }
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        if hz >= a.hz && hz <= b.hz {
            let t = (hz.ln() - a.hz.ln()) / (b.hz.ln() - a.hz.ln());
            return a.db + t * (b.db - a.db);
        }
    }
    last.db
}

/// Format a plain number with trailing zeros trimmed (`31.5`, `1000`, `0.7`).
fn fmt_num(v: f64) -> String {
    let mut s = format!("{v:.4}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// Two-decimal dB with a `-0.00` guard.
fn fmt_db(v: f64) -> String {
    let r = if v.abs() < 0.005 { 0.0 } else { v };
    format!("{r:.2}")
}

/// Two-decimal signed dB (`+1.90`, `-0.40`, `0.00`).
fn fmt_signed_db(v: f64) -> String {
    let r = if v.abs() < 0.005 { 0.0 } else { v };
    if r > 0.0 {
        format!("+{r:.2}")
    } else {
        format!("{r:.2}")
    }
}

/// One band's full derivation, in the order the report prints it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchedBand {
    pub hz: f64,
    /// The measured track level.
    pub track_db: f64,
    /// The target level at this frequency.
    pub target_db: f64,
    /// Raw difference (target − track), after the optional tone-only offset removal.
    pub diff_db: f64,
    /// `diff_db` after neighbour smoothing.
    pub smoothed_db: f64,
    /// Final corrective gain, after `amount` scaling and the ±limit clamp.
    pub gain_db: f64,
}

/// The complete derivation, before it is rendered into one of the output forms.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    pub bands: Vec<MatchedBand>,
    pub curve: TargetCurve,
    pub reference_bands: usize,
    pub amount: f64,
    pub max_gain_db: f64,
    pub smoothing: u32,
    pub q: f64,
    pub tone_only: bool,
    /// Broadband offset removed by `tone_only` (target − track, averaged).
    pub level_offset_db: f64,
    /// `target_lufs - track_lufs`, when both were supplied.
    pub loudness_offset_db: Option<f64>,
    pub track_lufs: f64,
    pub target_lufs: f64,
    /// Bands clamped by the ±`max_gain_db` limit.
    pub clamped: usize,
}

#[allow(clippy::too_many_arguments)]
fn derive(
    track: &str,
    reference: &str,
    target_curve: &str,
    amount: f64,
    max_gain_db: f64,
    smoothing: u32,
    q: f64,
    tone_only: bool,
    track_lufs: f64,
    target_lufs: f64,
) -> Result<MatchResult, String> {
    let curve = TargetCurve::parse(target_curve)?;
    if !(0.0..=100.0).contains(&amount) {
        return Err(format!(
            "amount {} is out of range (expected 0-100 percent)",
            fmt_num(amount)
        ));
    }
    if !(0.0..=24.0).contains(&max_gain_db) {
        return Err(format!(
            "max_gain_db {} is out of range (expected 0-24 dB)",
            fmt_num(max_gain_db)
        ));
    }
    if smoothing > 4 {
        return Err(format!(
            "smoothing {smoothing} is out of range (expected 0-4 neighbouring bands)"
        ));
    }
    if !(0.1..=10.0).contains(&q) {
        return Err(format!(
            "q {} is out of range (expected 0.1-10)",
            fmt_num(q)
        ));
    }
    for (name, v) in [("track_lufs", track_lufs), ("target_lufs", target_lufs)] {
        if !(-70.0..=0.0).contains(&v) {
            return Err(format!(
                "{name} {} is out of range (expected -70..0 LUFS, or 0 to skip the loudness offset)",
                fmt_num(v)
            ));
        }
    }

    let track_bands = parse_bands(track, "track")?;
    if track_bands.len() < 2 {
        return Err(format!(
            "track band levels have {} band; a curve needs at least 2 bands",
            track_bands.len()
        ));
    }

    let ref_bands = if curve == TargetCurve::Reference {
        parse_bands(reference, "reference").map_err(|e| {
            if reference.trim().is_empty() {
                "target_curve is 'reference' but no reference band levels were given; paste them or pick a built-in curve (flat, pink, bright, warm, speech)".to_string()
            } else {
                e
            }
        })?
    } else {
        Vec::new()
    };

    // Target level at each of the track's band frequencies.
    let targets: Vec<f64> = track_bands
        .iter()
        .map(|b| match curve {
            TargetCurve::Reference => interp_log(&ref_bands, b.hz),
            other => other.preset_db(b.hz),
        })
        .collect();

    let raw: Vec<f64> = track_bands
        .iter()
        .zip(&targets)
        .map(|(b, t)| t - b.db)
        .collect();

    let level_offset_db = raw.iter().sum::<f64>() / raw.len() as f64;
    let diff: Vec<f64> = if tone_only {
        raw.iter().map(|d| d - level_offset_db).collect()
    } else {
        raw.clone()
    };

    let r = smoothing as usize;
    let n = diff.len();
    let smoothed: Vec<f64> = (0..n)
        .map(|i| {
            let lo = i.saturating_sub(r);
            let hi = (i + r).min(n - 1);
            let slice = &diff[lo..=hi];
            slice.iter().sum::<f64>() / slice.len() as f64
        })
        .collect();

    let mut clamped = 0usize;
    let mut bands = Vec::with_capacity(n);
    for i in 0..n {
        let scaled = smoothed[i] * amount / 100.0;
        let gain = scaled.clamp(-max_gain_db, max_gain_db);
        if (gain - scaled).abs() > 1e-9 {
            clamped += 1;
        }
        bands.push(MatchedBand {
            hz: track_bands[i].hz,
            track_db: track_bands[i].db,
            target_db: targets[i],
            diff_db: diff[i],
            smoothed_db: smoothed[i],
            gain_db: gain,
        });
    }

    let loudness_offset_db = if track_lufs != 0.0 && target_lufs != 0.0 {
        Some(target_lufs - track_lufs)
    } else {
        None
    };

    Ok(MatchResult {
        bands,
        curve,
        reference_bands: ref_bands.len(),
        amount,
        max_gain_db,
        smoothing,
        q,
        tone_only,
        level_offset_db,
        loudness_offset_db,
        track_lufs,
        target_lufs,
        clamped,
    })
}

impl MatchResult {
    /// The peaking-`equalizer` filter chain (no `ffmpeg` prefix, no quotes).
    /// Bands whose gain rounds below [`GAIN_EPSILON_DB`] are omitted.
    pub fn equalizer_chain(&self) -> String {
        let mut stages: Vec<String> = self
            .bands
            .iter()
            .filter(|b| b.gain_db.abs() >= GAIN_EPSILON_DB)
            .map(|b| {
                format!(
                    "equalizer=f={}:t=q:w={}:g={}",
                    fmt_num(b.hz),
                    fmt_num(self.q),
                    fmt_db(b.gain_db)
                )
            })
            .collect();
        if let Some(off) = self.loudness_offset_db {
            if off.abs() >= 0.005 {
                stages.push(format!("volume={}dB", fmt_db(off)));
            }
        }
        stages.join(",")
    }

    /// The single-stage `firequalizer` chain. Every band is listed (the filter
    /// interpolates a continuous curve between the entries).
    pub fn firequalizer_chain(&self) -> String {
        let entries: Vec<String> = self
            .bands
            .iter()
            .map(|b| format!("entry({},{})", fmt_num(b.hz), fmt_db(b.gain_db)))
            .collect();
        let mut stages = vec![format!("firequalizer=gain_entry='{}'", entries.join(";"))];
        if let Some(off) = self.loudness_offset_db {
            if off.abs() >= 0.005 {
                stages.push(format!("volume={}dB", fmt_db(off)));
            }
        }
        stages.join(",")
    }

    fn command(chain: &str) -> String {
        format!("ffmpeg -i input.wav -af \"{chain}\" output.wav")
    }
}

/// Render the derivation as a human-readable report.
fn render_report(m: &MatchResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Spectral EQ match: {} bands", m.bands.len());
    let _ = match m.curve {
        TargetCurve::Reference => writeln!(
            out,
            "Target curve: {} ({} bands, interpolated onto the track bands)",
            m.curve.label(),
            m.reference_bands
        ),
        _ => writeln!(out, "Target curve: {}", m.curve.label()),
    };
    let _ = writeln!(
        out,
        "Settings: amount {}% | smoothing +/-{} band(s) | limit +/-{} dB | Q {} | tone-only {}",
        fmt_num(m.amount),
        m.smoothing,
        fmt_db(m.max_gain_db),
        fmt_num(m.q),
        if m.tone_only { "on" } else { "off" }
    );
    if m.tone_only {
        let _ = writeln!(
            out,
            "Broadband level removed before matching: {} dB",
            fmt_signed_db(m.level_offset_db)
        );
    } else {
        let _ = writeln!(
            out,
            "Broadband level difference (kept in the gains): {} dB",
            fmt_signed_db(m.level_offset_db)
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:>10}  {:>8}  {:>8}  {:>8}  {:>9}  {:>8}",
        "Freq (Hz)", "Track", "Target", "Diff", "Smoothed", "Gain"
    );
    for b in &m.bands {
        let _ = writeln!(
            out,
            "{:>10}  {:>8}  {:>8}  {:>8}  {:>9}  {:>8}",
            fmt_num(b.hz),
            fmt_db(b.track_db),
            fmt_db(b.target_db),
            fmt_signed_db(b.diff_db),
            fmt_signed_db(b.smoothed_db),
            fmt_signed_db(b.gain_db)
        );
    }
    let _ = writeln!(out);
    if m.clamped > 0 {
        let _ = writeln!(
            out,
            "{} band(s) hit the +/-{} dB limit.",
            m.clamped,
            fmt_db(m.max_gain_db)
        );
    }
    match m.loudness_offset_db {
        Some(off) => {
            let _ = writeln!(
                out,
                "Loudness: {:.1} LUFS -> {:.1} LUFS = {} dB offset",
                m.track_lufs,
                m.target_lufs,
                fmt_signed_db(off)
            );
        }
        None => {
            let _ = writeln!(
                out,
                "Loudness: not calculated (set both track_lufs and target_lufs)"
            );
        }
    }
    let _ = writeln!(out);
    let chain = m.equalizer_chain();
    if chain.is_empty() {
        let _ = writeln!(
            out,
            "No correction needed: every band is already within {GAIN_EPSILON_DB} dB of the target."
        );
    } else {
        let _ = writeln!(out, "{}", MatchResult::command(&chain));
    }
    out
}

fn render_csv(m: &MatchResult) -> String {
    let mut out = String::from("frequency_hz,track_db,target_db,diff_db,smoothed_db,gain_db\n");
    for b in &m.bands {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{}",
            fmt_num(b.hz),
            fmt_db(b.track_db),
            fmt_db(b.target_db),
            fmt_db(b.diff_db),
            fmt_db(b.smoothed_db),
            fmt_db(b.gain_db)
        );
    }
    out
}

/// Derive the corrective EQ and render it in the requested `output` form.
///
/// * `track` / `reference` — frequency/level pairs (`"31.5 -8.2, 63 -4.1"`, or one pair per line).
/// * `target_curve` — `reference` (use the pasted reference) or a preset: `flat`, `pink`,
///   `bright`, `warm`, `speech`.
/// * `amount` — percentage of the derived correction to apply (0–100).
/// * `max_gain_db` — symmetric boost/cut limit in dB (0–24).
/// * `smoothing` — neighbouring-band averaging radius (0–4).
/// * `q` — Q of every peaking `equalizer` stage (0.1–10).
/// * `tone_only` — remove the broadband level difference before matching.
/// * `track_lufs` / `target_lufs` — integrated loudness; both 0 skips the offset.
/// * `output` — `report`, `ffmpeg`, `firequalizer` or `csv`.
#[allow(clippy::too_many_arguments)]
pub fn match_eq(
    track: &str,
    reference: &str,
    target_curve: &str,
    amount: f64,
    max_gain_db: f64,
    smoothing: u32,
    q: f64,
    tone_only: bool,
    track_lufs: f64,
    target_lufs: f64,
    output: &str,
) -> Result<String, String> {
    let out_kind = Output::parse(output)?;
    let m = derive(
        track,
        reference,
        target_curve,
        amount,
        max_gain_db,
        smoothing,
        q,
        tone_only,
        track_lufs,
        target_lufs,
    )?;
    Ok(match out_kind {
        Output::Report => render_report(&m),
        Output::Csv => render_csv(&m),
        Output::Ffmpeg => {
            let chain = m.equalizer_chain();
            if chain.is_empty() {
                format!(
                    "No correction needed: every band is already within {GAIN_EPSILON_DB} dB of the target."
                )
            } else {
                MatchResult::command(&chain)
            }
        }
        Output::Firequalizer => MatchResult::command(&m.firequalizer_chain()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: &str = "63 -12, 250 -8, 1k -6, 4k -10, 12k -18";
    const REF: &str = "63 -10, 250 -8, 1k -6, 4k -8, 12k -14";

    #[test]
    fn derives_gains_against_a_pasted_reference() {
        let out = match_eq(TRACK, REF, "reference", 100.0, 6.0, 0, 1.0, false, 0.0, 0.0, "csv")
            .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "frequency_hz,track_db,target_db,diff_db,smoothed_db,gain_db"
        );
        // 63 Hz: target -10 minus track -12 = +2.00 dB of boost.
        assert_eq!(lines[1], "63,-12.00,-10.00,2.00,2.00,2.00");
        // 1 kHz matches exactly -> no correction.
        assert_eq!(lines[3], "1000,-6.00,-6.00,0.00,0.00,0.00");
        // 12 kHz: -14 - -18 = +4.00 dB.
        assert_eq!(lines[5], "12000,-18.00,-14.00,4.00,4.00,4.00");
    }

    #[test]
    fn ffmpeg_output_is_a_ready_to_paste_command() {
        let out = match_eq(TRACK, REF, "reference", 100.0, 6.0, 0, 1.0, false, 0.0, 0.0, "ffmpeg")
            .unwrap();
        assert_eq!(
            out,
            "ffmpeg -i input.wav -af \"equalizer=f=63:t=q:w=1:g=2.00,equalizer=f=4000:t=q:w=1:g=2.00,equalizer=f=12000:t=q:w=1:g=4.00\" output.wav"
        );
    }

    #[test]
    fn tone_only_removes_the_broadband_offset() {
        // Reference is a uniform +6 dB louder version of the track: with
        // tone_only the tonal correction is zero everywhere.
        let track = "100 -20, 1k -20, 10k -20";
        let reference = "100 -14, 1k -14, 10k -14";
        let toned =
            match_eq(track, reference, "reference", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "ffmpeg")
                .unwrap();
        assert_eq!(
            toned,
            "No correction needed: every band is already within 0.05 dB of the target."
        );
        let raw =
            match_eq(track, reference, "reference", 100.0, 6.0, 0, 1.0, false, 0.0, 0.0, "ffmpeg")
                .unwrap();
        assert!(raw.contains("equalizer=f=100:t=q:w=1:g=6.00"), "{raw}");
    }

    #[test]
    fn amount_scales_and_the_limit_clamps() {
        let track = "100 -30, 1k -10";
        let reference = "100 -10, 1k -10";
        let half =
            match_eq(track, reference, "reference", 50.0, 24.0, 0, 1.0, false, 0.0, 0.0, "csv")
                .unwrap();
        assert!(half.contains("100,-30.00,-10.00,20.00,20.00,10.00"), "{half}");
        let clamped =
            match_eq(track, reference, "reference", 100.0, 3.0, 0, 1.0, false, 0.0, 0.0, "report")
                .unwrap();
        assert!(clamped.contains("hit the +/-3.00 dB limit"), "{clamped}");
    }

    #[test]
    fn smoothing_averages_neighbouring_bands() {
        // One isolated 6 dB notch; +/-1 band smoothing spreads it across three bands.
        let track = "100 -10, 200 -16, 400 -10";
        let reference = "100 -10, 200 -10, 400 -10";
        let sharp =
            match_eq(track, reference, "reference", 100.0, 12.0, 0, 1.0, false, 0.0, 0.0, "csv")
                .unwrap();
        assert!(sharp.contains("200,-16.00,-10.00,6.00,6.00,6.00"), "{sharp}");
        let soft =
            match_eq(track, reference, "reference", 100.0, 12.0, 1, 1.0, false, 0.0, 0.0, "csv")
                .unwrap();
        assert!(soft.contains("200,-16.00,-10.00,6.00,2.00,2.00"), "{soft}");
        assert!(soft.contains("100,-10.00,-10.00,0.00,3.00,3.00"), "{soft}");
    }

    #[test]
    fn reference_is_interpolated_onto_the_track_bands() {
        // Two reference points an octave apart; the track band sits between them.
        let out = match_eq(
            "500 -10, 1k -10, 2k -10",
            "500 -12, 2k -6",
            "reference",
            100.0,
            12.0,
            0,
            1.0,
            false,
            0.0,
            0.0,
            "csv",
        )
        .unwrap();
        // log-midpoint of 500..2000 is 1000 -> target halfway between -12 and -6.
        assert!(out.contains("1000,-10.00,-9.00,1.00,1.00,1.00"), "{out}");
    }

    #[test]
    fn preset_curves_need_no_reference() {
        let out = match_eq(
            "500 -10, 1k -10, 2k -10",
            "",
            "pink",
            100.0,
            12.0,
            0,
            1.0,
            false,
            0.0,
            0.0,
            "csv",
        )
        .unwrap();
        // pink is -3 dB/octave from 1 kHz: +3 at 500 Hz, 0 at 1 kHz, -3 at 2 kHz.
        assert!(out.contains("500,-10.00,3.00,13.00"), "{out}");
        assert!(out.contains("1000,-10.00,0.00,10.00"), "{out}");
        assert!(out.contains("2000,-10.00,-3.00,7.00"), "{out}");
        for curve in ["flat", "bright", "warm", "speech"] {
            let r = match_eq(
                "500 -10, 1k -10, 2k -10",
                "",
                curve,
                100.0,
                12.0,
                0,
                1.0,
                true,
                0.0,
                0.0,
                "csv",
            );
            assert!(r.is_ok(), "{curve}: {r:?}");
        }
    }

    #[test]
    fn loudness_offset_appends_a_volume_stage() {
        let out = match_eq(
            TRACK, REF, "reference", 100.0, 6.0, 0, 1.0, false, -9.5, -14.0, "ffmpeg",
        )
        .unwrap();
        assert!(out.ends_with("volume=-4.50dB\" output.wav"), "{out}");
        let none = match_eq(
            TRACK, REF, "reference", 100.0, 6.0, 0, 1.0, false, 0.0, 0.0, "ffmpeg",
        )
        .unwrap();
        assert!(!none.contains("volume="), "{none}");
    }

    #[test]
    fn firequalizer_lists_every_band() {
        let out = match_eq(
            TRACK,
            REF,
            "reference",
            100.0,
            6.0,
            0,
            1.0,
            false,
            0.0,
            0.0,
            "firequalizer",
        )
        .unwrap();
        assert_eq!(
            out,
            "ffmpeg -i input.wav -af \"firequalizer=gain_entry='entry(63,2.00);entry(250,0.00);entry(1000,0.00);entry(4000,2.00);entry(12000,4.00)'\" output.wav"
        );
    }

    #[test]
    fn accepts_k_suffixes_hz_db_units_and_mixed_separators() {
        let a = match_eq(
            "1kHz: -6dB\n2k = -8dB",
            "1k -6\n2k -6",
            "reference",
            100.0,
            6.0,
            0,
            1.0,
            false,
            0.0,
            0.0,
            "csv",
        )
        .unwrap();
        assert!(a.contains("2000,-8.00,-6.00,2.00"), "{a}");
    }

    #[test]
    fn report_shows_the_table_settings_and_command() {
        let out = match_eq(
            TRACK, REF, "reference", 100.0, 6.0, 1, 1.4, true, -9.5, -14.0, "report",
        )
        .unwrap();
        assert!(out.starts_with("Spectral EQ match: 5 bands\n"), "{out}");
        assert!(out.contains("Target curve: pasted reference (5 bands, interpolated"), "{out}");
        assert!(out.contains("smoothing +/-1 band(s)"), "{out}");
        assert!(out.contains("| Q 1.4 |"), "{out}");
        assert!(out.contains("Broadband level removed before matching: +1.60 dB"), "{out}");
        assert!(out.contains("Loudness: -9.5 LUFS -> -14.0 LUFS = -4.50 dB offset"), "{out}");
        assert!(out.contains("ffmpeg -i input.wav -af \""), "{out}");
        assert!(out.contains(":t=q:w=1.4:g="), "{out}");
    }

    #[test]
    fn errors_when_reference_curve_has_no_reference_levels() {
        let err = match_eq(TRACK, "  ", "reference", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "report")
            .unwrap_err();
        assert!(err.contains("no reference band levels"), "{err}");
    }

    #[test]
    fn errors_on_odd_token_count_and_bad_numbers() {
        let err = match_eq("100 -10 200", REF, "reference", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "csv")
            .unwrap_err();
        assert!(err.contains("expected pairs"), "{err}");
        let err = match_eq("100 loud", REF, "reference", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "csv")
            .unwrap_err();
        assert!(err.contains("is not a number"), "{err}");
    }

    #[test]
    fn errors_on_duplicate_bands_single_band_and_bad_enums() {
        let err = match_eq("1k -6, 1k -8", REF, "reference", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "csv")
            .unwrap_err();
        assert!(err.contains("1000 Hz twice"), "{err}");
        let err = match_eq("1k -6", REF, "reference", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "csv")
            .unwrap_err();
        assert!(err.contains("at least 2 bands"), "{err}");
        let err = match_eq(TRACK, REF, "loudness", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "csv")
            .unwrap_err();
        assert!(err.contains("unknown target_curve"), "{err}");
        let err = match_eq(TRACK, REF, "reference", 100.0, 6.0, 0, 1.0, true, 0.0, 0.0, "xml")
            .unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn errors_on_out_of_range_settings() {
        for (bad, needle) in [
            (
                match_eq(TRACK, REF, "reference", 150.0, 6.0, 0, 1.0, true, 0.0, 0.0, "csv"),
                "amount",
            ),
            (
                match_eq(TRACK, REF, "reference", 100.0, 40.0, 0, 1.0, true, 0.0, 0.0, "csv"),
                "max_gain_db",
            ),
            (
                match_eq(TRACK, REF, "reference", 100.0, 6.0, 9, 1.0, true, 0.0, 0.0, "csv"),
                "smoothing",
            ),
            (
                match_eq(TRACK, REF, "reference", 100.0, 6.0, 0, 99.0, true, 0.0, 0.0, "csv"),
                "q",
            ),
            (
                match_eq(TRACK, REF, "reference", 100.0, 6.0, 0, 1.0, true, -99.0, -14.0, "csv"),
                "track_lufs",
            ),
        ] {
            let err = bad.unwrap_err();
            assert!(err.contains(needle), "expected '{needle}' in '{err}'");
        }
    }

    #[test]
    fn enforces_the_band_cap_at_the_boundary() {
        let at_cap: Vec<String> = (1..=MAX_BANDS).map(|i| format!("{} -10", i * 20)).collect();
        let over: Vec<String> = (1..=MAX_BANDS + 1).map(|i| format!("{} -10", i * 20)).collect();
        assert!(match_eq(
            &at_cap.join(","),
            "",
            "flat",
            100.0,
            6.0,
            0,
            1.0,
            true,
            0.0,
            0.0,
            "csv"
        )
        .is_ok());
        let err = match_eq(
            &over.join(","),
            "",
            "flat",
            100.0,
            6.0,
            0,
            1.0,
            true,
            0.0,
            0.0,
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("the limit is 64"), "{err}");
    }
}
