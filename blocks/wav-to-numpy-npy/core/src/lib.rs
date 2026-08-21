//! wav-to-numpy-npy core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps, no third-party crates: the WAV chunk
//! walker, the sample decoders and the whole `.npy` writer are std-only Rust, so
//! this compiles and instantiates for both `wasm32-wasip1` (chat/CLI) and
//! `wasm32-unknown-unknown` (page).
//!
//! Decodes an uncompressed PCM/IEEE-float WAV (pasted as base64 or hex bytes)
//! and serialises its samples as a NumPy `.npy` v1.0 array file:
//!
//! ```text
//! \x93NUMPY  1 0  <hlen: u16 LE>  {'descr': '<f4', 'fortran_order': False, 'shape': (3, 2), }  <raw data>
//!    6 B    2 B       2 B          ASCII dict, space padded, '\n' terminated     C or F order
//! ```
//!
//! The 10-byte preamble plus the header string is padded with spaces to a
//! multiple of 64 bytes, exactly as `numpy.save` writes it, so the result loads
//! with `np.load()` and round-trips through this repo's `npy-array-decoder`.
//!
//! Only uncompressed WAV is decoded (PCM 8/16/24/32-bit integer and 32/64-bit
//! IEEE float). Compressed containers/codecs (MP3, AAC/M4A, Opus/Ogg, FLAC,
//! A-law/mu-law) are rejected with a clear message rather than guessed at.

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Largest WAV accepted after base64/hex decoding.
pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

/// Largest value of `max_frames` (0 still means "to the end of the clip").
pub const MAX_FRAMES_CAP: u64 = 1_000_000;

/// Largest number of array elements (sample frames × channels) materialised.
pub const MAX_ELEMENTS: usize = 4_000_000;

/// Largest `.npy` payload emitted, per output encoding. Base64 inflates by ~4/3
/// and hex by 2×, so the caps differ to keep the rendered text comparable.
pub const MAX_NPY_BYTES_BASE64: usize = 6 * 1024 * 1024;
pub const MAX_NPY_BYTES_HEX: usize = 3 * 1024 * 1024;

/// Export a WAV clip's decoded PCM samples as a NumPy `.npy` array file.
///
/// - `input`: the WAV file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `dtype`: the NumPy dtype of the emitted array — `"float32"` (default,
///   normalized to [-1, 1]), `"float64"`, `"int16"`, `"int32"`, `"uint8"`, or
///   `"auto"` (the source's own dtype with its raw stored values, the way
///   `scipy.io.wavfile.read` returns them).
/// - `shape`: `"auto"` (default — 1-D `(frames,)` for mono, 2-D
///   `(frames, channels)` otherwise), `"frames_channels"` (always 2-D, like
///   `always_2d`), `"channels_frames"` (transposed, channels-first), or
///   `"flat"` (1-D interleaved regardless of channel count).
/// - `mono`: average all channels down to one before writing (default false).
/// - `fortran_order`: write column-major data and set the header flag
///   (default false = C order). Ignored for 1-D shapes, which NumPy always
///   records as `fortran_order: False`.
/// - `start_frame`: first sample frame to export (default 0).
/// - `max_frames`: how many frames to export; `0` (default) = to the end.
/// - `output`: `"base64"` (default), `"hex"`, or `"info"` (the array/source
///   report plus a ready-to-run `np.load` snippet, no sample bytes).
///
/// Returns a user-facing error string for undecodable input, a non-WAV or
/// compressed file, a malformed WAV, out-of-range options, or an export larger
/// than the output cap.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    dtype: &str,
    shape: &str,
    mono: bool,
    fortran_order: bool,
    start_frame: u64,
    max_frames: u64,
    output: &str,
) -> Result<String, String> {
    let want_dtype = parse_dtype(dtype)?;
    let want_shape = parse_shape(shape)?;
    let out_kind = parse_output(output)?;
    if max_frames > MAX_FRAMES_CAP {
        return Err(format!(
            "invalid max_frames {max_frames}: expected 0-{MAX_FRAMES_CAP} (0 = to the end of the clip)"
        ));
    }

    let bytes = decode_bytes(input, input_format)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large: {} bytes decoded, cap is {MAX_INPUT_BYTES} bytes ({} MiB)",
            bytes.len(),
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    let wav = parse_wav(&bytes)?;

    // Window the clip BEFORE decoding so only the exported slice is materialised.
    if start_frame >= wav.total_frames {
        return Err(format!(
            "start_frame {start_frame} is past the end: the clip has {} sample frames (indices 0-{})",
            wav.total_frames,
            wav.total_frames.saturating_sub(1)
        ));
    }
    let requested = if max_frames == 0 {
        wav.total_frames
    } else {
        max_frames
    };
    let end_frame = start_frame.saturating_add(requested).min(wav.total_frames);
    let frames = (end_frame - start_frame) as usize;

    // Channel count AFTER the optional downmix — it drives the shape and size.
    let src_channels = wav.channels as usize;
    let channels = if mono { 1 } else { src_channels };
    let elements = frames.saturating_mul(channels);
    if elements > MAX_ELEMENTS {
        return Err(format!(
            "the selected window has {elements} array values ({frames} frames x {channels} channel(s)), \
             over the {MAX_ELEMENTS} cap — narrow it with start_frame / max_frames, or set mono=true"
        ));
    }

    let dt = match want_dtype {
        WantDtype::Auto => auto_dtype(wav.bits_per_sample, wav.is_float),
        WantDtype::Fixed(d) => d,
    };
    let dims = want_shape.dims(frames, channels);
    // NumPy records a 1-D array as C-ordered no matter how it was built.
    let header_fortran = fortran_order && dims.len() == 2;
    let header = npy_header(dt, header_fortran, &dims);
    let data_bytes = elements.saturating_mul(dt.itemsize());
    let file_bytes = header.len() + data_bytes;

    if let Some(cap) = out_kind.byte_cap() {
        if file_bytes > cap {
            return Err(format!(
                "the .npy file would be {file_bytes} bytes, over the {cap}-byte ({} MiB) cap for the \
                 {} output — each frame costs {} bytes at dtype {}, so lower max_frames (or pick a \
                 smaller dtype). Run output=info to size an export without producing it.",
                cap / (1024 * 1024),
                out_kind.label(),
                channels * dt.itemsize(),
                dt.name()
            ));
        }
    }

    if out_kind == OutKind::Info {
        return Ok(render_info(
            &wav,
            bytes.len(),
            dt,
            &dims,
            header_fortran,
            header.len(),
            data_bytes,
            start_frame,
            end_frame,
            channels,
            mono,
        ));
    }

    // Decode just the window, normalized to [-1, 1] in f64 (lossless for every
    // supported integer depth — the divisors are powers of two).
    let samples = decode_window(&wav, &bytes, start_frame as usize, end_frame as usize)?;
    let samples = if mono && src_channels > 1 {
        downmix(&samples, src_channels)
    } else {
        samples
    };

    let mut file = header;
    file.reserve(data_bytes);
    write_data(&mut file, &samples, frames, channels, want_shape, fortran_order, dt);

    Ok(match out_kind {
        OutKind::Base64 => encode_base64(&file),
        OutKind::Hex => encode_hex(&file),
        OutKind::Info => unreachable!("handled above"),
    })
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum DType {
    U8,
    I16,
    I32,
    F32,
    F64,
}

impl DType {
    /// The `descr` string NumPy writes. Single-byte types use `|` (byte order
    /// is meaningless), everything else is little-endian `<`.
    fn descr(self) -> &'static str {
        match self {
            DType::U8 => "|u1",
            DType::I16 => "<i2",
            DType::I32 => "<i4",
            DType::F32 => "<f4",
            DType::F64 => "<f8",
        }
    }
    fn name(self) -> &'static str {
        match self {
            DType::U8 => "uint8",
            DType::I16 => "int16",
            DType::I32 => "int32",
            DType::F32 => "float32",
            DType::F64 => "float64",
        }
    }
    fn itemsize(self) -> usize {
        match self {
            DType::U8 => 1,
            DType::I16 => 2,
            DType::I32 => 4,
            DType::F32 => 4,
            DType::F64 => 8,
        }
    }
}

