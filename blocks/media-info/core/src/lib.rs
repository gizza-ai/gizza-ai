//! gizza-ai/media-info core — read container + per-track codec/stream metadata
//! from an audio or video file's bytes, using the pure-Rust `symphonia` demuxers
//! (no decoding, no I/O, no ffmpeg). Reports the container format, total
//! duration, overall bitrate, and a per-track summary (codec, sample rate,
//! channel count/layout, bit depth, frame count). No std clock / no FS deps, so
//! it instantiates in the wafer chat runtime as well as the CLI.

use std::io::Cursor;
use symphonia::core::codecs::CodecType;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// One media track (audio stream; symphonia's metadata demuxers expose audio
/// tracks — video tracks in a container appear with a video codec type and no
/// audio fields).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    /// 1-based index in the reported order.
    pub index: usize,
    /// Friendly codec name, e.g. "AAC", "FLAC", "MP3", "PCM".
    pub codec: String,
    /// "audio" or "video" or "other".
    pub kind: String,
    /// Sampling rate in Hz, when known.
    pub sample_rate: Option<u32>,
    /// Channel count, when known.
    pub channels: Option<u32>,
    /// Human channel layout, e.g. "mono", "stereo", "5.1", or "3 channels".
    pub channel_layout: Option<String>,
    /// Bits per coded sample, when known.
    pub bits_per_sample: Option<u32>,
    /// Duration of this track in seconds, when derivable from frames + rate.
    pub duration_seconds: Option<f64>,
}

/// Top-level media metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// Friendly container name, e.g. "MP4 / M4A (ISO BMFF)", "Matroska / WebM".
    pub container: String,
    /// Longest track duration in seconds, when known.
    pub duration_seconds: Option<f64>,
    /// Human-readable duration, e.g. "1:23.450" (omitted when unknown).
    pub duration_human: Option<String>,
    /// Overall bitrate in kbit/s, derived from file size / duration when known.
    pub overall_bitrate_kbps: Option<u32>,
    pub tracks: Vec<TrackInfo>,
    pub track_count: usize,
}

fn channel_layout(n: u32) -> String {
    match n {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        3 => "2.1".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        other => format!("{other} channels"),
    }
}

/// Map a symphonia `CodecType` to a friendly name + coarse kind ("audio"/"video").
fn codec_name(c: CodecType) -> (String, &'static str) {
    use symphonia::core::codecs::*;
    if c == CODEC_TYPE_NULL {
        // symphonia's metadata demuxers expose tracks they can't decode (e.g. a
        // container's video stream) as the null codec — report it honestly
        // rather than mislabelling it as a known audio codec.
        return ("unsupported/other".to_string(), "other");
    }
    let name = match c {
        CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE | CODEC_TYPE_PCM_S24LE | CODEC_TYPE_PCM_S24BE
        | CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE | CODEC_TYPE_PCM_S8 | CODEC_TYPE_PCM_U8
        | CODEC_TYPE_PCM_U16LE | CODEC_TYPE_PCM_U16BE | CODEC_TYPE_PCM_U24LE
        | CODEC_TYPE_PCM_U24BE | CODEC_TYPE_PCM_U32LE | CODEC_TYPE_PCM_U32BE
        | CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE | CODEC_TYPE_PCM_F64LE
        | CODEC_TYPE_PCM_F64BE => "PCM",
        CODEC_TYPE_ADPCM_MS | CODEC_TYPE_ADPCM_IMA_WAV => "ADPCM",
        CODEC_TYPE_VORBIS => "Vorbis",
        CODEC_TYPE_MP1 => "MP1",
        CODEC_TYPE_MP2 => "MP2",
        CODEC_TYPE_MP3 => "MP3",
        CODEC_TYPE_AAC => "AAC",
        CODEC_TYPE_FLAC => "FLAC",
        CODEC_TYPE_ALAC => "ALAC",
        CODEC_TYPE_OPUS => "Opus",
        _ => "",
    };
    if !name.is_empty() {
        return (name.to_string(), "audio");
    }
    (format!("codec {c}"), "other")
}

/// Friendly container name from the format reader's reported short name.
fn container_name(short: &str) -> String {
    match short {
        "isomp4" => "MP4 / M4A (ISO BMFF)",
        "matroska" | "mkv" | "webm" => "Matroska / WebM",
        "ogg" => "OGG",
        "wave" | "wav" | "riff" => "WAVE (RIFF)",
        "aiff" => "AIFF",
        "caf" => "Core Audio (CAF)",
        "flac" => "FLAC (native)",
        "mp3" | "mpa" => "MP3 (MPEG audio)",
        "adts" | "aac" => "AAC (ADTS)",
        "media" => "media",
        other => return other.to_string(),
    }
    .to_string()
}

fn human_duration(secs: f64) -> String {
    let total = secs.max(0.0);
    let h = (total / 3600.0).floor() as u64;
    let m = ((total % 3600.0) / 60.0).floor() as u64;
    let s = total % 60.0;
    if h > 0 {
        format!("{h}:{m:02}:{s:06.3}")
    } else {
        format!("{m}:{s:06.3}")
    }
}

