//! clipping-detector core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Decodes a short uncompressed PCM/IEEE-float WAV clip (given as base64 or hex
//! bytes), then scans the decoded PCM for samples that sit at or above a
//! full-scale clipping threshold. It reports how many samples clipped, groups
//! consecutive clipped sample-frames into runs ("regions"), and returns the
//! worst regions with their start/end timestamps so you can see WHERE the
//! distortion is — not just that it exists.
//!
//! Only uncompressed WAV is decoded here (PCM 8/16/24/32-bit integer and
//! 32/64-bit IEEE float). Compressed containers/codecs (MP3, AAC/M4A,
//! Opus/Ogg, FLAC, A-law/mu-law) are rejected with a clear message rather than
//! guessed at — see `parse_wav`.

/// Scan a WAV clip for clipped (full-scale) samples and report the worst regions.
///
/// - `input`: the WAV file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `output`: `"report"` (default, human-readable) or `"json"`.
/// - `threshold`: |sample| at/above this fraction of full scale (0.5–1.0)
///   counts as clipped. 0.99 (default) catches true 0 dBFS peaks with a small
///   guard band; 1.0 flags only exact full-scale samples.
/// - `min_run`: the minimum number of *consecutive* clipped sample-frames that
///   makes a run count as a clipped region (1–1000, default 1). Audacity's
///   classic rule is 3 consecutive full-scale samples.
/// - `gap`: bridge runs separated by at most this many unclipped frames into one
///   region so a single distorted burst isn't fragmented (0–1000, default 0 = no
///   bridging; Audacity's "stop threshold" of 3 corresponds to a gap of 2).
/// - `top_regions`: how many of the worst regions to list (1–100, default 5),
///   ranked by length then peak.
///
/// Returns a user-facing error string for undecodable input, a non-WAV or
/// compressed file, or a malformed WAV.
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    threshold: f64,
    min_run: u32,
    gap: u32,
    top_regions: u32,
) -> Result<String, String> {
    let out_mode = match output.trim() {
        "" | "report" => OutputMode::Report,
        "json" => OutputMode::Json,
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"report\" or \"json\""
            ))
        }
    };
    if !(0.5..=1.0).contains(&threshold) {
        return Err(format!(
            "invalid threshold {threshold}: expected 0.5-1.0 (fraction of full scale)"
        ));
    }
    if !(1..=1000).contains(&min_run) {
        return Err(format!("invalid min_run {min_run}: expected 1-1000"));
    }
    if gap > 1000 {
        return Err(format!("invalid gap {gap}: expected 0-1000"));
    }
    if !(1..=100).contains(&top_regions) {
        return Err(format!("invalid top_regions {top_regions}: expected 1-100"));
    }

    let bytes = decode_bytes(input, input_format)?;
    let wav = parse_wav(&bytes)?;
    let report = detect(&wav, threshold, min_run as usize, gap as usize);

    Ok(match out_mode {
        OutputMode::Report => {
            render_report(&wav, &report, threshold, min_run, gap, top_regions as usize)
        }
        OutputMode::Json => {
            render_json(&wav, &report, threshold, min_run, gap, top_regions as usize)
        }
    })
}

enum OutputMode {
    Report,
    Json,
}

// ---------------------------------------------------------------------------
// WAV decoding (uncompressed PCM / IEEE float only)
// ---------------------------------------------------------------------------

/// Decoded, normalized audio. `samples` are interleaved across channels and
/// scaled to the range [-1.0, 1.0] regardless of the source bit depth.
struct WavData {
    format_name: String,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    /// Interleaved, normalized to [-1.0, 1.0].
    samples: Vec<f32>,
}

impl WavData {
    /// Number of sample frames (one frame = one sample per channel).
    fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }
    fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.sample_rate as f64
        }
    }
}

