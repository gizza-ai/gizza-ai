//! gizza-ai/ffmpeg-filtergraph-builder core — compile an ordered list of described
//! filter steps into a validated ffmpeg filtergraph string.
//!
//! Pure text → text. **Nothing is executed**: this crate only *composes and
//! validates* the string a user would paste into their own ffmpeg invocation. No
//! process is spawned, no media is read, and no user-supplied filter ever runs
//! here or on the page.
//!
//! Safety rules that shape the code below:
//! * `drawtext` text is emitted with `expansion=none` and rejects `'`, `\` and
//!   control characters, so neither filtergraph escaping nor drawtext's `%{…}`
//!   expansion can be smuggled through.
//! * The `command` output form validates file names against a strict allowlist,
//!   so shell metacharacters can never be interpolated into a command line.
//! * The assembled graph is re-validated (balanced quotes/brackets, no stray
//!   `;`, no control characters) before it is returned.

/// Which stream family the steps apply to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stream {
    Video,
    Audio,
}

impl Stream {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "video" | "v" => Ok(Stream::Video),
            "audio" | "a" => Ok(Stream::Audio),
            other => Err(format!(
                "stream: expected 'video' or 'audio', got '{other}'"
            )),
        }
    }

    fn default_label(self) -> &'static str {
        match self {
            Stream::Video => "0:v",
            Stream::Audio => "0:a",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Stream::Video => "video",
            Stream::Audio => "audio",
        }
    }
}

/// Which string shape to emit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputForm {
    /// `[0:v]scale=-2:720,hue=s=0[out]` — for `-filter_complex`.
    FilterComplex,
    /// `scale=-2:720,hue=s=0` — for `-vf` / `-af`.
    FilterChain,
    /// A complete `ffmpeg -i … -filter_complex "…" -map "[out]" …` line.
    Command,
}

impl OutputForm {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "filter_complex" => Ok(OutputForm::FilterComplex),
            "filter_chain" => Ok(OutputForm::FilterChain),
            "command" => Ok(OutputForm::Command),
            other => Err(format!(
                "output: expected 'filter_complex', 'filter_chain' or 'command', got '{other}'"
            )),
        }
    }
}

/// Maximum number of steps in one graph (a filtergraph longer than this is a
/// script, not a form field).
pub const MAX_STEPS: usize = 30;
/// Maximum size of the `steps` text.
pub const MAX_STEPS_CHARS: usize = 8_000;

/// Everything except the step list.
#[derive(Clone, Debug)]
pub struct Options {
    pub stream: Stream,
    pub output: OutputForm,
    /// Source pad label, e.g. `0:v`. Empty or `auto` → `0:v` / `0:a` by stream.
    pub input_label: String,
    /// Sink pad label, e.g. `out`.
    pub output_label: String,
    /// Input file used by the `command` form.
    pub input_file: String,
    /// Output file used by the `command` form.
    pub output_file: String,
    /// Append a `#` breakdown of what each step compiled to.
    pub explain: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            stream: Stream::Video,
            output: OutputForm::FilterComplex,
            input_label: "auto".into(),
            output_label: "out".into(),
            input_file: "input.mp4".into(),
            output_file: "output.mp4".into(),
            explain: false,
        }
    }
}

/// One compiled step: the text the user wrote and the filters it produced.
struct Compiled {
    source: String,
    filters: Vec<String>,
}

/// Every step keyword the builder models, per stream, for error messages and docs.
pub const VIDEO_STEPS: &[&str] = &[
    "blur",
    "brightness",
    "contrast",
    "crop",
    "fade",
    "flip",
    "fps",
    "grayscale",
    "hue",
    "pad",
    "raw",
    "reverse",
    "rotate",
    "saturation",
    "scale",
    "sharpen",
    "speed",
    "text",
    "trim",
];
pub const AUDIO_STEPS: &[&str] = &[
    "fade", "highpass", "lowpass", "mono", "normalize", "raw", "reverse", "speed", "trim",
    "volume",
];

/// Build the filtergraph from `steps` (one step per line) and string-valued options —
/// the single entry point shared by the chat/CLI block and the browser page.
#[allow(clippy::too_many_arguments)]
pub fn build_from_strs(
    steps: &str,
    stream: &str,
    output: &str,
    input_label: &str,
    output_label: &str,
    input_file: &str,
    output_file: &str,
    explain: bool,
) -> Result<String, String> {
    let opts = Options {
        stream: Stream::parse(stream)?,
        output: OutputForm::parse(output)?,
        input_label: input_label.to_string(),
        output_label: output_label.to_string(),
        input_file: input_file.to_string(),
        output_file: output_file.to_string(),
        explain,
    };
    build(steps, &opts)
}

/// Compile `steps` into the requested filtergraph string.
pub fn build(steps: &str, opts: &Options) -> Result<String, String> {
    if steps.chars().count() > MAX_STEPS_CHARS {
        return Err(format!(
            "steps: too long — {} characters, the limit is {MAX_STEPS_CHARS}",
            steps.chars().count()
        ));
    }

    let lines = split_steps(steps);
    if lines.is_empty() {
        return Err(
            "steps: expected at least one filter step, one per line (e.g. 'scale to 720p'), got nothing"
                .into(),
        );
    }
    if lines.len() > MAX_STEPS {
        return Err(format!(
            "steps: too many steps — {} given, the limit is {MAX_STEPS}",
            lines.len()
        ));
    }

    let mut compiled: Vec<Compiled> = Vec::with_capacity(lines.len());
    for (idx, raw) in lines.iter().enumerate() {
        let filters = compile_step(raw, opts.stream)
            .map_err(|e| format!("step {} ('{}'): {e}", idx + 1, raw))?;
        compiled.push(Compiled {
            source: raw.clone(),
            filters,
        });
    }

    let chain: Vec<String> = compiled
        .iter()
        .flat_map(|c| c.filters.iter().cloned())
        .collect();
    let chain = chain.join(",");
    validate_graph(&chain)?;

    let in_label = resolve_label(&opts.input_label, opts.stream)?;
    let out_label = {
        let l = opts.output_label.trim();
        let l = if l.is_empty() { "out" } else { l };
        check_label(l)?;
        l.to_string()
    };

    let mut result = match opts.output {
        OutputForm::FilterChain => chain.clone(),
        OutputForm::FilterComplex => format!("[{in_label}]{chain}[{out_label}]"),
        OutputForm::Command => {
            let infile = check_path(&opts.input_file, "input_file", "input.mp4")?;
            let outfile = check_path(&opts.output_file, "output_file", "output.mp4")?;
            let graph = format!("[{in_label}]{chain}[{out_label}]");
            let passthrough = match opts.stream {
                Stream::Video => "0:a?",
                Stream::Audio => "0:v?",
            };
            let (first, second) = match opts.stream {
                Stream::Video => (format!("[{out_label}]"), passthrough.to_string()),
                Stream::Audio => (passthrough.to_string(), format!("[{out_label}]")),
            };
            format!(
                "ffmpeg -i {infile} -filter_complex \"{graph}\" -map \"{first}\" -map \"{second}\" {outfile}"
            )
        }
    };

    if opts.explain {
        result.push_str("\n\n# How each step compiled:");
        for (i, c) in compiled.iter().enumerate() {
            result.push_str(&format!(
                "\n# {}. {} → {}",
                i + 1,
                c.source,
                c.filters.join(",")
            ));
        }
    }
    Ok(result)
}