enum WantDtype {
    Auto,
    Fixed(DType),
}

fn parse_dtype(s: &str) -> Result<WantDtype, String> {
    Ok(match s.trim() {
        "" | "float32" => WantDtype::Fixed(DType::F32),
        "float64" => WantDtype::Fixed(DType::F64),
        "int16" => WantDtype::Fixed(DType::I16),
        "int32" => WantDtype::Fixed(DType::I32),
        "uint8" => WantDtype::Fixed(DType::U8),
        "auto" => WantDtype::Auto,
        other => {
            return Err(format!(
                "invalid dtype {other:?}: expected \"auto\", \"float32\", \"float64\", \"int16\", \
                 \"int32\", or \"uint8\""
            ))
        }
    })
}

/// The dtype `scipy.io.wavfile.read` would return for this source: unsigned for
/// 8-bit, signed for 9-bit and up, 24-bit widened (left-justified) into int32.
fn auto_dtype(bits: u16, is_float: bool) -> DType {
    if is_float {
        if bits == 64 {
            DType::F64
        } else {
            DType::F32
        }
    } else {
        match bits {
            8 => DType::U8,
            16 => DType::I16,
            _ => DType::I32,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShapeKind {
    Auto,
    FramesChannels,
    ChannelsFrames,
    Flat,
}

impl ShapeKind {
    fn dims(self, frames: usize, channels: usize) -> Vec<usize> {
        match self {
            ShapeKind::Auto if channels == 1 => vec![frames],
            ShapeKind::Auto | ShapeKind::FramesChannels => vec![frames, channels],
            ShapeKind::ChannelsFrames => vec![channels, frames],
            ShapeKind::Flat => vec![frames * channels],
        }
    }
}

fn parse_shape(s: &str) -> Result<ShapeKind, String> {
    Ok(match s.trim() {
        "" | "auto" => ShapeKind::Auto,
        "frames_channels" => ShapeKind::FramesChannels,
        "channels_frames" => ShapeKind::ChannelsFrames,
        "flat" => ShapeKind::Flat,
        other => {
            return Err(format!(
                "invalid shape {other:?}: expected \"auto\", \"frames_channels\", \
                 \"channels_frames\", or \"flat\""
            ))
        }
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutKind {
    Base64,
    Hex,
    Info,
}

impl OutKind {
    fn byte_cap(self) -> Option<usize> {
        match self {
            OutKind::Base64 => Some(MAX_NPY_BYTES_BASE64),
            OutKind::Hex => Some(MAX_NPY_BYTES_HEX),
            OutKind::Info => None,
        }
    }
    fn label(self) -> &'static str {
        match self {
            OutKind::Base64 => "base64",
            OutKind::Hex => "hex",
            OutKind::Info => "info",
        }
    }
}

fn parse_output(s: &str) -> Result<OutKind, String> {
    Ok(match s.trim() {
        "" | "base64" => OutKind::Base64,
        "hex" => OutKind::Hex,
        "info" => OutKind::Info,
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"base64\", \"hex\", or \"info\""
            ))
        }
    })
}

// ---------------------------------------------------------------------------
// .npy writing
// ---------------------------------------------------------------------------

/// NumPy's `.npy` magic string.
const NPY_MAGIC: &[u8; 6] = b"\x93NUMPY";
/// v1.0 preamble: magic (6) + version (2) + header length (2).
const NPY_PREAMBLE_V1: usize = 10;
/// NumPy pads the header so the data starts on a 64-byte boundary.
const NPY_ALIGN: usize = 64;

/// Render the shape tuple the way Python does — a 1-tuple keeps its comma.
fn shape_literal(dims: &[usize]) -> String {
    match dims {
        [n] => format!("({n},)"),
        _ => {
            let parts: Vec<String> = dims.iter().map(|d| d.to_string()).collect();
            format!("({})", parts.join(", "))
        }
    }
}

/// Build the complete `.npy` v1.0 header: magic, version, length, the padded
/// dict, and the terminating newline. The returned Vec is the file prefix that
/// the raw array data is appended to.
fn npy_header(dt: DType, fortran_order: bool, dims: &[usize]) -> Vec<u8> {
    let dict = format!(
        "{{'descr': '{}', 'fortran_order': {}, 'shape': {}, }}",
        dt.descr(),
        if fortran_order { "True" } else { "False" },
        shape_literal(dims)
    );
    // Pad with spaces so preamble + header (incl. the trailing '\n') is a
    // multiple of 64; the '\n' is the last byte of the header.
    let unpadded = NPY_PREAMBLE_V1 + dict.len() + 1;
    let padding = (NPY_ALIGN - (unpadded % NPY_ALIGN)) % NPY_ALIGN;
    let header_len = dict.len() + padding + 1;

    let mut out = Vec::with_capacity(NPY_PREAMBLE_V1 + header_len);
    out.extend_from_slice(NPY_MAGIC);
    out.push(1); // major
    out.push(0); // minor
    out.extend_from_slice(&(header_len as u16).to_le_bytes());
    out.extend_from_slice(dict.as_bytes());
    out.extend(std::iter::repeat(b' ').take(padding));
    out.push(b'\n');
    out
}

/// Append the array data in the memory order the header advertises.
///
/// `samples` is interleaved (frame-major). `(frames, channels)` in C order is
/// therefore the source order, and its Fortran order is the de-interleaved,
/// channel-major order; `(channels, frames)` is the transpose of both.
fn write_data(
    out: &mut Vec<u8>,
    samples: &[f64],
    frames: usize,
    channels: usize,
    shape: ShapeKind,
    fortran_order: bool,
    dt: DType,
) {
    let interleaved = |out: &mut Vec<u8>| {
        for &s in samples {
            encode_sample(out, s, dt);
        }
    };
    let channel_major = |out: &mut Vec<u8>| {
        for c in 0..channels {
            for f in 0..frames {
                encode_sample(out, samples[f * channels + c], dt);
            }
        }
    };
    match shape {
        // 1-D data has only one possible order.
        ShapeKind::Flat => interleaved(out),
        ShapeKind::Auto if channels == 1 => interleaved(out),
        // (frames, channels)
        ShapeKind::Auto | ShapeKind::FramesChannels => {
            if fortran_order {
                channel_major(out)
            } else {
                interleaved(out)
            }
        }
        // (channels, frames) — the transpose, so the orders swap.
        ShapeKind::ChannelsFrames => {
            if fortran_order {
                interleaved(out)
            } else {
                channel_major(out)
            }
        }
    }
}

/// Write one normalized sample in the target dtype. Float dtypes keep the
/// [-1, 1] normalization; integer dtypes are scaled to that type's full scale,
/// which makes `dtype=auto` reproduce the source's stored integers exactly
/// (every divisor used when decoding is a power of two).
fn encode_sample(out: &mut Vec<u8>, v: f64, dt: DType) {
    match dt {
        DType::U8 => {
            let x = (v * 128.0).round() + 128.0;
            out.push(x.clamp(0.0, 255.0) as u8);
        }
        DType::I16 => {
            let x = (v * 32_768.0).round().clamp(-32_768.0, 32_767.0) as i16;
            out.extend_from_slice(&x.to_le_bytes());
        }
        DType::I32 => {
            let x = (v * 2_147_483_648.0)
                .round()
                .clamp(-2_147_483_648.0, 2_147_483_647.0) as i32;
            out.extend_from_slice(&x.to_le_bytes());
        }
        DType::F32 => out.extend_from_slice(&(v as f32).to_le_bytes()),
        DType::F64 => out.extend_from_slice(&v.to_le_bytes()),
    }
}

fn downmix(samples: &[f64], channels: usize) -> Vec<f64> {
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f64>() / frame.len() as f64)
        .collect()
}

// ---------------------------------------------------------------------------
// Info report
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_info(
    wav: &WavInfo,
    file_len: usize,
    dt: DType,
    dims: &[usize],
    fortran_order: bool,
    header_bytes: usize,
    data_bytes: usize,
    start_frame: u64,
    end_frame: u64,
    channels: usize,
    mono: bool,
) -> String {
    let duration = wav.total_frames as f64 / wav.sample_rate as f64;
    let codec = if wav.is_float {
        "IEEE float (format tag 0x0003)"
    } else {
        "PCM integer (format tag 0x0001)"
    };
    let shape = shape_literal(dims);
    let mut out = String::with_capacity(1024);

    out.push_str("Source WAV\n");
    out.push_str(&format!("  file bytes      {file_len}\n"));
    out.push_str(&format!("  codec           {codec}\n"));
    out.push_str(&format!("  sample rate     {} Hz\n", wav.sample_rate));
    out.push_str(&format!("  channels        {}\n", wav.channels));
    out.push_str(&format!("  bit depth       {}-bit\n", wav.bits_per_sample));
    out.push_str(&format!("  total frames    {}\n", wav.total_frames));
    out.push_str(&format!("  duration        {duration:.6} s\n"));

    out.push_str("\nNumPy array\n");
    out.push_str(&format!(
        "  dtype           {} (descr '{}')\n",
        dt.name(),
        dt.descr()
    ));
    out.push_str(&format!("  shape           {shape}\n"));
    out.push_str(&format!(
        "  order           {} (fortran_order: {})\n",
        if fortran_order { "Fortran" } else { "C" },
        if fortran_order { "True" } else { "False" }
    ));
    out.push_str(&format!(
        "  frames          {} of {} (index {} - {})\n",
        end_frame - start_frame,
        wav.total_frames,
        start_frame,
        end_frame.saturating_sub(1)
    ));
    out.push_str(&format!(
        "  channels kept   {}{}\n",
        channels,
        if mono && wav.channels > 1 {
            " (mono downmix)"
        } else {
            ""
        }
    ));
    out.push_str(&format!("  itemsize        {} bytes\n", dt.itemsize()));
    out.push_str(&format!("  header bytes    {header_bytes}\n"));
    out.push_str(&format!("  data bytes      {data_bytes}\n"));
    out.push_str(&format!(
        "  .npy file bytes {}\n",
        header_bytes + data_bytes
    ));

    out.push_str("\nLoad it back (a .npy stores the array only — the sample rate is NOT in the file)\n");
    out.push_str("  save            base64 -d > audio.npy\n");
    out.push_str("  python          import numpy as np\n");
    out.push_str(&format!(
        "                  data = np.load(\"audio.npy\")   # dtype {}, shape {}\n",
        dt.name(),
        shape
    ));
    out.push_str(&format!(
        "                  sample_rate = {}\n",
        wav.sample_rate
    ));
    out
}

// ---------------------------------------------------------------------------
// WAV parsing (uncompressed RIFF/WAVE)
// ---------------------------------------------------------------------------

/// Everything needed to decode a window without materialising the whole clip:
/// the `fmt ` facts plus the byte range of the `data` chunk in the input.
struct WavInfo {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    is_float: bool,
    total_frames: u64,
    data_start: usize,
    block_align: usize,
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

fn parse_wav(b: &[u8]) -> Result<WavInfo, String> {
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
    let mut data: Option<(usize, usize)> = None; // (start, len)

    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let size = u32_le(b, pos + 4) as usize;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size).min(b.len());
        match id {
            b"fmt " => fmt = Some(parse_fmt(&b[body_start..body_end])?),
            b"data" if data.is_none() => data = Some((body_start, body_end - body_start)),
            _ => {}
        }
        let advance = 8 + size + (size & 1);
        pos = match pos.checked_add(advance) {
            Some(p) => p,
            None => break,
        };
    }

    let fmt = fmt.ok_or("malformed WAV: no `fmt ` chunk found")?;
    let (data_start, data_len) = data.ok_or("malformed WAV: no `data` chunk found")?;

    if fmt.channels == 0 {
        return Err("malformed WAV: channel count is 0".into());
    }
    if fmt.sample_rate == 0 {
        return Err("malformed WAV: sample rate is 0".into());
    }
    check_codec(&fmt)?;

    // Trust the decoded sample width over a bogus `block_align`.
    let block_align = fmt.channels as usize * (fmt.bits_per_sample as usize / 8);
    if block_align == 0 {
        return Err(format!(
            "malformed WAV: bit depth is {} — expected 8, 16, 24 or 32",
            fmt.bits_per_sample
        ));
    }
    let total_frames = (data_len / block_align) as u64;
    if total_frames == 0 {
        return Err("malformed WAV: the `data` chunk holds no complete sample frames".into());
    }

    Ok(WavInfo {
        sample_rate: fmt.sample_rate,
        channels: fmt.channels,
        bits_per_sample: fmt.bits_per_sample,
        is_float: fmt.audio_format == WAVE_FORMAT_IEEE_FLOAT,
        total_frames,
        data_start,
        block_align,
    })
}