fn u16_le(b: &[u8], off: usize) -> u16 {
    (b[off] as u16) | ((b[off + 1] as u16) << 8)
}
fn u32_le(b: &[u8], off: usize) -> u32 {
    (b[off] as u32)
        | ((b[off + 1] as u32) << 8)
        | ((b[off + 2] as u32) << 16)
        | ((b[off + 3] as u32) << 24)
}

/// Parse a single RIFF/WAVE file: the 12-byte RIFF header, then the `fmt ` and
/// `data` chunks (other chunks — `LIST`, `fact`, `bext`, … — are skipped).
fn parse_wav(b: &[u8]) -> Result<WavData, String> {
    if b.len() < 12 {
        return Err(format!(
            "not a WAV file: only {} bytes, too short for a RIFF header",
            b.len()
        ));
    }
    if &b[0..4] != b"RIFF" {
        return Err(sniff_container(b));
    }
    if &b[8..12] != b"WAVE" {
        return Err("not a WAV file: RIFF header is not of type WAVE".into());
    }

    let mut pos = 12usize;
    let mut fmt: Option<FmtChunk> = None;
    let mut data: Option<&[u8]> = None;

    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let size = u32_le(b, pos + 4) as usize;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size).min(b.len());
        let body = &b[body_start..body_end];
        match id {
            b"fmt " => fmt = Some(parse_fmt(body)?),
            b"data" => data = Some(body),
            _ => {}
        }
        let advance = 8 + size + (size & 1);
        pos = match pos.checked_add(advance) {
            Some(p) => p,
            None => break,
        };
    }

    let fmt = fmt.ok_or("malformed WAV: no `fmt ` chunk found")?;
    let data = data.ok_or("malformed WAV: no `data` chunk found")?;

    if fmt.channels == 0 {
        return Err("malformed WAV: channel count is 0".into());
    }
    if fmt.sample_rate == 0 {
        return Err("malformed WAV: sample rate is 0".into());
    }

    let (samples, format_name) = decode_samples(&fmt, data)?;
    Ok(WavData {
        format_name,
        sample_rate: fmt.sample_rate,
        channels: fmt.channels,
        bits_per_sample: fmt.bits_per_sample,
        samples,
    })
}

/// Best-effort identification of a non-RIFF file so the error names the codec
/// the user pasted instead of a generic "not a WAV".
fn sniff_container(b: &[u8]) -> String {
    let starts = |sig: &[u8]| b.len() >= sig.len() && &b[..sig.len()] == sig;
    let named = if starts(b"OggS") {
        Some("an Ogg container (Vorbis/Opus)")
    } else if starts(b"fLaC") {
        Some("a FLAC file")
    } else if starts(b"ID3") || (b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0) {
        Some("an MP3 file")
    } else if b.len() >= 12 && &b[4..8] == b"ftyp" {
        Some("an MP4/M4A container (AAC/ALAC)")
    } else if starts(b"FORM") {
        Some("an AIFF file")
    } else {
        None
    };
    match named {
        Some(what) => format!(
            "unsupported format: this looks like {what}, but only uncompressed \
             WAV (RIFF/WAVE) is decoded. Convert it to WAV first."
        ),
        None => "not a WAV file: missing the 'RIFF' signature. Only uncompressed \
                 WAV (RIFF/WAVE) is supported; convert other formats to WAV first."
            .into(),
    }
}

struct FmtChunk {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
}

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_ALAW: u16 = 0x0006;
const WAVE_FORMAT_MULAW: u16 = 0x0007;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

fn parse_fmt(body: &[u8]) -> Result<FmtChunk, String> {
    if body.len() < 16 {
        return Err("malformed WAV: `fmt ` chunk is shorter than 16 bytes".into());
    }
    let mut audio_format = u16_le(body, 0);
    let channels = u16_le(body, 2);
    let sample_rate = u32_le(body, 4);
    let bits_per_sample = u16_le(body, 14);

    // WAVE_FORMAT_EXTENSIBLE stores the real tag in the SubFormat GUID.
    if audio_format == WAVE_FORMAT_EXTENSIBLE && body.len() >= 26 {
        let ext_size = u16_le(body, 16) as usize;
        if ext_size >= 22 && body.len() >= 18 + 22 {
            audio_format = u16_le(body, 24);
        }
    }
    Ok(FmtChunk {
        audio_format,
        channels,
        sample_rate,
        bits_per_sample,
    })
}