/// Split the step text into individual steps. Steps are separated by newlines or
/// `;`, and may be written as a "then" chain; list markers and filler words are
/// stripped so a pasted recipe works as-is.
fn split_steps(steps: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in steps.split(['\n', '\r', ';']) {
        for piece in split_then(chunk) {
            let cleaned = clean_step(&piece);
            if !cleaned.is_empty() {
                out.push(cleaned);
            }
        }
    }
    out
}

/// Split a chunk on the connector words `, then` / ` then ` / `, and then`.
fn split_then(chunk: &str) -> Vec<String> {
    let lower = chunk.to_ascii_lowercase();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        // Only split on a whole word "then" that is not inside a quoted string.
        if lower[i..].starts_with("then")
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
            && (i + 4 == bytes.len() || !bytes[i + 4].is_ascii_alphanumeric())
            && !inside_quotes(&chunk[..i])
        {
            parts.push(chunk[start..i].to_string());
            start = i + 4;
            i += 4;
            continue;
        }
        i += 1;
    }
    parts.push(chunk[start..].to_string());
    parts
}

fn inside_quotes(prefix: &str) -> bool {
    let mut double = false;
    let mut single = false;
    for c in prefix.chars() {
        match c {
            '"' if !single => double = !double,
            '\'' if !double => single = !single,
            _ => {}
        }
    }
    double || single
}

/// Strip list markers, connector words and trailing punctuation from one step.
fn clean_step(step: &str) -> String {
    let mut s = step.trim();
    // list markers: "-", "*", "•", "1.", "2)"
    loop {
        let before = s;
        s = s.trim_start_matches(['-', '*', '•', '.', ')', ',']).trim();
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let rest = &s[digits.len()..];
            if rest.starts_with('.') || rest.starts_with(')') {
                s = rest[1..].trim();
            }
        }
        if s == before {
            break;
        }
    }
    // connector words at the front
    loop {
        let lower = s.to_ascii_lowercase();
        let mut trimmed = false;
        for w in ["and ", "next ", "finally ", "after that ", "also "] {
            if lower.starts_with(w) {
                s = s[w.len()..].trim();
                trimmed = true;
                break;
            }
        }
        if !trimmed {
            break;
        }
    }
    s.trim_end_matches(['.', ',', ';']).trim().to_string()
}

/// A token from a step's argument list; `quoted` marks a `"…"` / `'…'` literal.
struct Token {
    text: String,
    quoted: bool,
}

fn tokenize(args: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut chars = args.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            chars.next();
            let mut buf = String::new();
            let mut closed = false;
            for ch in chars.by_ref() {
                if ch == quote {
                    closed = true;
                    break;
                }
                buf.push(ch);
            }
            if !closed {
                return Err(format!("unterminated {quote} quote"));
            }
            out.push(Token {
                text: buf,
                quoted: true,
            });
        } else {
            let mut buf = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                buf.push(ch);
                chars.next();
            }
            out.push(Token {
                text: buf,
                quoted: false,
            });
        }
    }
    Ok(out)
}

/// Words that carry no meaning between a keyword and its value.
fn is_filler(t: &str) -> bool {
    matches!(
        t.to_ascii_lowercase().as_str(),
        "to" | "the" | "a" | "an" | "of" | "by" | "into" | "it" | "=" | "with" | "using"
    )
}

fn drop_filler(tokens: &[Token]) -> Vec<&Token> {
    tokens.iter().filter(|t| t.quoted || !is_filler(&t.text)).collect()
}

/// Compile one cleaned step into the ffmpeg filters it represents.
fn compile_step(step: &str, stream: Stream) -> Result<Vec<String>, String> {
    let tokens = tokenize(step)?;
    if tokens.is_empty() {
        return Err("expected a filter step".into());
    }
    let op = tokens[0].text.to_ascii_lowercase();
    let rest: Vec<&Token> = drop_filler(&tokens[1..]);
    let words: Vec<String> = rest.iter().map(|t| t.text.to_ascii_lowercase()).collect();

    // `raw` keeps the remainder verbatim (validated, never executed).
    if op == "raw" || op == "filter" {
        let literal = step[op.len()..].trim();
        return compile_raw(literal).map(|f| vec![f]);
    }

    let canon = canonical_op(&op, &words).ok_or_else(|| unknown_step_err(&op, stream))?;

    let allowed = match stream {
        Stream::Video => VIDEO_STEPS,
        Stream::Audio => AUDIO_STEPS,
    };
    if !allowed.contains(&canon) {
        let other = match stream {
            Stream::Video => "audio",
            Stream::Audio => "video",
        };
        return Err(format!(
            "'{canon}' is a {other} step but stream is {} — set stream to {other}, or use one of: {}",
            stream.name(),
            allowed.join(", ")
        ));
    }

    match canon {
        "scale" => compile_scale(&rest),
        "crop" => compile_crop(&rest),
        "pad" => compile_pad(&rest),
        "fade" => compile_fade(&rest, stream),
        "rotate" => compile_rotate(&rest),
        "flip" => compile_flip(&rest),
        "grayscale" => Ok(vec!["hue=s=0".into()]),
        "blur" => compile_blur(&rest),
        "sharpen" => compile_sharpen(&rest),
        "fps" => compile_fps(&rest),
        "speed" => compile_speed(&rest, stream),
        "trim" => compile_trim(&rest, stream),
        "reverse" => Ok(vec![match stream {
            Stream::Video => "reverse".into(),
            Stream::Audio => "areverse".into(),
        }]),
        "brightness" => compile_eq("brightness", &rest, -1.0, 1.0),
        "contrast" => compile_eq("contrast", &rest, -2.0, 2.0),
        "saturation" => compile_eq("saturation", &rest, 0.0, 3.0),
        "hue" => compile_hue(&rest),
        "text" => compile_text(&rest),
        "volume" => compile_volume(&rest),
        "normalize" => Ok(vec!["loudnorm=I=-16:TP=-1.5:LRA=11".into()]),
        "mono" => Ok(vec!["aformat=channel_layouts=mono".into()]),
        "highpass" => compile_pass("highpass", &rest, 200.0),
        "lowpass" => compile_pass("lowpass", &rest, 3000.0),
        other => Err(unknown_step_err(other, stream)),
    }
}