/// Name a non-RIFF container so the error names the codec instead of a generic
/// "not a WAV".
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
    } else if starts(b"\x93NUMPY") {
        Some("a NumPy .npy file (this tool writes .npy, it does not read one)")
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

    // WAVE_FORMAT_EXTENSIBLE hides the real codec in the SubFormat GUID's
    // first two bytes.
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

/// Reject anything we can't decode BEFORE reporting frame counts, so an
/// `output=info` run on an A-law file errors instead of describing an array
/// that could never be written.
fn check_codec(fmt: &FmtChunk) -> Result<(), String> {
    match fmt.audio_format {
        WAVE_FORMAT_PCM => match fmt.bits_per_sample {
            8 | 16 | 24 | 32 => Ok(()),
            other => Err(format!(
                "unsupported PCM bit depth: {other}-bit. Supported: 8, 16, 24, 32-bit integer."
            )),
        },
        WAVE_FORMAT_IEEE_FLOAT => match fmt.bits_per_sample {
            32 | 64 => Ok(()),
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

/// Decode `frames[start..end]` to interleaved f64 normalized to [-1, 1]. Every
/// divisor is a power of two, so an integer source round-trips bit-exactly back
/// through `encode_sample`.
fn decode_window(
    wav: &WavInfo,
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<Vec<f64>, String> {
    let from = wav.data_start + start * wav.block_align;
    let to = wav.data_start + end * wav.block_align;
    let slice = bytes
        .get(from..to)
        .ok_or("malformed WAV: the `data` chunk is shorter than its declared size")?;
    let width = wav.bits_per_sample as usize / 8;
    let n = slice.len() / width;
    let mut out = Vec::with_capacity(n);
    for c in slice.chunks_exact(width) {
        out.push(match (wav.is_float, wav.bits_per_sample) {
            (false, 8) => (c[0] as f64 - 128.0) / 128.0,
            (false, 16) => i16::from_le_bytes([c[0], c[1]]) as f64 / 32_768.0,
            (false, 24) => {
                let raw = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
                (((raw << 8) >> 8) as f64) / 8_388_608.0 // sign-extend 24-bit
            }
            (false, _) => i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64 / 2_147_483_648.0,
            (true, 64) => f64::from_le_bytes([
                c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
            ]),
            (true, _) => f32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Byte encoding / decoding (hex / base64)
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

/// Standard base64 with `=` padding — what `base64 -d` expects.
fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// One unbroken run of lowercase hex — the form `xxd -r -p` reverses.
fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn build_riff(
        fmt_tag: u16,
        channels: u16,
        sample_rate: u32,
        bits: u16,
        data: &[u8],
    ) -> Vec<u8> {
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&fmt_tag.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        out
    }

    /// 16-bit PCM WAV from exact stored integers.
    fn wav16(sample_rate: u32, channels: u16, raw: &[i16]) -> Vec<u8> {
        let mut data = Vec::new();
        for &v in raw {
            data.extend_from_slice(&v.to_le_bytes());
        }
        build_riff(WAVE_FORMAT_PCM, channels, sample_rate, 16, &data)
    }

    fn wav8(sample_rate: u32, channels: u16, raw: &[u8]) -> Vec<u8> {
        build_riff(WAVE_FORMAT_PCM, channels, sample_rate, 8, raw)
    }

    fn wav24(sample_rate: u32, channels: u16, raw: &[i32]) -> Vec<u8> {
        let mut data = Vec::new();
        for &v in raw {
            data.extend_from_slice(&v.to_le_bytes()[0..3]);
        }
        build_riff(WAVE_FORMAT_PCM, channels, sample_rate, 24, &data)
    }

    fn wav32f(sample_rate: u32, channels: u16, raw: &[f32]) -> Vec<u8> {
        let mut data = Vec::new();
        for &v in raw {
            data.extend_from_slice(&v.to_le_bytes());
        }
        build_riff(WAVE_FORMAT_IEEE_FLOAT, channels, sample_rate, 32, &data)
    }

    fn wav64f(sample_rate: u32, channels: u16, raw: &[f64]) -> Vec<u8> {
        let mut data = Vec::new();
        for &v in raw {
            data.extend_from_slice(&v.to_le_bytes());
        }
        build_riff(WAVE_FORMAT_IEEE_FLOAT, channels, sample_rate, 64, &data)
    }

    fn b64(bytes: &[u8]) -> String {
        encode_base64(bytes)
    }

    /// Run with every option at its default.
    fn run_default(input: &str) -> Result<String, String> {
        run(input, "base64", "float32", "auto", false, false, 0, 0, "base64")
    }

    /// Decode a hex run back into bytes (the tests read the .npy we wrote).
    fn unhex(s: &str) -> Vec<u8> {
        decode_hex(s).unwrap()
    }

    /// Minimal `.npy` reader for assertions: (header dict, data bytes).
    fn split_npy(file: &[u8]) -> (String, Vec<u8>) {
        assert_eq!(&file[0..6], NPY_MAGIC, "magic");
        assert_eq!(file[6], 1, "major version");
        assert_eq!(file[7], 0, "minor version");
        let hlen = u16_le(file, 8) as usize;
        assert_eq!(
            (NPY_PREAMBLE_V1 + hlen) % NPY_ALIGN,
            0,
            "header must be 64-byte aligned"
        );
        let header = String::from_utf8(file[10..10 + hlen].to_vec()).unwrap();
        assert!(header.ends_with('\n'), "header must end with a newline");
        (header.trim().to_string(), file[10 + hlen..].to_vec())
    }

    /// `split_npy` over a hex-encoded result (the common assertion shape).
    fn split_hex(s: &str) -> (String, Vec<u8>) {
        split_npy(&unhex(s))
    }

    // -- happy paths --------------------------------------------------------

    #[test]
    fn happy_mono_float32_default() {
        // 16 kHz mono 16-bit, stored 16384, -8192, 0 → 0.5, -0.25, 0.0.
        let wav = wav16(16000, 1, &[16384, -8192, 0]);
        let out = run_default(&b64(&wav)).unwrap();
        let file = decode_base64(&out).unwrap();
        let (header, data) = split_npy(&file);
        assert_eq!(
            header,
            "{'descr': '<f4', 'fortran_order': False, 'shape': (3,), }"
        );
        let vals: Vec<f32> = data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(vals, vec![0.5, -0.25, 0.0]);
    }

    #[test]
    fn exact_base64_for_the_page_placeholder_clip() {
        // Pinned byte-for-byte: 10-byte preamble + 118-byte padded header (128
        // total, a multiple of 64) + 3 float32 samples = 140 bytes.
        let wav = wav16(16000, 1, &[16384, -8192, 0]);
        let out = run_default(&b64(&wav)).unwrap();
        assert_eq!(
            out,
            "k05VTVBZAQB2AHsnZGVzY3InOiAnPGY0JywgJ2ZvcnRyYW5fb3JkZXInOiBGYWxzZSwgJ3NoYXBlJzogKDMsKSwgfSAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAoAAAA/AACAvgAAAAA=",
            "got: {out}"
        );
        let file = decode_base64(&out).unwrap();
        assert_eq!(file.len(), 128 + 12);
    }

    #[test]
    fn auto_dtype_matches_scipy_per_bit_depth() {
        // 8-bit → uint8 with the raw stored bytes.
        let out = run(
            &b64(&wav8(8000, 1, &[0, 128, 255])),
            "base64", "auto", "auto", false, false, 0, 0, "hex",
        )
        .unwrap();
        let file = unhex(&out);
        let (header, data) = split_npy(&file);
        assert!(header.contains("'descr': '|u1'"), "{header}");
        assert_eq!(data, vec![0u8, 128, 255]);

        // 16-bit → int16, values verbatim.
        let out = run(
            &b64(&wav16(16000, 1, &[16384, -8192, 32767, -32768])),
            "base64", "auto", "auto", false, false, 0, 0, "hex",
        )
        .unwrap();
        let file = unhex(&out);
        let (header, data) = split_npy(&file);
        assert!(header.contains("'descr': '<i2'"), "{header}");
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![16384, -8192, 32767, -32768]);

        // 24-bit → int32, LEFT-JUSTIFIED (<< 8), exactly like scipy.
        let out = run(
            &b64(&wav24(48000, 1, &[1, -1, 8_388_607])),
            "base64", "auto", "auto", false, false, 0, 0, "hex",
        )
        .unwrap();
        let file = unhex(&out);
        let (header, data) = split_npy(&file);
        assert!(header.contains("'descr': '<i4'"), "{header}");
        let vals: Vec<i32> = data
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(vals, vec![256, -256, 2_147_483_392]);

        // 32-bit float → float32, values untouched.
        let out = run(
            &b64(&wav32f(44100, 1, &[0.25, -0.75])),
            "base64", "auto", "auto", false, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&out);
        assert!(header.contains("'descr': '<f4'"), "{header}");
        assert_eq!(data.len(), 8);

        // 64-bit float → float64.
        let out = run(
            &b64(&wav64f(44100, 1, &[0.25, -0.75])),
            "base64", "auto", "auto", false, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&out);
        assert!(header.contains("'descr': '<f8'"), "{header}");
        let vals: Vec<f64> = data
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
            .collect();
        assert_eq!(vals, vec![0.25, -0.75]);
    }

    #[test]
    fn every_fixed_dtype_scales_to_its_full_range() {
        let wav = b64(&wav16(16000, 1, &[16384, -16384]));
        for (name, descr, itemsize) in [
            ("float32", "<f4", 4usize),
            ("float64", "<f8", 8),
            ("int16", "<i2", 2),
            ("int32", "<i4", 4),
            ("uint8", "|u1", 1),
        ] {
            let out = run(&wav, "base64", name, "auto", false, false, 0, 0, "hex").unwrap();
            let (header, data) = split_hex(&out);
            assert!(header.contains(&format!("'descr': '{descr}'")), "{name}: {header}");
            assert_eq!(data.len(), 2 * itemsize, "{name} itemsize");
        }
        // 0.5 → int32 full scale, and → uint8 midpoint ± 64.
        let out = run(&wav, "base64", "int32", "auto", false, false, 0, 0, "hex").unwrap();
        let (_, data) = split_hex(&out);
        let vals: Vec<i32> = data
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(vals, vec![1_073_741_824, -1_073_741_824]);

        let out = run(&wav, "base64", "uint8", "auto", false, false, 0, 0, "hex").unwrap();
        let (_, data) = split_hex(&out);
        assert_eq!(data, vec![192u8, 64]);
    }

    #[test]
    fn shape_auto_is_1d_for_mono_and_2d_for_stereo() {
        let mono = run(
            &b64(&wav16(16000, 1, &[1, 2])),
            "base64", "int16", "auto", false, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, _) = split_hex(&mono);
        assert!(header.contains("'shape': (2,)"), "{header}");

        let stereo = run(
            &b64(&wav16(16000, 2, &[1, 2, 3, 4])),
            "base64", "int16", "auto", false, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&stereo);
        assert!(header.contains("'shape': (2, 2)"), "{header}");
        // C order over (frames, channels) is the interleaved source order.
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![1, 2, 3, 4]);
    }

    #[test]
    fn shape_frames_channels_forces_2d_for_mono() {
        let out = run(
            &b64(&wav16(16000, 1, &[7, 8])),
            "base64", "int16", "frames_channels", false, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, _) = split_hex(&out);
        assert!(header.contains("'shape': (2, 1)"), "{header}");
    }

    #[test]
    fn shape_channels_frames_transposes_and_deinterleaves() {
        // Stereo L,R = (1,2), (3,4) → channels-first C order is 1,3,2,4.
        let out = run(
            &b64(&wav16(16000, 2, &[1, 2, 3, 4])),
            "base64", "int16", "channels_frames", false, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&out);
        assert!(header.contains("'shape': (2, 2)"), "{header}");
        assert!(header.contains("'fortran_order': False"), "{header}");
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![1, 3, 2, 4]);
    }

    #[test]
    fn shape_flat_keeps_interleaved_1d() {
        let out = run(
            &b64(&wav16(16000, 2, &[1, 2, 3, 4])),
            "base64", "int16", "flat", false, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&out);
        assert!(header.contains("'shape': (4,)"), "{header}");
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![1, 2, 3, 4]);
    }

    #[test]
    fn fortran_order_writes_channel_major_data() {
        // (frames, channels) in Fortran order is de-interleaved: 1,3,2,4.
        let out = run(
            &b64(&wav16(16000, 2, &[1, 2, 3, 4])),
            "base64", "int16", "frames_channels", false, true, 0, 0, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&out);
        assert!(header.contains("'fortran_order': True"), "{header}");
        assert!(header.contains("'shape': (2, 2)"), "{header}");
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![1, 3, 2, 4]);
    }

    #[test]
    fn fortran_order_is_ignored_for_1d_shapes() {
        let out = run(
            &b64(&wav16(16000, 1, &[1, 2])),
            "base64", "int16", "auto", false, true, 0, 0, "hex",
        )
        .unwrap();
        let (header, _) = split_hex(&out);
        assert!(header.contains("'fortran_order': False"), "{header}");
    }

    #[test]
    fn mono_downmix_averages_channels() {
        // L=1.0-ish and R=0 average to half: 32766 and 0 → 16383.
        let out = run(
            &b64(&wav16(16000, 2, &[32766, 0, -32768, 0])),
            "base64", "int16", "auto", true, false, 0, 0, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&out);
        assert!(header.contains("'shape': (2,)"), "{header}");
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![16383, -16384]);
    }

    #[test]
    fn window_start_frame_and_max_frames() {
        let out = run(
            &b64(&wav16(16000, 1, &[0, 100, 200, 300, 400])),
            "base64", "int16", "auto", false, false, 1, 2, "hex",
        )
        .unwrap();
        let (header, data) = split_hex(&out);
        assert!(header.contains("'shape': (2,)"), "{header}");
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![100, 200]);
    }

    #[test]
    fn max_frames_clamps_to_the_clip_length() {
        let out = run(
            &b64(&wav16(16000, 1, &[1, 2, 3])),
            "base64", "int16", "auto", false, false, 0, MAX_FRAMES_CAP, "hex",
        )
        .unwrap();
        let (header, _) = split_hex(&out);
        assert!(header.contains("'shape': (3,)"), "{header}");
    }

    #[test]
    fn hex_input_is_accepted() {
        let wav = wav16(16000, 1, &[16384]);
        let hex: String = wav.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, "hex", "int16", "auto", false, false, 0, 0, "hex").unwrap();
        let (_, data) = split_hex(&out);
        assert_eq!(data, 16384i16.to_le_bytes().to_vec());
    }

    #[test]
    fn header_is_64_byte_aligned_for_every_shape() {
        // split_npy asserts the alignment; sweep shapes so the padding maths is
        // exercised at several dict lengths.
        for shape in ["auto", "frames_channels", "channels_frames", "flat"] {
            for dtype in ["float32", "float64", "uint8"] {
                let out = run(
                    &b64(&wav16(16000, 2, &[1, 2, 3, 4])),
                    "base64", dtype, shape, false, false, 0, 0, "hex",
                )
                .unwrap();
                split_hex(&out);
            }
        }
    }

    #[test]
    fn skips_unknown_chunks() {
        let data: Vec<u8> = (0..4).flat_map(|_| 100i16.to_le_bytes()).collect();
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((36 + 16 + data.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&WAVE_FORMAT_PCM.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&16000u32.to_le_bytes());
        out.extend_from_slice(&32000u32.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"LIST");
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(b"INFOjunk");
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        let res = run(&b64(&out), "base64", "int16", "auto", false, false, 0, 0, "hex").unwrap();
        let (header, data) = split_hex(&res);
        assert!(header.contains("'shape': (4,)"), "{header}");
        let vals: Vec<i16> = data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(vals, vec![100, 100, 100, 100]);
    }

    #[test]
    fn info_report_names_dtype_shape_and_sample_rate() {
        let wav = wav16(16000, 2, &[1, 2, 3, 4]);
        let out = run(
            &b64(&wav), "base64", "auto", "auto", false, false, 0, 0, "info",
        )
        .unwrap();
        assert!(out.contains("sample rate     16000 Hz"), "{out}");
        assert!(out.contains("channels        2"), "{out}");
        assert!(out.contains("dtype           int16 (descr '<i2')"), "{out}");
        assert!(out.contains("shape           (2, 2)"), "{out}");
        assert!(out.contains("order           C (fortran_order: False)"), "{out}");
        assert!(out.contains("header bytes    128"), "{out}");
        assert!(out.contains("data bytes      8"), "{out}");
        assert!(out.contains(".npy file bytes 136"), "{out}");
        assert!(out.contains("np.load"), "{out}");
        assert!(out.contains("sample_rate = 16000"), "{out}");
    }

    #[test]
    fn info_sizes_an_export_over_the_base64_cap() {
        // info has no byte cap, so it can size a export base64 would refuse.
        let big: Vec<i16> = vec![0; 200_000];
        let wav = wav16(44100, 1, &big);
        let out = run(
            &b64(&wav), "base64", "float64", "auto", false, false, 0, 0, "info",
        )
        .unwrap();
        assert!(out.contains("data bytes      1600000"), "{out}");
    }

    // -- errors -------------------------------------------------------------

    #[test]
    fn error_invalid_base64() {
        let err = run_default("not base64 @@@").unwrap_err();
        assert!(err.contains("base64"), "{err}");
    }

    #[test]
    fn error_empty_input() {
        let err = run_default("   ").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn error_odd_hex() {
        let err = run("abc", "hex", "float32", "auto", false, false, 0, 0, "base64").unwrap_err();
        assert!(err.contains("odd number"), "{err}");
    }

    #[test]
    fn error_not_a_wav() {
        let err = run_default(&b64(b"hello world not a wav at all")).unwrap_err();
        assert!(err.contains("not a WAV"), "{err}");
    }

    #[test]
    fn error_compressed_inputs_are_named() {
        let mp3 = run_default(&b64(b"ID3\x04\x00\x00\x00\x00\x00\x00rest")).unwrap_err();
        assert!(mp3.contains("MP3"), "{mp3}");
        let flac = run_default(&b64(b"fLaC\x00\x00\x00\x22rest")).unwrap_err();
        assert!(flac.contains("FLAC"), "{flac}");
        let npy = run_default(&b64(b"\x93NUMPY\x01\x00v\x00{'descr'")).unwrap_err();
        assert!(npy.contains(".npy"), "{npy}");
    }

    #[test]
    fn error_alaw_wav_is_rejected_by_name() {
        let wav = build_riff(WAVE_FORMAT_ALAW, 1, 8000, 8, &[0x55, 0x55]);
        let err = run_default(&b64(&wav)).unwrap_err();
        assert!(err.contains("A-law"), "{err}");
    }

    #[test]
    fn error_bad_dtype() {
        let wav = b64(&wav16(16000, 1, &[0]));
        let err = run(&wav, "base64", "float16", "auto", false, false, 0, 0, "base64").unwrap_err();
        assert!(err.contains("dtype"), "{err}");
    }

    #[test]
    fn error_bad_shape() {
        let wav = b64(&wav16(16000, 1, &[0]));
        let err = run(&wav, "base64", "float32", "matrix", false, false, 0, 0, "base64").unwrap_err();
        assert!(err.contains("shape"), "{err}");
    }

    #[test]
    fn error_bad_output() {
        let wav = b64(&wav16(16000, 1, &[0]));
        let err = run(&wav, "base64", "float32", "auto", false, false, 0, 0, "npz").unwrap_err();
        assert!(err.contains("output"), "{err}");
    }

    #[test]
    fn error_bad_input_format() {
        let wav = b64(&wav16(16000, 1, &[0]));
        let err = run(&wav, "base85", "float32", "auto", false, false, 0, 0, "base64").unwrap_err();
        assert!(err.contains("input_format"), "{err}");
    }

    #[test]
    fn error_max_frames_over_cap() {
        let wav = b64(&wav16(16000, 1, &[0]));
        let err = run(
            &wav, "base64", "float32", "auto", false, false, 0, MAX_FRAMES_CAP + 1, "base64",
        )
        .unwrap_err();
        assert!(err.contains("max_frames"), "{err}");
    }

    #[test]
    fn error_start_frame_past_end() {
        let wav = b64(&wav16(16000, 1, &[0, 0, 0]));
        let err = run(&wav, "base64", "float32", "auto", false, false, 5, 0, "base64").unwrap_err();
        assert!(err.contains("start_frame"), "{err}");
        assert!(err.contains("3 sample frames"), "{err}");
    }

    #[test]
    fn error_export_over_the_output_byte_cap() {
        // 500k frames x float64 = 4 MB — under the base64 cap, over the hex one.
        let wav = b64(&wav16(44100, 1, &vec![0i16; 500_000]));
        assert!(run(
            &wav, "base64", "float64", "auto", false, false, 0, 0, "base64"
        )
        .is_ok());
        let err = run(&wav, "base64", "float64", "auto", false, false, 0, 0, "hex").unwrap_err();
        assert!(err.contains("3145728-byte (3 MiB) cap for the hex output"), "{err}");
        assert!(err.contains("output=info"), "{err}");
    }

    #[test]
    fn error_window_over_the_element_cap() {
        // 1M frames x 8 channels = 8M elements, over MAX_ELEMENTS.
        let wav = b64(&wav16(48000, 8, &vec![0i16; 8 * 600_000]));
        let err = run(&wav, "base64", "uint8", "auto", false, false, 0, 0, "base64").unwrap_err();
        assert!(err.contains("array values"), "{err}");
    }

    #[test]
    fn error_no_data_chunk() {
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&28u32.to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&WAVE_FORMAT_PCM.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&16000u32.to_le_bytes());
        out.extend_from_slice(&32000u32.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        let err = run_default(&b64(&out)).unwrap_err();
        assert!(err.contains("`data` chunk"), "{err}");
    }
}