fn decode_samples(fmt: &FmtChunk, data: &[u8]) -> Result<(Vec<f32>, String), String> {
    match fmt.audio_format {
        WAVE_FORMAT_PCM => match fmt.bits_per_sample {
            8 => Ok((decode_pcm8(data), "PCM 8-bit integer WAV".into())),
            16 => Ok((decode_pcm16(data), "PCM 16-bit integer WAV".into())),
            24 => Ok((decode_pcm24(data), "PCM 24-bit integer WAV".into())),
            32 => Ok((decode_pcm32(data), "PCM 32-bit integer WAV".into())),
            other => Err(format!(
                "unsupported PCM bit depth: {other}-bit. Supported: 8, 16, 24, 32-bit integer."
            )),
        },
        WAVE_FORMAT_IEEE_FLOAT => match fmt.bits_per_sample {
            32 => Ok((decode_f32(data), "IEEE 32-bit float WAV".into())),
            64 => Ok((decode_f64(data), "IEEE 64-bit float WAV".into())),
            other => Err(format!(
                "unsupported IEEE-float bit depth: {other}-bit. Supported: 32 and 64-bit float."
            )),
        },
        WAVE_FORMAT_ALAW => Err("unsupported format: A-law compressed WAV. \
             Convert to uncompressed PCM WAV first."
            .into()),
        WAVE_FORMAT_MULAW => Err("unsupported format: mu-law compressed WAV. \
             Convert to uncompressed PCM WAV first."
            .into()),
        other => Err(format!(
            "unsupported WAV format tag 0x{other:04x}: only uncompressed PCM \
             (0x0001) and IEEE float (0x0003) are decoded."
        )),
    }
}

fn decode_pcm8(data: &[u8]) -> Vec<f32> {
    // 8-bit WAV PCM is unsigned, centered at 128.
    data.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect()
}
fn decode_pcm16(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}
fn decode_pcm24(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(3)
        .map(|c| {
            let raw = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
            let v = (raw << 8) >> 8; // sign-extend
            v as f32 / 8_388_608.0
        })
        .collect()
}
fn decode_pcm32(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0)
        .collect()
}
fn decode_f32(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
fn decode_f64(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(8)
        .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) as f32)
        .collect()
}

// ---------------------------------------------------------------------------
// Clipping detection
// ---------------------------------------------------------------------------

/// One maximal run of consecutive clipped sample-frames.
struct Region {
    start_frame: usize,
    len_frames: usize,
    peak: f64,
}
impl Region {
    fn end_frame(&self) -> usize {
        self.start_frame + self.len_frames // exclusive
    }
}

struct Report {
    /// Individual interleaved samples (per channel) at/above the threshold.
    clipped_samples: usize,
    total_samples: usize,
    /// Sample-frames with at least one clipped channel.
    clipped_frames: usize,
    total_frames: usize,
    /// Longest run of consecutive clipped frames (any length).
    longest_run_frames: usize,
    /// Highest sample magnitude seen anywhere (fraction of full scale).
    peak: f64,
    /// Maximal runs of consecutive clipped frames with length ≥ min_run,
    /// sorted worst-first (length desc, then peak desc, then earliest).
    regions: Vec<Region>,
}