/// Map a written keyword (plus, for a few, its first argument) onto a canonical step.
fn canonical_op(op: &str, words: &[String]) -> Option<&'static str> {
    let first = words.first().map(|s| s.as_str()).unwrap_or("");
    Some(match op {
        "scale" | "resize" | "size" => "scale",
        "crop" => "crop",
        "pad" | "letterbox" | "pillarbox" => "pad",
        "fade" | "fadein" | "fadeout" => "fade",
        "rotate" | "rotation" | "transpose" => "rotate",
        "flip" | "mirror" | "hflip" | "vflip" => "flip",
        "grayscale" | "greyscale" | "gray" | "grey" | "desaturate" | "monochrome" | "bw" => {
            "grayscale"
        }
        "blur" | "gblur" | "soften" => "blur",
        "sharpen" | "unsharp" => "sharpen",
        "fps" | "framerate" | "frames" => "fps",
        "speed" | "speedup" | "tempo" | "slow" | "faster" | "slower" => "speed",
        "trim" | "cut" | "clip" => "trim",
        "reverse" => "reverse",
        "brightness" | "brighten" => "brightness",
        "contrast" => "contrast",
        "saturation" | "saturate" => "saturation",
        "hue" => "hue",
        "text" | "drawtext" | "caption" | "label" | "watermark" => "text",
        "volume" | "gain" | "loudness" => "volume",
        "normalize" | "loudnorm" => "normalize",
        "mono" | "downmix" => "mono",
        "highpass" | "high-pass" => "highpass",
        "lowpass" | "low-pass" => "lowpass",
        // "black and white", "make it square"
        "make" | "convert" | "add" | "apply" | "set" => return canonical_op(first, &words[1..]),
        "black" if first == "and" => "grayscale",
        _ => return None,
    })
}

fn unknown_step_err(op: &str, stream: Stream) -> String {
    let allowed = match stream {
        Stream::Video => VIDEO_STEPS,
        Stream::Audio => AUDIO_STEPS,
    };
    format!(
        "unknown {} step '{op}' — supported steps are: {}",
        stream.name(),
        allowed.join(", ")
    )
}

// ---------------------------------------------------------------- value parsing

/// Format a number without a trailing `.0`, and without scientific notation.
fn num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn parse_number(tok: &str, what: &str) -> Result<f64, String> {
    tok.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("expected a number for {what}, got '{tok}'"))
}

/// Parse a duration: `2`, `2s`, `1.5sec`, `500ms`.
fn parse_duration(tok: &str, what: &str) -> Result<f64, String> {
    let t = tok.trim().to_ascii_lowercase();
    let (numpart, mult) = if let Some(v) = t.strip_suffix("ms") {
        (v, 0.001)
    } else if let Some(v) = t.strip_suffix("seconds") {
        (v, 1.0)
    } else if let Some(v) = t.strip_suffix("second") {
        (v, 1.0)
    } else if let Some(v) = t.strip_suffix("sec") {
        (v, 1.0)
    } else if let Some(v) = t.strip_suffix('s') {
        (v, 1.0)
    } else {
        (t.as_str(), 1.0)
    };
    let v = numpart
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| {
            format!("expected {what} in seconds (e.g. 1, 1.5s, 500ms), got '{tok}'")
        })?;
    Ok(v * mult)
}

/// A width/height preset (`720p`, `4k`, `vga`) → (width or -2, height).
fn preset_size(tok: &str) -> Option<(f64, f64)> {
    Some(match tok.trim().to_ascii_lowercase().as_str() {
        "240p" => (-2.0, 240.0),
        "360p" => (-2.0, 360.0),
        "480p" | "sd" => (-2.0, 480.0),
        "576p" => (-2.0, 576.0),
        "720p" | "hd" => (-2.0, 720.0),
        "1080p" | "fhd" | "fullhd" => (-2.0, 1080.0),
        "1440p" | "2k" | "qhd" => (-2.0, 1440.0),
        "2160p" | "4k" | "uhd" => (-2.0, 2160.0),
        "4320p" | "8k" => (-2.0, 4320.0),
        "vga" => (640.0, 480.0),
        "qvga" => (320.0, 240.0),
        _ => return None,
    })
}

/// `1280x720`, `1280X720`, `1280×720` → (1280, 720). Negative auto values allowed.
fn parse_dims(tok: &str) -> Option<(f64, f64)> {
    let lower = tok.to_ascii_lowercase();
    let parts: Vec<&str> = lower.split(['x', '×']).collect();
    if parts.len() != 2 {
        return None;
    }
    let w = parts[0].trim().parse::<f64>().ok()?;
    let h = parts[1].trim().parse::<f64>().ok()?;
    if !w.is_finite() || !h.is_finite() {
        return None;
    }
    Some((w, h))
}

/// `16:9` → (16, 9).
fn parse_aspect(tok: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = tok.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let a = parts[0].trim().parse::<f64>().ok()?;
    let b = parts[1].trim().parse::<f64>().ok()?;
    if !(a.is_finite() && b.is_finite()) || a <= 0.0 || b <= 0.0 {
        return None;
    }
    Some((a, b))
}

/// `50%` → 0.5.
fn parse_percent(tok: &str) -> Option<f64> {
    let v = tok.strip_suffix('%')?.trim().parse::<f64>().ok()?;
    if v > 0.0 && v.is_finite() {
        Some(v / 100.0)
    } else {
        None
    }
}