/// Inspect media `bytes`. Errors if the container is unrecognised by any of the
/// enabled demuxers.
pub fn info(bytes: &[u8]) -> Result<MediaInfo, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let total_bytes = bytes.len();
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());

    let hint = Hint::new();
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unrecognised or unsupported media container: {e}"))?;

    let reader = probed.format;

    // symphonia 0.5 doesn't surface the container short-name via a public method,
    // so re-sniff the container for the friendly label.
    let container = container_name(sniff_container(bytes));

    let mut tracks = Vec::new();
    let mut max_duration: Option<f64> = None;

    for (i, t) in reader.tracks().iter().enumerate() {
        let p = &t.codec_params;
        let (codec, kind) = codec_name(p.codec);
        let sample_rate = p.sample_rate;
        let channels = p.channels.map(|c| c.count() as u32);
        let channel_layout = channels.map(channel_layout);
        let bits_per_sample = p.bits_per_sample;

        let duration_seconds = match (p.n_frames, p.sample_rate) {
            (Some(frames), Some(rate)) if rate > 0 => Some(frames as f64 / rate as f64),
            _ => p
                .time_base
                .zip(p.n_frames)
                .map(|(tb, frames)| frames as f64 * tb.numer as f64 / tb.denom as f64),
        };
        if let Some(d) = duration_seconds {
            max_duration = Some(max_duration.map_or(d, |m: f64| m.max(d)));
        }

        tracks.push(TrackInfo {
            index: i + 1,
            codec,
            kind: kind.to_string(),
            sample_rate,
            channels,
            channel_layout,
            bits_per_sample,
            duration_seconds,
        });
    }

    if tracks.is_empty() {
        return Err("no media tracks found in the container".into());
    }

    let overall_bitrate_kbps = max_duration
        .filter(|d| *d > 0.0)
        .map(|d| ((total_bytes as f64 * 8.0) / d / 1000.0).round() as u32);

    Ok(MediaInfo {
        container,
        duration_seconds: max_duration,
        duration_human: max_duration.map(human_duration),
        overall_bitrate_kbps,
        track_count: tracks.len(),
        tracks,
    })
}

/// Lightweight container sniff for the friendly name (symphonia hides the short
/// name). Falls back to "media" when uncertain.
fn sniff_container(b: &[u8]) -> &'static str {
    if b.len() < 12 {
        return "media";
    }
    if &b[4..8] == b"ftyp" {
        return "isomp4";
    }
    if b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return "matroska";
    }
    if b.starts_with(b"OggS") {
        return "ogg";
    }
    if b.starts_with(b"RIFF") && &b[8..12] == b"WAVE" {
        return "wave";
    }
    if b.starts_with(b"FORM") && (&b[8..12] == b"AIFF" || &b[8..12] == b"AIFC") {
        return "aiff";
    }
    if b.starts_with(b"caff") {
        return "caf";
    }
    if b.starts_with(b"fLaC") {
        return "flac";
    }
    if b.starts_with(b"ID3") || (b[0] == 0xFF && (b[1] & 0xE0) == 0xE0) {
        return "mp3";
    }
    "media"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_layouts() {
        assert_eq!(channel_layout(1), "mono");
        assert_eq!(channel_layout(2), "stereo");
        assert_eq!(channel_layout(6), "5.1");
        assert_eq!(channel_layout(5), "5 channels");
    }

    #[test]
    fn human_duration_formats() {
        assert_eq!(human_duration(83.45), "1:23.450");
        assert!(human_duration(3661.0).starts_with("1:01:01"));
    }

    #[test]
    fn sniff_recognises_wav_and_flac() {
        let mut wav = b"RIFF\x00\x00\x00\x00WAVE".to_vec();
        wav.extend_from_slice(&[0u8; 4]);
        assert_eq!(sniff_container(&wav), "wave");
        assert_eq!(sniff_container(b"fLaC\x00\x00\x00\x00\x00\x00\x00\x00"), "flac");
    }

    #[test]
    fn errors_on_empty_and_garbage() {
        assert!(info(&[]).is_err());
        assert!(info(b"not a media file at all, just text bytes here xx").is_err());
    }

    #[test]
    fn reads_a_real_wav() {
        // Minimal valid 8-bit mono PCM WAV: 44-byte header + 4 sample bytes.
        let sample_rate: u32 = 8000;
        let data: [u8; 4] = [128, 130, 126, 128];
        let byte_rate = sample_rate; // 1 ch * 8-bit
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
        w.extend_from_slice(b"WAVE");
        w.extend_from_slice(b"fmt ");
        w.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&sample_rate.to_le_bytes());
        w.extend_from_slice(&byte_rate.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // block align
        w.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(data.len() as u32).to_le_bytes());
        w.extend_from_slice(&data);

        let info = info(&w).unwrap();
        assert_eq!(info.container, "WAVE (RIFF)");
        assert_eq!(info.track_count, 1);
        let t = &info.tracks[0];
        assert_eq!(t.codec, "PCM");
        assert_eq!(t.sample_rate, Some(8000));
        assert_eq!(t.channels, Some(1));
        assert_eq!(t.channel_layout.as_deref(), Some("mono"));
    }
}