/// A frame is "clipped" if any of its channel samples reach the threshold;
/// consecutive clipped frames form a run. Runs separated by at most `gap`
/// unclipped frames are merged; `min_run` then filters the merged runs into
/// regions. A region's `len_frames` spans from its first to its last clipped
/// frame (inclusive of any bridged internal gap).
fn detect(wav: &WavData, threshold: f64, min_run: usize, gap: usize) -> Report {
    let ch = wav.channels as usize;
    let total_frames = wav.frames();
    let thr = threshold as f32;

    let mut clipped_samples = 0usize;
    let mut clipped_frames = 0usize;
    let mut peak = 0.0f64;
    let mut longest_run_frames = 0usize;

    let mut regions: Vec<Region> = Vec::new();
    // The currently-open run: its first clipped frame, its last clipped frame,
    // and the highest per-frame peak seen inside it.
    let mut run_start = 0usize;
    let mut run_last = 0usize;
    let mut run_peak = 0.0f64;
    let mut run_open = false;

    let flush =
        |start: usize, last: usize, peak: f64, regions: &mut Vec<Region>, longest: &mut usize| {
            let len = last - start + 1;
            if len > *longest {
                *longest = len;
            }
            if len >= min_run {
                regions.push(Region {
                    start_frame: start,
                    len_frames: len,
                    peak,
                });
            }
        };

    for f in 0..total_frames {
        let base = f * ch;
        let mut frame_clipped = false;
        let mut frame_peak = 0.0f64;
        for c in 0..ch {
            let mag = wav.samples[base + c].abs();
            let magf = mag as f64;
            if magf > peak {
                peak = magf;
            }
            if magf > frame_peak {
                frame_peak = magf;
            }
            if mag >= thr {
                clipped_samples += 1;
                frame_clipped = true;
            }
        }
        if frame_clipped {
            clipped_frames += 1;
            if !run_open {
                run_start = f;
                run_peak = 0.0;
                run_open = true;
            }
            run_last = f;
            if frame_peak > run_peak {
                run_peak = frame_peak;
            }
        } else if run_open && f - run_last > gap {
            // More than `gap` unclipped frames since the last clip: close the run.
            flush(
                run_start,
                run_last,
                run_peak,
                &mut regions,
                &mut longest_run_frames,
            );
            run_open = false;
        }
    }
    // Flush a run that reaches the end of the clip.
    if run_open {
        flush(
            run_start,
            run_last,
            run_peak,
            &mut regions,
            &mut longest_run_frames,
        );
    }

    regions.sort_by(|a, b| {
        b.len_frames
            .cmp(&a.len_frames)
            .then(
                b.peak
                    .partial_cmp(&a.peak)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.start_frame.cmp(&b.start_frame))
    });

    Report {
        clipped_samples,
        total_samples: wav.samples.len(),
        clipped_frames,
        total_frames,
        longest_run_frames,
        peak,
        regions,
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Floor for dBFS readouts so a zero magnitude reports a finite number.
const DBFS_FLOOR: f64 = -120.0;

fn to_dbfs(frac: f64) -> f64 {
    if frac <= 0.0 {
        DBFS_FLOOR
    } else {
        (20.0 * frac.log10()).max(DBFS_FLOOR)
    }
}

/// `M:SS.mmm` (or `H:MM:SS.mmm` past an hour) from a frame index + sample rate.
fn frame_time(frame: usize, sample_rate: u32) -> String {
    if sample_rate == 0 {
        return "0:00.000".into();
    }
    let secs = frame as f64 / sample_rate as f64;
    fmt_time(secs)
}

fn fmt_time(secs: f64) -> String {
    let total_ms = (secs * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let m = (total_s / 60) % 60;
    let h = total_s / 3600;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{ms:03}")
    } else {
        format!("{m}:{s:02}.{ms:03}")
    }
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 / d as f64 * 100.0
    }
}

fn render_report(
    wav: &WavData,
    r: &Report,
    threshold: f64,
    min_run: u32,
    _gap: u32,
    top_regions: usize,
) -> String {
    let sr = wav.sample_rate;
    let mut s = String::new();
    s.push_str("Clipping report\n");
    s.push_str(&format!(
        "Format:        {} ({} Hz, {} ch, {}-bit)\n",
        wav.format_name, sr, wav.channels, wav.bits_per_sample
    ));
    s.push_str(&format!(
        "Duration:      {} ({} frames)\n",
        fmt_time(wav.duration_secs()),
        wav.frames()
    ));
    s.push_str(&format!(
        "Threshold:     {:.4} of full scale ({:.2} dBFS)\n",
        threshold,
        to_dbfs(threshold)
    ));
    s.push_str(&format!(
        "Peak:          {:.4} ({:.2} dBFS)\n",
        r.peak,
        to_dbfs(r.peak)
    ));

    if r.clipped_samples == 0 {
        s.push_str("Result:        No clipping detected.\n");
        return s;
    }

    s.push_str(&format!(
        "Clipped:       {} of {} samples ({:.3}%)\n",
        r.clipped_samples,
        r.total_samples,
        pct(r.clipped_samples, r.total_samples)
    ));
    s.push_str(&format!(
        "Clipped frames:{} of {} ({:.3}%)\n",
        r.clipped_frames,
        r.total_frames,
        pct(r.clipped_frames, r.total_frames)
    ));
    let longest_ms = r.longest_run_frames as f64 / sr.max(1) as f64 * 1000.0;
    s.push_str(&format!(
        "Longest run:   {} frames ({:.1} ms)\n",
        r.longest_run_frames, longest_ms
    ));
    s.push_str(&format!(
        "Regions:       {} run(s) of >= {} consecutive clipped frame(s)\n",
        r.regions.len(),
        min_run
    ));

    if r.regions.is_empty() {
        s.push_str(&format!(
            "               (no run reached the {min_run}-frame minimum)\n"
        ));
        return s;
    }

    let show = r.regions.len().min(top_regions);
    s.push_str(&format!("Worst {show} region(s):\n"));
    for (i, reg) in r.regions.iter().take(show).enumerate() {
        let start = frame_time(reg.start_frame, sr);
        let end = frame_time(reg.end_frame(), sr);
        let dur_ms = reg.len_frames as f64 / sr.max(1) as f64 * 1000.0;
        s.push_str(&format!(
            "  {}. {} - {}  ({} frames, {:.1} ms, peak {:.2} dBFS)\n",
            i + 1,
            start,
            end,
            reg.len_frames,
            dur_ms,
            to_dbfs(reg.peak)
        ));
    }
    s
}

fn render_json(
    wav: &WavData,
    r: &Report,
    threshold: f64,
    min_run: u32,
    gap: u32,
    top_regions: usize,
) -> String {
    let sr = wav.sample_rate;
    let mut s = String::new();
    s.push('{');
    s.push_str(&format!("\"format\":{},", json_str(&wav.format_name)));
    s.push_str(&format!("\"sample_rate\":{sr},"));
    s.push_str(&format!("\"channels\":{},", wav.channels));
    s.push_str(&format!("\"bits_per_sample\":{},", wav.bits_per_sample));
    s.push_str(&format!("\"duration_secs\":{:.6},", wav.duration_secs()));
    s.push_str(&format!("\"frames\":{},", wav.frames()));
    s.push_str(&format!("\"threshold\":{threshold:.6},"));
    s.push_str(&format!("\"threshold_dbfs\":{:.4},", to_dbfs(threshold)));
    s.push_str(&format!("\"min_run\":{min_run},"));
    s.push_str(&format!("\"gap\":{gap},"));
    s.push_str(&format!("\"peak\":{:.6},", r.peak));
    s.push_str(&format!("\"peak_dbfs\":{:.4},", to_dbfs(r.peak)));
    s.push_str(&format!("\"clipped_samples\":{},", r.clipped_samples));
    s.push_str(&format!("\"total_samples\":{},", r.total_samples));
    s.push_str(&format!(
        "\"clipped_pct\":{:.6},",
        pct(r.clipped_samples, r.total_samples)
    ));
    s.push_str(&format!("\"clipped_frames\":{},", r.clipped_frames));
    s.push_str(&format!(
        "\"clipped_frames_pct\":{:.6},",
        pct(r.clipped_frames, r.total_frames)
    ));
    s.push_str(&format!("\"longest_run_frames\":{},", r.longest_run_frames));
    s.push_str(&format!("\"region_count\":{},", r.regions.len()));
    s.push_str("\"regions\":[");
    let show = r.regions.len().min(top_regions);
    for (i, reg) in r.regions.iter().take(show).enumerate() {
        if i > 0 {
            s.push(',');
        }
        let start_secs = reg.start_frame as f64 / sr.max(1) as f64;
        let dur_secs = reg.len_frames as f64 / sr.max(1) as f64;
        s.push('{');
        s.push_str(&format!("\"start_frame\":{},", reg.start_frame));
        s.push_str(&format!("\"end_frame\":{},", reg.end_frame()));
        s.push_str(&format!("\"len_frames\":{},", reg.len_frames));
        s.push_str(&format!("\"start_secs\":{start_secs:.6},"));
        s.push_str(&format!("\"end_secs\":{:.6},", start_secs + dur_secs));
        s.push_str(&format!(
            "\"start_time\":{},",
            json_str(&frame_time(reg.start_frame, sr))
        ));
        s.push_str(&format!(
            "\"end_time\":{},",
            json_str(&frame_time(reg.end_frame(), sr))
        ));
        s.push_str(&format!("\"peak\":{:.6},", reg.peak));
        s.push_str(&format!("\"peak_dbfs\":{:.4}", to_dbfs(reg.peak)));
        s.push('}');
    }
    s.push_str("]}");
    s
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// byte decoding (base64 / hex, hand-rolled, no deps)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the WAV bytes as base64 or hex".into());
    }
    match input_format.trim() {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("invalid hex: odd number of digits".into());
    }
    let bytes = compact.as_bytes();
    let mut out = Vec::with_capacity(compact.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit {:?}", c as char)),
    }
}