/// Validate a color: a name, `#rgb`/`#rrggbb`/`#rrggbbaa`, or either with `@0.5` alpha.
fn check_color(c: &str) -> Result<String, String> {
    let (base, alpha) = match c.split_once('@') {
        Some((b, a)) => (b, Some(a)),
        None => (c, None),
    };
    let ok_base = if let Some(hex) = base.strip_prefix('#') {
        matches!(hex.len(), 3 | 6 | 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit())
    } else {
        !base.is_empty()
            && base.len() <= 32
            && base.chars().all(|ch| ch.is_ascii_alphanumeric())
    };
    if !ok_base {
        return Err(format!(
            "expected a color name (e.g. black) or hex (#000, #ff0000), got '{c}'"
        ));
    }
    if let Some(a) = alpha {
        let v = a.parse::<f64>().ok().filter(|v| (0.0..=1.0).contains(v));
        if v.is_none() {
            return Err(format!("expected an alpha between 0 and 1 after '@', got '{a}'"));
        }
    }
    Ok(c.to_string())
}

// ---------------------------------------------------------------- step compilers

fn compile_scale(rest: &[&Token]) -> Result<Vec<String>, String> {
    if rest.is_empty() {
        return Err(
            "expected a size — e.g. '1280x720', '720p', 'width 1280', 'height 720' or '50%'".into(),
        );
    }
    let first = rest[0].text.to_ascii_lowercase();
    if matches!(first.as_str(), "width" | "w") {
        let v = rest
            .get(1)
            .ok_or("expected a width in pixels after 'width'")?;
        let w = parse_number(&v.text, "width")?;
        return Ok(vec![format!("scale={}:-2", num(w))]);
    }
    if matches!(first.as_str(), "height" | "h") {
        let v = rest
            .get(1)
            .ok_or("expected a height in pixels after 'height'")?;
        let h = parse_number(&v.text, "height")?;
        return Ok(vec![format!("scale=-2:{}", num(h))]);
    }
    if let Some(f) = parse_percent(&first) {
        return Ok(vec![format!(
            "scale=trunc(iw*{f}/2)*2:trunc(ih*{f}/2)*2",
            f = num(f)
        )]);
    }
    if let Some((w, h)) = preset_size(&first) {
        return Ok(vec![format!("scale={}:{}", num(w), num(h))]);
    }
    let dims = parse_dims(&first).or_else(|| {
        // `1280:720` is also accepted for scale (it has no aspect-ratio meaning).
        parse_aspect(&first).filter(|(a, b)| *a > 16.0 || *b > 16.0)
    });
    if let Some((w, h)) = dims {
        return Ok(vec![format!("scale={}:{}", num(w), num(h))]);
    }
    Err(format!(
        "expected a size like '1280x720', a preset like '720p', 'width 1280', 'height 720' or '50%', got '{}'",
        rest[0].text
    ))
}

fn compile_crop(rest: &[&Token]) -> Result<Vec<String>, String> {
    if rest.is_empty() {
        return Err("expected a crop size — e.g. '640x640', 'square', '16:9' or '80%'".into());
    }
    let first = rest[0].text.to_ascii_lowercase();
    if first == "square" || first == "1:1" {
        return Ok(vec!["crop='min(iw,ih)':'min(iw,ih)'".into()]);
    }
    if let Some(f) = parse_percent(&first) {
        return Ok(vec![format!(
            "crop=trunc(iw*{f}/2)*2:trunc(ih*{f}/2)*2",
            f = num(f)
        )]);
    }
    if let Some((w, h)) = parse_dims(&first) {
        if w <= 0.0 || h <= 0.0 {
            return Err(format!(
                "crop needs positive pixel dimensions, got '{}'",
                rest[0].text
            ));
        }
        return Ok(vec![format!("crop={}:{}", num(w), num(h))]);
    }
    if let Some((a, b)) = parse_aspect(&first) {
        return Ok(vec![format!(
            "crop='min(iw,ih*{a}/{b})':'min(ih,iw*{b}/{a})'",
            a = num(a),
            b = num(b)
        )]);
    }
    Err(format!(
        "expected 'square', pixels like '640x640', an aspect ratio like '16:9', or '80%', got '{}'",
        rest[0].text
    ))
}

fn compile_pad(rest: &[&Token]) -> Result<Vec<String>, String> {
    if rest.is_empty() {
        return Err("expected a pad size — e.g. '1920x1080' or '16:9' (optionally followed by a color)".into());
    }
    let first = rest[0].text.to_ascii_lowercase();
    let color = match rest.get(1) {
        Some(t) => check_color(&t.text)?,
        None => "black".to_string(),
    };
    if let Some((w, h)) = parse_dims(&first) {
        if w <= 0.0 || h <= 0.0 {
            return Err(format!(
                "pad needs positive pixel dimensions, got '{}'",
                rest[0].text
            ));
        }
        return Ok(vec![format!(
            "pad={}:{}:(ow-iw)/2:(oh-ih)/2:{color}",
            num(w),
            num(h)
        )]);
    }
    if let Some((a, b)) = parse_aspect(&first) {
        return Ok(vec![format!(
            "pad='max(iw,ih*{a}/{b})':'max(ih,iw*{b}/{a})':'(ow-iw)/2':'(oh-ih)/2':{color}",
            a = num(a),
            b = num(b)
        )]);
    }
    Err(format!(
        "expected pixels like '1920x1080' or an aspect ratio like '16:9', got '{}'",
        rest[0].text
    ))
}

fn compile_fade(rest: &[&Token], stream: Stream) -> Result<Vec<String>, String> {
    let filter = match stream {
        Stream::Video => "fade",
        Stream::Audio => "afade",
    };
    let mut dir: Option<&str> = None;
    let mut dur: Option<f64> = None;
    let mut start: Option<f64> = None;
    let mut i = 0usize;
    while i < rest.len() {
        let w = rest[i].text.to_ascii_lowercase();
        match w.as_str() {
            "in" => dir = Some("in"),
            "out" => dir = Some("out"),
            "at" | "from" | "starting" | "start" => {
                let v = rest
                    .get(i + 1)
                    .ok_or("expected a start time in seconds after 'at'")?;
                start = Some(parse_duration(&v.text, "start time")?);
                i += 1;
            }
            "over" | "for" | "lasting" | "duration" => {}
            _ => {
                let v = parse_duration(&w, "fade duration")?;
                if dur.is_none() {
                    dur = Some(v);
                } else if start.is_none() {
                    start = Some(v);
                }
            }
        }
        i += 1;
    }
    let dir = dir.ok_or("expected a direction — 'fade in' or 'fade out'")?;
    let dur = dur.unwrap_or(1.0);
    if dur <= 0.0 {
        return Err("fade duration must be greater than 0 seconds".into());
    }
    let start = start.unwrap_or(0.0);
    Ok(vec![format!(
        "{filter}=t={dir}:st={}:d={}",
        num(start),
        num(dur)
    )])
}

