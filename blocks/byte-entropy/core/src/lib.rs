//! byte-entropy core — pure, dependency-free Shannon-entropy analysis of a byte
//! buffer. Computes the overall entropy (bits/byte, 0–8) plus a per-block
//! breakdown so high-entropy regions (encrypted/compressed/packed data) can be
//! told apart from low-entropy ones (text, padding, sparse data). No I/O, no
//! deps — runs on every backend including the chat Service Worker.

/// Shannon entropy in bits per byte (0.0–8.0) over `data`. An empty slice has
/// entropy 0.0 by convention.
pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    let mut h = 0.0f64;
    for &c in counts.iter() {
        if c == 0 {
            continue;
        }
        let p = c as f64 / len;
        h -= p * p.log2();
    }
    // Guard against tiny negative noise from floating-point rounding.
    if h < 0.0 {
        0.0
    } else {
        h
    }
}

/// One fixed-size window of the input and its Shannon entropy.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Byte offset where this block begins.
    pub offset: usize,
    /// Number of bytes in this block (the final block may be shorter).
    pub size: usize,
    /// Shannon entropy of this block, bits/byte (0.0–8.0).
    pub entropy: f64,
}

/// A full entropy report for a buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Total number of bytes analysed.
    pub total_bytes: usize,
    /// Shannon entropy over the whole buffer, bits/byte (0.0–8.0).
    pub overall_entropy: f64,
    /// Count of distinct byte values (0–256) that appear at least once.
    pub distinct_bytes: usize,
    /// Block size used for the per-block breakdown.
    pub block_size: usize,
    /// Per-block entropy, in order.
    pub blocks: Vec<Block>,
    /// Minimum block entropy (None when there are no blocks, i.e. empty input).
    pub min_block_entropy: Option<f64>,
    /// Maximum block entropy (None when there are no blocks).
    pub max_block_entropy: Option<f64>,
    /// A short human-readable classification of the overall entropy.
    pub assessment: String,
}

/// Largest accepted block size (4 MiB) — keeps the per-block vector bounded.
pub const MAX_BLOCK_SIZE: usize = 4 * 1024 * 1024;
/// Smallest accepted block size.
pub const MIN_BLOCK_SIZE: usize = 16;
/// Default block size when the caller does not specify one.
pub const DEFAULT_BLOCK_SIZE: usize = 256;

/// Coarse English label for an overall entropy value (bits/byte).
fn assess(h: f64) -> &'static str {
    if h >= 7.5 {
        "very high — likely encrypted, compressed, or otherwise random data"
    } else if h >= 6.5 {
        "high — likely compressed or packed data, or a media container"
    } else if h >= 4.0 {
        "moderate — mixed or structured binary data"
    } else if h >= 1.0 {
        "low — likely text, code, or repetitive structured data"
    } else {
        "very low — highly repetitive (padding, runs, or near-constant bytes)"
    }
}

/// Analyse `data`, splitting it into `block_size`-byte windows. `block_size` is
/// clamped into `[MIN_BLOCK_SIZE, MAX_BLOCK_SIZE]`.
pub fn analyze(data: &[u8], block_size: usize) -> Report {
    let block_size = block_size.clamp(MIN_BLOCK_SIZE, MAX_BLOCK_SIZE);

    let overall_entropy = shannon_entropy(data);

    let mut seen = [false; 256];
    for &b in data {
        seen[b as usize] = true;
    }
    let distinct_bytes = seen.iter().filter(|&&s| s).count();

    let mut blocks = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        let end = (offset + block_size).min(data.len());
        let chunk = &data[offset..end];
        blocks.push(Block {
            offset,
            size: chunk.len(),
            entropy: shannon_entropy(chunk),
        });
        offset = end;
    }

    let (min_block_entropy, max_block_entropy) = if blocks.is_empty() {
        (None, None)
    } else {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for b in &blocks {
            if b.entropy < lo {
                lo = b.entropy;
            }
            if b.entropy > hi {
                hi = b.entropy;
            }
        }
        (Some(lo), Some(hi))
    };

    Report {
        total_bytes: data.len(),
        overall_entropy,
        distinct_bytes,
        block_size,
        blocks,
        min_block_entropy,
        max_block_entropy,
        assessment: assess(overall_entropy).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(shannon_entropy(&[]), 0.0);
        let r = analyze(&[], DEFAULT_BLOCK_SIZE);
        assert_eq!(r.total_bytes, 0);
        assert_eq!(r.overall_entropy, 0.0);
        assert_eq!(r.distinct_bytes, 0);
        assert!(r.blocks.is_empty());
        assert_eq!(r.min_block_entropy, None);
        assert_eq!(r.max_block_entropy, None);
    }

    #[test]
    fn constant_buffer_is_zero_entropy() {
        let data = vec![0x41u8; 1000];
        assert!(close(shannon_entropy(&data), 0.0));
        let r = analyze(&data, 256);
        assert!(close(r.overall_entropy, 0.0));
        assert_eq!(r.distinct_bytes, 1);
    }

    #[test]
    fn uniform_all_256_values_is_eight_bits() {
        // Every byte value exactly once → maximum entropy of 8 bits/byte.
        let data: Vec<u8> = (0..=255u8).collect();
        assert!(close(shannon_entropy(&data), 8.0));
    }

    #[test]
    fn two_equally_likely_values_is_one_bit() {
        // 0x00 and 0xFF in equal proportion → 1 bit/byte.
        let mut data = vec![0u8; 500];
        data.extend(std::iter::repeat(0xFFu8).take(500));
        assert!(close(shannon_entropy(&data), 1.0));
    }

    #[test]
    fn block_split_offsets_and_sizes() {
        let data = vec![0u8; 1000];
        let r = analyze(&data, 256);
        assert_eq!(r.block_size, 256);
        // 1000 / 256 = 3 full + 1 partial (232).
        assert_eq!(r.blocks.len(), 4);
        assert_eq!(r.blocks[0].offset, 0);
        assert_eq!(r.blocks[0].size, 256);
        assert_eq!(r.blocks[3].offset, 768);
        assert_eq!(r.blocks[3].size, 232);
        // Sum of block sizes equals the total.
        let sum: usize = r.blocks.iter().map(|b| b.size).sum();
        assert_eq!(sum, r.total_bytes);
    }

    #[test]
    fn block_size_is_clamped() {
        let data = vec![0u8; 100];
        // Too small → clamped up to MIN.
        let r = analyze(&data, 1);
        assert_eq!(r.block_size, MIN_BLOCK_SIZE);
        // Too large → clamped down to MAX.
        let r2 = analyze(&data, usize::MAX);
        assert_eq!(r2.block_size, MAX_BLOCK_SIZE);
    }

    #[test]
    fn min_max_block_entropy_bracket() {
        // Half constant (entropy 0), half all-256-values (entropy 8).
        let mut data = vec![0u8; 256];
        data.extend(0..=255u8);
        let r = analyze(&data, 256);
        assert_eq!(r.blocks.len(), 2);
        assert!(close(r.min_block_entropy.unwrap(), 0.0));
        assert!(close(r.max_block_entropy.unwrap(), 8.0));
    }

    #[test]
    fn assessments_span_the_range() {
        assert_eq!(assess(8.0), assess(7.9)); // both "very high"
        assert!(assess(0.0).starts_with("very low"));
        assert!(assess(2.0).starts_with("low"));
        assert!(assess(5.0).starts_with("moderate"));
        assert!(assess(7.0).starts_with("high"));
        assert!(assess(7.99).starts_with("very high"));
    }
}