/// Standard + URL-safe base64, padding optional.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const INVALID: u8 = 255;
    let val = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => INVALID,
        }
    };
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!("invalid base64 character {:?}", c as char));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PCM16 WAV from interleaved f32 samples in [-1, 1].
    fn make_wav(sample_rate: u32, channels: u16, samples: &[f32]) -> Vec<u8> {
        let bits = 16u16;
        let block_align = channels * bits / 8;
        let byte_rate = sample_rate * block_align as u32;
        let data_len = samples.len() * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        b.extend_from_slice(b"WAVE");
        b.extend_from_slice(b"fmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // PCM
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&sample_rate.to_le_bytes());
        b.extend_from_slice(&byte_rate.to_le_bytes());
        b.extend_from_slice(&block_align.to_le_bytes());
        b.extend_from_slice(&bits.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data_len as u32).to_le_bytes());
        for &s in samples {
            let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            b.extend_from_slice(&v.to_le_bytes());
        }
        b
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn detects_a_clipped_run_with_timestamps() {
        // 8 frames at 8 Hz mono: frames 2,3,4 are full-scale (a 3-frame run).
        let samples = [0.1, 0.2, 1.0, 1.0, 1.0, 0.3, 0.1, 0.0];
        let wav = make_wav(8, 1, &samples);
        let out = run(&hex(&wav), "hex", "report", 0.99, 1, 0, 5).unwrap();
        assert!(out.contains("Clipped:       3 of 8 samples"), "{out}");
        assert!(out.contains("Longest run:   3 frames"), "{out}");
        assert!(out.contains("Regions:       1 run(s)"), "{out}");
        // frame 2 -> 0.25 s, frame 5 (exclusive end) -> 0.625 s
        assert!(out.contains("0:00.250 - 0:00.625"), "{out}");
    }

    #[test]
    fn no_clipping_reports_clean() {
        let samples = [0.1, 0.2, 0.3, -0.4];
        let wav = make_wav(8, 1, &samples);
        let out = run(&hex(&wav), "hex", "report", 0.99, 1, 0, 5).unwrap();
        assert!(out.contains("No clipping detected"), "{out}");
    }

    #[test]
    fn min_run_filters_short_runs() {
        // one isolated clipped frame + a 2-frame run.
        let samples = [1.0, 0.1, 0.1, 1.0, 1.0, 0.1];
        let wav = make_wav(8, 1, &samples);
        // min_run = 2 keeps only the 2-frame run as a region.
        let out = run(&hex(&wav), "hex", "json", 0.99, 2, 0, 5).unwrap();
        assert!(out.contains("\"region_count\":1"), "{out}");
        // but all 3 clipped samples still count.
        assert!(out.contains("\"clipped_samples\":3"), "{out}");
        assert!(out.contains("\"longest_run_frames\":2"), "{out}");
    }

    #[test]
    fn json_lists_worst_region_first() {
        // a 1-frame run early, a 3-frame run later: 3-frame must sort first.
        let samples = [1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0];
        let wav = make_wav(8, 1, &samples);
        let out = run(&hex(&wav), "hex", "json", 0.99, 1, 0, 5).unwrap();
        let first = out.find("\"len_frames\":3").unwrap();
        let second = out.find("\"len_frames\":1").unwrap();
        assert!(
            first < second,
            "worst (longest) region should be listed first: {out}"
        );
    }

    #[test]
    fn stereo_counts_per_channel_samples_but_frame_timestamps() {
        // 2 frames stereo; frame 0 has one clipped channel, frame 1 both.
        let samples = [1.0, 0.1, 1.0, 1.0];
        let wav = make_wav(8, 2, &samples);
        let out = run(&hex(&wav), "hex", "json", 0.99, 1, 0, 5).unwrap();
        assert!(out.contains("\"clipped_samples\":3"), "{out}");
        assert!(out.contains("\"clipped_frames\":2"), "{out}");
        assert!(out.contains("\"channels\":2"), "{out}");
    }

    #[test]
    fn gap_bridges_split_runs_into_one_region() {
        // two clipped frames separated by one clean frame: [1, 0, 1] then clean.
        let samples = [1.0, 0.0, 1.0, 0.1, 0.1];
        let wav = make_wav(8, 1, &samples);
        // gap = 0: two separate 1-frame runs.
        let out0 = run(&hex(&wav), "hex", "json", 0.99, 1, 0, 5).unwrap();
        assert!(out0.contains("\"region_count\":2"), "{out0}");
        // gap = 1: the single clean frame is bridged into one 3-frame region.
        let out1 = run(&hex(&wav), "hex", "json", 0.99, 1, 1, 5).unwrap();
        assert!(out1.contains("\"region_count\":1"), "{out1}");
        assert!(out1.contains("\"len_frames\":3"), "{out1}");
    }

    #[test]
    fn rejects_out_of_range_threshold() {
        let wav = make_wav(8, 1, &[0.1]);
        let err = run(&hex(&wav), "hex", "report", 0.3, 1, 0, 5).unwrap_err();
        assert!(err.contains("invalid threshold"), "{err}");
    }

    #[test]
    fn rejects_compressed_input() {
        // an ID3-tagged MP3 header as hex (full 10-byte ID3 header + padding).
        let err = run("494433030000000000000000", "hex", "report", 0.99, 1, 0, 5).unwrap_err();
        assert!(err.contains("MP3"), "{err}");
    }

    #[test]
    fn base64_input_works() {
        let samples = [1.0, 1.0, 1.0, 0.0];
        let wav = make_wav(8, 1, &samples);
        let b64 = base64_encode(&wav);
        let out = run(&b64, "base64", "report", 0.99, 1, 0, 5).unwrap();
        assert!(out.contains("Longest run:   3 frames"), "{out}");
    }

    fn base64_encode(data: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            out.push(T[(b[0] >> 2) as usize] as char);
            out.push(T[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
            if chunk.len() > 1 {
                out.push(T[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(T[(b[2] & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }
}