fn compile_rotate(rest: &[&Token]) -> Result<Vec<String>, String> {
    let tok = rest
        .first()
        .ok_or("expected an angle — 90, 180 or 270 degrees")?;
    let cleaned = tok
        .text
        .to_ascii_lowercase()
        .replace("degrees", "")
        .replace("degree", "")
        .replace("deg", "")
        .replace('°', "");
    let deg = parse_number(cleaned.trim(), "rotation angle")?;
    let norm = ((deg % 360.0) + 360.0) % 360.0;
    match norm as i64 {
        0 => Err("a rotation of 0 degrees does nothing — use 90, 180 or 270".into()),
        90 => Ok(vec!["transpose=1".into()]),
        180 => Ok(vec!["transpose=1".into(), "transpose=1".into()]),
        270 => Ok(vec!["transpose=2".into()]),
        _ => Err(format!(
            "expected 90, 180 or 270 degrees (ffmpeg's transpose steps), got '{}'",
            tok.text
        )),
    }
}

fn compile_flip(rest: &[&Token]) -> Result<Vec<String>, String> {
    let tok = rest
        .first()
        .map(|t| t.text.to_ascii_lowercase())
        .unwrap_or_default();
    match tok.as_str() {
        "horizontal" | "horizontally" | "h" | "x" | "left-right" => Ok(vec!["hflip".into()]),
        "vertical" | "vertically" | "v" | "y" | "up-down" => Ok(vec!["vflip".into()]),
        "" => Err("expected a direction — 'flip horizontal' or 'flip vertical'".into()),
        other => Err(format!(
            "expected 'horizontal' or 'vertical', got '{other}'"
        )),
    }
}

fn compile_blur(rest: &[&Token]) -> Result<Vec<String>, String> {
    let sigma = match rest.first() {
        Some(t) => parse_number(&t.text, "blur strength")?,
        None => 5.0,
    };
    if !(0.0..=100.0).contains(&sigma) {
        return Err(format!(
            "blur strength must be between 0 and 100 (gblur sigma), got '{}'",
            num(sigma)
        ));
    }
    Ok(vec![format!("gblur=sigma={}", num(sigma))])
}

fn compile_sharpen(rest: &[&Token]) -> Result<Vec<String>, String> {
    let amount = match rest.first() {
        Some(t) => parse_number(&t.text, "sharpen amount")?,
        None => 1.0,
    };
    if !(-2.0..=5.0).contains(&amount) {
        return Err(format!(
            "sharpen amount must be between -2 and 5 (unsharp luma_amount), got '{}'",
            num(amount)
        ));
    }
    Ok(vec![format!("unsharp=5:5:{}:5:5:0", num(amount))])
}

fn compile_fps(rest: &[&Token]) -> Result<Vec<String>, String> {
    let tok = rest
        .first()
        .ok_or("expected a frame rate — e.g. 'fps 30'")?;
    let v = parse_number(&tok.text, "frame rate")?;
    if !(v > 0.0 && v <= 1000.0) {
        return Err(format!(
            "frame rate must be between 0 and 1000, got '{}'",
            tok.text
        ));
    }
    Ok(vec![format!("fps={}", num(v))])
}

fn compile_speed(rest: &[&Token], stream: Stream) -> Result<Vec<String>, String> {
    let tok = rest
        .first()
        .ok_or("expected a speed factor — e.g. 'speed 2x' or 'speed 0.5'")?;
    let cleaned = tok.text.to_ascii_lowercase();
    let cleaned = cleaned.trim_end_matches('x');
    let f = parse_number(cleaned, "speed factor")?;
    if !(0.01..=100.0).contains(&f) {
        return Err(format!(
            "speed factor must be between 0.01 and 100, got '{}'",
            tok.text
        ));
    }
    match stream {
        Stream::Video => Ok(vec![format!("setpts={}*PTS", num(1.0 / f))]),
        Stream::Audio => Ok(atempo_chain(f)),
    }
}

/// `atempo` is only defined for 0.5–2.0 per instance, so a bigger change is a
/// chain of instances whose factors multiply back to `f`.
fn atempo_chain(f: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut remaining = f;
    while remaining > 2.0 {
        out.push("atempo=2".to_string());
        remaining /= 2.0;
    }
    while remaining < 0.5 {
        out.push("atempo=0.5".to_string());
        remaining /= 0.5;
    }
    if (remaining - 1.0).abs() > 1e-9 || out.is_empty() {
        out.push(format!("atempo={}", num(remaining)));
    }
    out
}

fn compile_trim(rest: &[&Token], stream: Stream) -> Result<Vec<String>, String> {
    let mut start: Option<f64> = None;
    let mut end: Option<f64> = None;
    let mut i = 0usize;
    while i < rest.len() {
        let w = rest[i].text.to_ascii_lowercase();
        match w.as_str() {
            "from" | "start" | "at" => {
                let v = rest.get(i + 1).ok_or("expected a start time after 'from'")?;
                start = Some(parse_duration(&v.text, "start time")?);
                i += 1;
            }
            "to" | "until" | "end" => {
                let v = rest.get(i + 1).ok_or("expected an end time after 'to'")?;
                end = Some(parse_duration(&v.text, "end time")?);
                i += 1;
            }
            _ => {
                let v = parse_duration(&w, "trim time")?;
                if start.is_none() {
                    start = Some(v);
                } else if end.is_none() {
                    end = Some(v);
                }
            }
        }
        i += 1;
    }
    let start = start.ok_or("expected a start and end time — e.g. 'trim 5 to 12'")?;
    let end = end.ok_or("expected an end time — e.g. 'trim 5 to 12'")?;
    if end <= start {
        return Err(format!(
            "trim end must be after start, got start {} and end {}",
            num(start),
            num(end)
        ));
    }
    Ok(match stream {
        Stream::Video => vec![
            format!("trim=start={}:end={}", num(start), num(end)),
            "setpts=PTS-STARTPTS".into(),
        ],
        Stream::Audio => vec![
            format!("atrim=start={}:end={}", num(start), num(end)),
            "asetpts=PTS-STARTPTS".into(),
        ],
    })
}

fn compile_eq(key: &str, rest: &[&Token], lo: f64, hi: f64) -> Result<Vec<String>, String> {
    let tok = rest
        .first()
        .ok_or_else(|| format!("expected a {key} value between {} and {}", num(lo), num(hi)))?;
    let v = parse_number(&tok.text, key)?;
    if !(lo..=hi).contains(&v) {
        return Err(format!(
            "{key} must be between {} and {}, got '{}'",
            num(lo),
            num(hi),
            tok.text
        ));
    }
    Ok(vec![format!("eq={key}={}", num(v))])
}

fn compile_hue(rest: &[&Token]) -> Result<Vec<String>, String> {
    let tok = rest
        .first()
        .ok_or("expected a hue shift in degrees — e.g. 'hue 90'")?;
    let cleaned = tok
        .text
        .to_ascii_lowercase()
        .replace("degrees", "")
        .replace("deg", "")
        .replace('°', "");
    let v = parse_number(cleaned.trim(), "hue shift")?;
    if !(-360.0..=360.0).contains(&v) {
        return Err(format!(
            "hue shift must be between -360 and 360 degrees, got '{}'",
            tok.text
        ));
    }
    Ok(vec![format!("hue=h={}", num(v))])
}

fn compile_text(rest: &[&Token]) -> Result<Vec<String>, String> {
    let quoted = rest.iter().find(|t| t.quoted).ok_or(
        "expected the caption in quotes — e.g. text \"Hello\" size 36 color yellow position bottom",
    )?;
    let content = &quoted.text;
    if content.is_empty() {
        return Err("the caption text is empty".into());
    }
    if content.chars().count() > 200 {
        return Err(format!(
            "caption text must be at most 200 characters, got {}",
            content.chars().count()
        ));
    }
    // Keep the emitted value trivially safe: no quote/backslash escaping games and
    // no control characters. drawtext is written with expansion=none as well.
    if let Some(bad) = content
        .chars()
        .find(|c| *c == '\'' || *c == '\\' || c.is_control())
    {
        return Err(format!(
            "caption text may not contain {} — single quotes, backslashes and control characters cannot be safely escaped in a filtergraph",
            if bad.is_control() {
                "control characters".to_string()
            } else {
                format!("'{bad}'")
            }
        ));
    }

    let mut size = 24.0f64;
    let mut color = "white".to_string();
    let mut position = "bottom".to_string();
    let mut box_bg = false;
    let mut i = 0usize;
    while i < rest.len() {
        if rest[i].quoted {
            i += 1;
            continue;
        }
        let w = rest[i].text.to_ascii_lowercase();
        match w.as_str() {
            "size" | "fontsize" => {
                let v = rest.get(i + 1).ok_or("expected a font size after 'size'")?;
                size = parse_number(&v.text, "font size")?;
                if !(4.0..=400.0).contains(&size) {
                    return Err(format!(
                        "font size must be between 4 and 400, got '{}'",
                        v.text
                    ));
                }
                i += 1;
            }
            "color" | "colour" | "fontcolor" => {
                let v = rest.get(i + 1).ok_or("expected a color after 'color'")?;
                color = check_color(&v.text)?;
                i += 1;
            }
            "position" | "at" | "align" => {
                let v = rest.get(i + 1).ok_or("expected a position after 'position'")?;
                position = v.text.to_ascii_lowercase();
                i += 1;
            }
            "top" | "bottom" | "center" | "centre" | "middle" => position = w,
            "box" | "boxed" | "background" => box_bg = true,
            _ => {}
        }
        i += 1;
    }
    let y = match position.as_str() {
        "top" => "20".to_string(),
        "center" | "centre" | "middle" => "(h-text_h)/2".to_string(),
        "bottom" => "h-text_h-20".to_string(),
        other => {
            return Err(format!(
                "expected a position of top, center or bottom, got '{other}'"
            ))
        }
    };
    let mut f = format!(
        "drawtext=text='{content}':x=(w-text_w)/2:y={y}:fontsize={}:fontcolor={color}",
        num(size)
    );
    if box_bg {
        f.push_str(":box=1:boxcolor=black@0.5:boxborderw=10");
    }
    f.push_str(":expansion=none");
    Ok(vec![f])
}

fn compile_volume(rest: &[&Token]) -> Result<Vec<String>, String> {
    let tok = rest
        .first()
        .ok_or("expected a volume factor or dB value — e.g. 'volume 2' or 'volume -6dB'")?;
    let lower = tok.text.to_ascii_lowercase();
    if let Some(dbs) = lower.strip_suffix("db") {
        let v = parse_number(dbs.trim(), "volume in dB")?;
        if !(-60.0..=30.0).contains(&v) {
            return Err(format!(
                "volume in dB must be between -60 and 30, got '{}'",
                tok.text
            ));
        }
        return Ok(vec![format!("volume={}dB", num(v))]);
    }
    let v = parse_number(&lower, "volume factor")?;
    if !(0.0..=64.0).contains(&v) {
        return Err(format!(
            "volume factor must be between 0 and 64, got '{}'",
            tok.text
        ));
    }
    Ok(vec![format!("volume={}", num(v))])
}

fn compile_pass(filter: &str, rest: &[&Token], default_hz: f64) -> Result<Vec<String>, String> {
    let hz = match rest.first() {
        Some(t) => {
            let cleaned = t.text.to_ascii_lowercase();
            let cleaned = cleaned.trim_end_matches("hz");
            parse_number(cleaned, "cutoff frequency in Hz")?
        }
        None => default_hz,
    };
    if !(1.0..=20_000.0).contains(&hz) {
        return Err(format!(
            "cutoff frequency must be between 1 and 20000 Hz, got '{}'",
            num(hz)
        ));
    }
    Ok(vec![format!("{filter}=f={}", num(hz))])
}

/// Syntax-check a `raw` filter the user wrote by hand. It is validated, never run.
fn compile_raw(literal: &str) -> Result<String, String> {
    let f = literal.trim();
    if f.is_empty() {
        return Err("expected a filter after 'raw' — e.g. raw vibrance=intensity=0.5".into());
    }
    if f.chars().any(|c| c.is_control()) {
        return Err("a raw filter may not contain control characters".into());
    }
    let name: String = f
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return Err(format!(
            "expected a filter name (letters, digits, underscore) at the start of the raw filter, got '{f}'"
        ));
    }
    let after = &f[name.len()..];
    if !after.is_empty() && !after.starts_with('=') && !after.starts_with('@') {
        return Err(format!(
            "expected '=' after the raw filter name '{name}', got '{}'",
            after.chars().next().unwrap()
        ));
    }
    if f.contains(',') && !f.contains('\'') {
        return Err(format!(
            "a raw step must be ONE filter — '{f}' contains a comma; put each filter on its own line, or quote an expression that needs commas"
        ));
    }
    validate_graph(f)?;
    Ok(f.to_string())
}

// ---------------------------------------------------------------- validation

/// Check the assembled chain is a syntactically well-formed single filter chain.
fn validate_graph(chain: &str) -> Result<(), String> {
    if chain.is_empty() {
        return Err("the filtergraph is empty".into());
    }
    if chain.chars().any(|c| c.is_control()) {
        return Err("the filtergraph contains a control character".into());
    }
    if chain.contains(';') {
        return Err(
            "the filtergraph contains ';', which starts a second chain — this tool builds one linear chain".into(),
        );
    }
    let mut quote = false;
    let mut depth: i32 = 0;
    let mut brackets: i32 = 0;
    let mut prev = '\0';
    for c in chain.chars() {
        if prev != '\\' {
            match c {
                '\'' => quote = !quote,
                '(' if !quote => depth += 1,
                ')' if !quote => {
                    depth -= 1;
                    if depth < 0 {
                        return Err("the filtergraph has an unmatched ')'".into());
                    }
                }
                '[' if !quote => brackets += 1,
                ']' if !quote => {
                    brackets -= 1;
                    if brackets < 0 {
                        return Err("the filtergraph has an unmatched ']'".into());
                    }
                }
                _ => {}
            }
        }
        prev = c;
    }
    if quote {
        return Err("the filtergraph has an unbalanced ' quote".into());
    }
    if depth != 0 {
        return Err("the filtergraph has an unmatched '('".into());
    }
    if brackets != 0 {
        return Err("the filtergraph has an unmatched '['".into());
    }
    Ok(())
}

/// A pad label must be a plain identifier — it goes inside `[...]` in the graph.
fn check_label(label: &str) -> Result<(), String> {
    let ok = !label.is_empty()
        && label.len() <= 40
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "label '{label}': expected letters, digits, '_', ':', '.' or '-' (e.g. '0:v' or 'out')"
        ))
    }
}

fn resolve_label(label: &str, stream: Stream) -> Result<String, String> {
    let l = label.trim();
    if l.is_empty() || l.eq_ignore_ascii_case("auto") {
        return Ok(stream.default_label().to_string());
    }
    let l = l.trim_start_matches('[').trim_end_matches(']');
    check_label(l)?;
    Ok(l.to_string())
}

/// File names go into a copy-me shell command, so they are restricted to an
/// allowlist — no metacharacter can ever be interpolated into that line.
fn check_path(path: &str, what: &str, example: &str) -> Result<String, String> {
    let p = path.trim();
    if p.is_empty() {
        return Ok(example.to_string());
    }
    if p.len() > 200 {
        return Err(format!("{what}: path is too long (limit 200 characters)"));
    }
    if let Some(bad) = p
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '+' | '=')))
    {
        return Err(format!(
            "{what}: '{bad}' is not allowed in a file name here — use letters, digits, '.', '_', '-', '+' or '/' (e.g. {example}). Rename the file, or edit the generated command by hand."
        ));
    }
    Ok(p.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(steps: &str) -> String {
        build(steps, &Options::default()).unwrap()
    }

    fn chain(steps: &str) -> String {
        build(
            steps,
            &Options {
                output: OutputForm::FilterChain,
                ..Options::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn happy_path_scale_crop_fade() {
        assert_eq!(
            v("scale to 720p\ncrop to square\nfade in 1s"),
            "[0:v]scale=-2:720,crop='min(iw,ih)':'min(iw,ih)',fade=t=in:st=0:d=1[out]"
        );
    }

    #[test]
    fn then_chain_on_one_line() {
        assert_eq!(
            chain("scale 1280x720, then grayscale, then fade out 2 at 10"),
            "scale=1280:720,hue=s=0,fade=t=out:st=10:d=2"
        );
    }

    #[test]
    fn list_markers_and_numbering_are_stripped() {
        assert_eq!(chain("1. scale 640x360\n2. blur 3"), "scale=640:360,gblur=sigma=3");
        assert_eq!(chain("- fps 30\n- reverse"), "fps=30,reverse");
    }

    #[test]
    fn scale_forms() {
        assert_eq!(chain("scale width 1280"), "scale=1280:-2");
        assert_eq!(chain("scale height 720"), "scale=-2:720");
        assert_eq!(chain("scale 50%"), "scale=trunc(iw*0.5/2)*2:trunc(ih*0.5/2)*2");
        assert_eq!(chain("scale 4k"), "scale=-2:2160");
        assert_eq!(chain("scale 1280x-2"), "scale=1280:-2");
    }

    #[test]
    fn crop_and_pad_forms() {
        assert_eq!(chain("crop 640x640"), "crop=640:640");
        assert_eq!(
            chain("crop 16:9"),
            "crop='min(iw,ih*16/9)':'min(ih,iw*9/16)'"
        );
        assert_eq!(
            chain("pad 1920x1080 #101010"),
            "pad=1920:1080:(ow-iw)/2:(oh-ih)/2:#101010"
        );
        assert_eq!(
            chain("pad 16:9"),
            "pad='max(iw,ih*16/9)':'max(ih,iw*9/16)':'(ow-iw)/2':'(oh-ih)/2':black"
        );
    }

    #[test]
    fn rotate_flip_and_eq() {
        assert_eq!(chain("rotate 90"), "transpose=1");
        assert_eq!(chain("rotate 180 degrees"), "transpose=1,transpose=1");
        assert_eq!(chain("rotate 270"), "transpose=2");
        assert_eq!(chain("flip horizontal"), "hflip");
        assert_eq!(chain("brightness 0.1\ncontrast 1.2\nsaturation 1.4"), "eq=brightness=0.1,eq=contrast=1.2,eq=saturation=1.4");
        assert_eq!(chain("hue 90"), "hue=h=90");
    }

    #[test]
    fn speed_and_trim() {
        assert_eq!(chain("speed 2x"), "setpts=0.5*PTS");
        assert_eq!(chain("speed 0.5"), "setpts=2*PTS");
        assert_eq!(
            chain("trim 5 to 12.5"),
            "trim=start=5:end=12.5,setpts=PTS-STARTPTS"
        );
    }

    #[test]
    fn drawtext_is_escaped_and_expansion_disabled() {
        assert_eq!(
            chain("text \"Hello World\" size 36 color yellow position top"),
            "drawtext=text='Hello World':x=(w-text_w)/2:y=20:fontsize=36:fontcolor=yellow:expansion=none"
        );
        assert!(chain("text \"Hi\" box").contains("box=1:boxcolor=black@0.5"));
    }

    #[test]
    fn drawtext_rejects_unescapable_characters() {
        let e = build("text \"it's here\"", &Options::default()).unwrap_err();
        assert!(e.contains("single quotes"), "{e}");
        let e = build("text \"a%{pts}b\"\n", &Options::default()).unwrap();
        assert!(e.contains("expansion=none"), "{e}");
    }

    #[test]
    fn audio_stream_steps() {
        let opts = Options {
            stream: Stream::Audio,
            output: OutputForm::FilterChain,
            ..Options::default()
        };
        assert_eq!(
            build("volume -6dB\nfade in 2\nnormalize", &opts).unwrap(),
            "volume=-6dB,afade=t=in:st=0:d=2,loudnorm=I=-16:TP=-1.5:LRA=11"
        );
        assert_eq!(build("speed 4x", &opts).unwrap(), "atempo=2,atempo=2");
        assert_eq!(build("speed 3", &opts).unwrap(), "atempo=2,atempo=1.5");
        assert_eq!(
            build("trim 0 to 30", &opts).unwrap(),
            "atrim=start=0:end=30,asetpts=PTS-STARTPTS"
        );
        assert_eq!(build("highpass\nlowpass 8000", &opts).unwrap(), "highpass=f=200,lowpass=f=8000");
        assert_eq!(build("mono\nreverse", &opts).unwrap(), "aformat=channel_layouts=mono,areverse");
    }

    #[test]
    fn audio_default_label_and_map_order() {
        let opts = Options {
            stream: Stream::Audio,
            output: OutputForm::Command,
            ..Options::default()
        };
        assert_eq!(
            build("volume 2", &opts).unwrap(),
            "ffmpeg -i input.mp4 -filter_complex \"[0:a]volume=2[out]\" -map \"0:v?\" -map \"[out]\" output.mp4"
        );
    }

    #[test]
    fn command_form() {
        let opts = Options {
            output: OutputForm::Command,
            input_file: "clip.mov".into(),
            output_file: "out/final.mp4".into(),
            ..Options::default()
        };
        assert_eq!(
            build("scale 720p", &opts).unwrap(),
            "ffmpeg -i clip.mov -filter_complex \"[0:v]scale=-2:720[out]\" -map \"[out]\" -map \"0:a?\" out/final.mp4"
        );
    }

    #[test]
    fn command_form_rejects_shell_metacharacters() {
        let opts = Options {
            output: OutputForm::Command,
            input_file: "in.mp4; rm -rf /".into(),
            ..Options::default()
        };
        let e = build("scale 720p", &opts).unwrap_err();
        assert!(e.contains("not allowed in a file name"), "{e}");
    }

    #[test]
    fn custom_labels() {
        let opts = Options {
            input_label: "[1:v]".into(),
            output_label: "vout".into(),
            ..Options::default()
        };
        assert_eq!(
            build("grayscale", &opts).unwrap(),
            "[1:v]hue=s=0[vout]"
        );
    }

    #[test]
    fn explain_lists_each_step() {
        let opts = Options {
            explain: true,
            ..Options::default()
        };
        let out = build("scale 720p\nfade in", &opts).unwrap();
        assert!(out.starts_with("[0:v]scale=-2:720,fade=t=in:st=0:d=1[out]\n\n# How each step compiled:"), "{out}");
        assert!(out.contains("\n# 1. scale 720p → scale=-2:720"), "{out}");
        assert!(out.contains("\n# 2. fade in → fade=t=in:st=0:d=1"), "{out}");
    }

    #[test]
    fn raw_escape_hatch_is_validated() {
        assert_eq!(chain("raw vibrance=intensity=0.5"), "vibrance=intensity=0.5");
        let e = build("raw scale=1280:720,crop=100:100", &Options::default()).unwrap_err();
        assert!(e.contains("ONE filter"), "{e}");
        let e = build("raw =broken", &Options::default()).unwrap_err();
        assert!(e.contains("filter name"), "{e}");
        let e = build("raw drawtext=text='unclosed", &Options::default()).unwrap_err();
        assert!(e.contains("unbalanced"), "{e}");
    }

    #[test]
    fn error_unknown_step_names_the_line_and_alternatives() {
        let e = build("scale 720p\nsparkle 3", &Options::default()).unwrap_err();
        assert!(e.starts_with("step 2 ('sparkle 3')"), "{e}");
        assert!(e.contains("unknown video step 'sparkle'"), "{e}");
        assert!(e.contains("scale"), "{e}");
    }

    #[test]
    fn error_wrong_stream_for_step() {
        let opts = Options {
            stream: Stream::Audio,
            ..Options::default()
        };
        let e = build("scale 720p", &opts).unwrap_err();
        assert!(e.contains("'scale' is a video step but stream is audio"), "{e}");
    }

    #[test]
    fn error_empty_and_oversized_input() {
        let e = build("   \n\n", &Options::default()).unwrap_err();
        assert!(e.contains("at least one filter step"), "{e}");
        let many = (0..MAX_STEPS + 1)
            .map(|_| "grayscale")
            .collect::<Vec<_>>()
            .join("\n");
        let e = build(&many, &Options::default()).unwrap_err();
        assert!(e.contains("too many steps"), "{e}");
    }

    #[test]
    fn error_bad_values_say_what_was_expected() {
        let e = build("scale huge", &Options::default()).unwrap_err();
        assert!(e.contains("expected a size like '1280x720'"), "{e}");
        let e = build("fps 0", &Options::default()).unwrap_err();
        assert!(e.contains("between 0 and 1000"), "{e}");
        let e = build("trim 10 to 5", &Options::default()).unwrap_err();
        assert!(e.contains("end must be after start"), "{e}");
        let e = build("pad 16:9 nope!", &Options::default()).unwrap_err();
        assert!(e.contains("expected a color"), "{e}");
        let e = build("fade sideways", &Options::default()).unwrap_err();
        assert!(e.contains("seconds"), "{e}");
    }

    #[test]
    fn build_from_strs_matches_typed_api() {
        let out = build_from_strs(
            "scale 720p",
            "video",
            "filter_complex",
            "auto",
            "out",
            "input.mp4",
            "output.mp4",
            false,
        )
        .unwrap();
        assert_eq!(out, "[0:v]scale=-2:720[out]");
        let e = build_from_strs("scale 720p", "sideways", "command", "", "", "", "", false)
            .unwrap_err();
        assert!(e.contains("expected 'video' or 'audio'"), "{e}");
    }
}
