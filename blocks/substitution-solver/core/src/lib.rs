//! substitution-solver core — pure compute, shared by the chat skill block and
//! the web page.
//!
//! Four jobs over one monoalphabetic (simple) substitution alphabet:
//!
//! * `solve`   — recover the key automatically by hill-climbing a fixed-seed
//!               search against English letter statistics (unigram + bigram +
//!               trigram + common-word scoring). Deterministic: the same input
//!               always produces the same key, because every restart is drawn
//!               from a seeded LCG rather than a clock or system entropy.
//! * `decode`  — apply a key you already have (cipher letter → plain letter).
//! * `encode`  — apply the inverse of that key (plain letter → cipher letter).
//! * `analyze` — frequency / index-of-coincidence report plus the
//!               frequency-matched starting key, for solving by hand.
//!
//! No I/O, no clock, no randomness beyond the fixed seed.

use std::collections::HashMap;

/// Hard cap on the input, so a pasted book can't wedge the page.
pub const MAX_TEXT_CHARS: usize = 100_000;
/// Letters actually fed to the scorer during `solve`. Longer inputs are still
/// decoded in full — only the search window is capped, which bounds runtime.
pub const SCORE_WINDOW: usize = 1_200;
/// Safety valve on the hill-climb inner loop (a pass = all 325 letter swaps).
const MAX_PASSES: usize = 12;

/// Restart budget per effort level. Restart 0 is always the frequency-matched
/// key; the rest are seeded shuffles of it.
fn restarts_for(effort: &str) -> Result<usize, String> {
    match effort {
        "quick" => Ok(3),
        "standard" => Ok(15),
        "thorough" => Ok(50),
        other => Err(format!(
            "effort must be one of quick, standard, thorough (got '{other}')"
        )),
    }
}

// ---------------------------------------------------------------------------
// English statistics
// ---------------------------------------------------------------------------

/// Percentage frequency of each letter in ordinary English prose.
const UNIGRAM_PCT: [f64; 26] = [
    8.167, 1.492, 2.782, 4.253, 12.702, 2.228, 2.015, 6.094, 6.966, 0.153, 0.772, 4.025, 2.406,
    6.749, 7.507, 1.929, 0.095, 5.987, 6.327, 9.056, 2.758, 0.978, 2.360, 0.150, 1.974, 0.074,
];

/// Percentage frequency of the common English bigrams, measured over the
/// letters-only stream (so word-boundary pairs are included).
const BIGRAM_PCT: &[(&str, f64)] = &[
    ("th", 3.56),
    ("he", 3.07),
    ("in", 2.43),
    ("er", 2.05),
    ("an", 1.99),
    ("re", 1.85),
    ("on", 1.76),
    ("at", 1.49),
    ("en", 1.45),
    ("nd", 1.35),
    ("ti", 1.34),
    ("es", 1.34),
    ("or", 1.28),
    ("te", 1.20),
    ("of", 1.17),
    ("ed", 1.17),
    ("is", 1.13),
    ("it", 1.12),
    ("al", 1.09),
    ("ar", 1.07),
    ("st", 1.05),
    ("to", 1.04),
    ("nt", 1.04),
    ("ng", 0.95),
    ("se", 0.93),
    ("ha", 0.93),
    ("as", 0.87),
    ("ou", 0.87),
    ("io", 0.83),
    ("le", 0.83),
    ("ve", 0.83),
    ("co", 0.79),
    ("me", 0.79),
    ("de", 0.76),
    ("hi", 0.76),
    ("ri", 0.73),
    ("ro", 0.73),
    ("ic", 0.70),
    ("ne", 0.69),
    ("ea", 0.69),
    ("ra", 0.69),
    ("ce", 0.65),
    ("li", 0.62),
    ("ch", 0.60),
    ("ll", 0.58),
    ("be", 0.58),
    ("ma", 0.57),
    ("si", 0.55),
    ("om", 0.55),
    ("ur", 0.54),
    ("ca", 0.54),
    ("el", 0.53),
    ("ta", 0.53),
    ("la", 0.53),
    ("ns", 0.51),
    ("di", 0.50),
    ("fo", 0.49),
    ("ho", 0.46),
    ("pe", 0.45),
    ("ec", 0.45),
    ("pr", 0.45),
    ("no", 0.44),
    ("ct", 0.44),
    ("us", 0.44),
    ("ac", 0.43),
    ("ot", 0.43),
    ("il", 0.43),
    ("tr", 0.42),
    ("ly", 0.42),
    ("nc", 0.42),
    ("et", 0.41),
    ("ut", 0.40),
    ("ss", 0.40),
    ("so", 0.40),
    ("rs", 0.40),
    ("un", 0.40),
    ("lo", 0.39),
    ("wa", 0.38),
    ("ge", 0.37),
    ("ie", 0.37),
    ("wh", 0.36),
    ("ee", 0.35),
    ("wi", 0.35),
    ("em", 0.35),
    ("ad", 0.35),
    ("ol", 0.34),
    ("rt", 0.33),
    ("po", 0.33),
    ("we", 0.33),
    ("na", 0.32),
    ("ul", 0.32),
    ("ni", 0.31),
    ("ts", 0.31),
    ("mo", 0.30),
    ("ow", 0.30),
    ("pa", 0.30),
    ("im", 0.29),
    ("mi", 0.29),
    ("ai", 0.29),
    ("sh", 0.29),
    ("ir", 0.29),
    ("su", 0.29),
    ("id", 0.28),
    ("os", 0.28),
    ("iv", 0.27),
    ("ia", 0.27),
    ("am", 0.27),
    ("fi", 0.27),
    ("ci", 0.26),
    ("vi", 0.25),
    ("pl", 0.25),
    ("ig", 0.25),
    ("tu", 0.25),
    ("ev", 0.25),
    ("ld", 0.25),
    ("ry", 0.25),
    ("mp", 0.24),
    ("fe", 0.24),
    ("bl", 0.22),
    ("ab", 0.22),
    ("gh", 0.22),
    ("ty", 0.22),
    ("op", 0.22),
    ("wo", 0.21),
    ("sa", 0.21),
    ("ay", 0.21),
    ("ex", 0.21),
    ("ke", 0.21),
    ("fr", 0.21),
    ("oo", 0.21),
    ("av", 0.20),
    ("ag", 0.20),
    ("if", 0.20),
    ("ap", 0.20),
    ("gr", 0.20),
    ("od", 0.20),
    ("bo", 0.20),
    ("sp", 0.20),
    ("rd", 0.19),
    ("do", 0.19),
    ("uc", 0.19),
    ("bu", 0.19),
    ("ei", 0.19),
    ("ov", 0.19),
    ("by", 0.19),
    ("rm", 0.18),
    ("ep", 0.18),
    ("tt", 0.18),
    ("oc", 0.18),
    ("fa", 0.18),
    ("ef", 0.17),
    ("cu", 0.17),
    ("rn", 0.17),
    ("sc", 0.16),
    ("gi", 0.16),
    ("da", 0.16),
    ("yo", 0.16),
    ("cr", 0.16),
    ("cl", 0.15),
    ("du", 0.15),
    ("ga", 0.15),
    ("qu", 0.15),
    ("ue", 0.15),
    ("ff", 0.14),
    ("ba", 0.14),
    ("ey", 0.14),
    ("ls", 0.14),
    ("va", 0.14),
    ("um", 0.14),
    ("pp", 0.14),
    ("ua", 0.14),
    ("up", 0.13),
    ("lu", 0.13),
    ("go", 0.13),
    ("ht", 0.13),
    ("ru", 0.13),
    ("ug", 0.13),
    ("ds", 0.13),
    ("lt", 0.12),
    ("pi", 0.12),
    ("rc", 0.12),
    ("rr", 0.12),
    ("eg", 0.11),
    ("au", 0.11),
    ("ck", 0.11),
    ("ew", 0.11),
    ("mu", 0.11),
    ("br", 0.11),
    ("bi", 0.11),
    ("pt", 0.11),
    ("ak", 0.11),
    ("pu", 0.10),
    ("ui", 0.10),
    ("rg", 0.10),
    ("ib", 0.10),
    ("tl", 0.10),
    ("ny", 0.10),
    ("ki", 0.10),
    ("rk", 0.10),
    ("ys", 0.10),
    ("ob", 0.10),
    ("mm", 0.10),
    ("fu", 0.10),
    ("ph", 0.10),
    ("og", 0.10),
    ("ms", 0.09),
    ("ye", 0.09),
    ("ud", 0.09),
    ("mb", 0.08),
    ("ip", 0.08),
    ("ub", 0.08),
    ("oi", 0.08),
    ("rl", 0.08),
    ("gu", 0.08),
    ("dr", 0.08),
    ("hr", 0.08),
    ("cc", 0.08),
    ("tw", 0.07),
    ("ft", 0.07),
    ("wn", 0.07),
    ("nu", 0.07),
    ("af", 0.07),
    ("hu", 0.07),
    ("nn", 0.07),
    ("eo", 0.07),
    ("vo", 0.07),
    ("rv", 0.07),
    ("nf", 0.07),
    ("xp", 0.07),
    ("gn", 0.06),
    ("sm", 0.06),
    ("fl", 0.06),
    ("iz", 0.06),
    ("ok", 0.06),
    ("nl", 0.06),
    ("my", 0.06),
    ("gl", 0.06),
    ("aw", 0.06),
    ("ju", 0.05),
    ("oa", 0.05),
    ("eq", 0.05),
    ("sy", 0.05),
    ("sl", 0.05),
    ("ps", 0.05),
    ("jo", 0.05),
    ("lf", 0.05),
    ("nv", 0.05),
    ("je", 0.05),
    ("nk", 0.05),
    ("kn", 0.05),
    ("gs", 0.05),
    ("dy", 0.05),
    ("hy", 0.05),
    ("ze", 0.04),
    ("ks", 0.04),
    ("xt", 0.04),
    ("bs", 0.04),
    ("ik", 0.04),
    ("dd", 0.04),
    ("cy", 0.04),
    ("rp", 0.04),
    ("sk", 0.04),
    ("xi", 0.03),
    ("oe", 0.03),
    ("oy", 0.03),
    ("ws", 0.03),
    ("lv", 0.03),
    ("dl", 0.03),
    ("rf", 0.03),
    ("eu", 0.03),
    ("dg", 0.03),
    ("wr", 0.03),
    ("xa", 0.03),
    ("yi", 0.03),
    ("nm", 0.03),
    ("eb", 0.03),
    ("rb", 0.03),
    ("tm", 0.03),
    ("xc", 0.02),
    ("eh", 0.02),
    ("tc", 0.02),
    ("gy", 0.02),
    ("ja", 0.02),
    ("hn", 0.02),
    ("yp", 0.02),
    ("za", 0.02),
    ("gg", 0.02),
    ("ym", 0.02),
    ("sw", 0.02),
    ("bt", 0.02),
    ("nh", 0.02),
    ("ej", 0.02),
    ("nr", 0.02),
    ("rh", 0.02),
    ("ox", 0.02),
    ("yt", 0.02),
    ("hs", 0.02),
    ("ka", 0.02),
];

/// Bonus weight for the common English trigrams (same letters-only stream).
const TRIGRAM_PCT: &[(&str, f64)] = &[
    ("the", 3.51),
    ("and", 1.59),
    ("ing", 1.15),
    ("her", 0.82),
    ("hat", 0.65),
    ("his", 0.60),
    ("tha", 0.59),
    ("ere", 0.56),
    ("for", 0.55),
    ("ent", 0.53),
    ("ion", 0.51),
    ("ter", 0.46),
    ("was", 0.46),
    ("you", 0.44),
    ("ith", 0.43),
    ("ver", 0.43),
    ("all", 0.42),
    ("wit", 0.40),
    ("thi", 0.39),
    ("tio", 0.38),
    ("nde", 0.35),
    ("has", 0.33),
    ("nce", 0.33),
    ("edt", 0.33),
    ("tis", 0.33),
    ("oft", 0.32),
    ("sth", 0.32),
    ("men", 0.32),
    ("res", 0.31),
    ("ate", 0.31),
    ("ted", 0.30),
    ("hes", 0.29),
    ("eth", 0.29),
    ("dth", 0.28),
    ("est", 0.28),
    ("ont", 0.27),
    ("ers", 0.27),
    ("ati", 0.27),
    ("con", 0.26),
    ("sto", 0.26),
    ("com", 0.25),
    ("ist", 0.25),
    ("tin", 0.25),
    ("din", 0.24),
    ("out", 0.24),
    ("hav", 0.24),
    ("not", 0.24),
    ("are", 0.23),
    ("but", 0.23),
    ("hin", 0.23),
    ("one", 0.23),
    ("int", 0.22),
    ("rea", 0.22),
    ("ave", 0.22),
    ("ies", 0.21),
    ("thr", 0.20),
    ("ain", 0.20),
    ("ove", 0.19),
    ("pro", 0.19),
    ("sta", 0.19),
    ("ect", 0.19),
    ("ory", 0.18),
    ("ide", 0.18),
    ("nte", 0.18),
];

/// The most common English words — a strong signal whenever the ciphertext
/// keeps its word spacing (an "aristocrat"), inert when it doesn't.
const COMMON_WORDS: &[&str] = &[
    "a", "about", "after", "again", "all", "also", "an", "and", "any", "are", "as", "at", "back",
    "be", "because", "been", "before", "being", "between", "both", "but", "by", "call", "can",
    "come", "could", "day", "did", "do", "does", "down", "each", "even", "every", "find", "first",
    "for", "from", "get", "give", "go", "good", "great", "had", "has", "have", "he", "her", "here",
    "him", "his", "how", "i", "if", "in", "into", "is", "it", "its", "just", "know", "last",
    "less", "life", "like", "little", "long", "look", "made", "make", "man", "many", "may", "me",
    "men", "might", "more", "most", "much", "must", "my", "never", "new", "no", "not", "now",
    "number", "of", "off", "old", "on", "one", "only", "or", "other", "our", "out", "over", "own",
    "part", "people", "place", "put", "right", "said", "same", "say", "see", "she", "should",
    "since", "so", "some", "still", "such", "take", "than", "that", "the", "their", "them", "then",
    "there", "these", "they", "thing", "think", "this", "those", "though", "three", "through",
    "time", "to", "too", "two", "under", "up", "upon", "us", "use", "used", "very", "want", "was",
    "water", "way", "we", "well", "were", "what", "when", "where", "which", "while", "who", "why",
    "will", "with", "without", "work", "world", "would", "year", "yes", "yet", "you", "your",
];

/// Log10 probability floor for an n-gram the table doesn't list.
const FLOOR_PCT: f64 = 0.002;

struct Model {
    uni: [f64; 26],
    bi: Vec<f64>,
    tri: Vec<f64>,
    words: HashMap<&'static str, f64>,
}

fn idx(b: u8) -> usize {
    (b - b'a') as usize
}

impl Model {
    fn new() -> Self {
        let mut uni = [0.0f64; 26];
        for (i, pct) in UNIGRAM_PCT.iter().enumerate() {
            uni[i] = (pct / 100.0).log10();
        }
        let floor = (FLOOR_PCT / 100.0).log10();
        let mut bi = vec![floor; 26 * 26];
        for (g, pct) in BIGRAM_PCT {
            let b = g.as_bytes();
            bi[idx(b[0]) * 26 + idx(b[1])] = (pct / 100.0).log10();
        }
        let mut tri = vec![0.0f64; 26 * 26 * 26];
        for (g, pct) in TRIGRAM_PCT {
            let b = g.as_bytes();
            if b.len() == 3 && *pct > 0.0 {
                tri[idx(b[0]) * 676 + idx(b[1]) * 26 + idx(b[2])] = *pct;
            }
        }
        let words = COMMON_WORDS
            .iter()
            .map(|w| (*w, 1.0 + 0.8 * w.len() as f64))
            .collect();
        Model {
            uni,
            bi,
            tri,
            words,
        }
    }

    /// Fitness of a decoded letter stream. Bigram log-probability dominates;
    /// unigram, trigram and whole-word hits break ties between near-miss keys.
    fn score(&self, plain: &[u8], words: &[(usize, usize)], buf: &mut String) -> f64 {
        let mut total = self.bigram_score(plain) + 0.35 * self.unigram_score(plain);
        for w in plain.windows(3) {
            total += 2.0 * self.tri[w[0] as usize * 676 + w[1] as usize * 26 + w[2] as usize];
        }
        for &(a, b) in words {
            if b - a > 12 {
                continue;
            }
            buf.clear();
            buf.extend(plain[a..b].iter().map(|&c| (b'a' + c) as char));
            if let Some(bonus) = self.words.get(buf.as_str()) {
                total += bonus;
            }
        }
        total
    }

    fn bigram_score(&self, plain: &[u8]) -> f64 {
        plain
            .windows(2)
            .map(|w| self.bi[w[0] as usize * 26 + w[1] as usize])
            .sum()
    }

    fn unigram_score(&self, plain: &[u8]) -> f64 {
        plain.iter().map(|&c| self.uni[c as usize]).sum()
    }
}

// ---------------------------------------------------------------------------
// Deterministic PRNG — a fixed-seed LCG, so every run of the same input walks
// exactly the same restarts. No clock, no system entropy.
// ---------------------------------------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

// ---------------------------------------------------------------------------
// Input parsing
// ---------------------------------------------------------------------------

struct Parsed {
    chars: Vec<char>,
    /// 0..=25 for every ASCII letter in `chars`, in order.
    stream: Vec<u8>,
    /// Half-open ranges into `stream`, one per run of consecutive letters.
    words: Vec<(usize, usize)>,
}

fn parse(text: &str) -> Parsed {
    let chars: Vec<char> = text.chars().collect();
    let mut stream = Vec::with_capacity(chars.len());
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    for &c in &chars {
        if c.is_ascii_alphabetic() {
            if start.is_none() {
                start = Some(stream.len());
            }
            stream.push(c.to_ascii_uppercase() as u8 - b'A');
        } else if let Some(s) = start.take() {
            words.push((s, stream.len()));
        }
    }
    if let Some(s) = start {
        words.push((s, stream.len()));
    }
    Parsed {
        chars,
        stream,
        words,
    }
}

// ---------------------------------------------------------------------------
// Keys. A key is `[u8; 26]`: position i (cipher letter 'A'+i) holds the plain
// letter index it stands for, or `UNSET` for "not assigned".
// ---------------------------------------------------------------------------

const UNSET: u8 = 255;

fn parse_key(key: &str) -> Result<[u8; 26], String> {
    let cleaned: Vec<char> = key.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.len() != 26 {
        return Err(format!(
            "key must be 26 characters, one plaintext letter for each cipher letter A-Z \
             (got {}: '{}'). Use ? for a letter you have not worked out yet.",
            cleaned.len(),
            key.trim()
        ));
    }
    let mut out = [UNSET; 26];
    let mut seen = [false; 26];
    for (i, c) in cleaned.iter().enumerate() {
        if matches!(c, '?' | '.' | '-' | '_') {
            continue;
        }
        if !c.is_ascii_alphabetic() {
            return Err(format!(
                "key position {} (cipher letter {}) is '{}' — it must be a letter A-Z or ? for unknown",
                i + 1,
                (b'A' + i as u8) as char,
                c
            ));
        }
        let p = c.to_ascii_uppercase() as u8 - b'A';
        if seen[p as usize] {
            return Err(format!(
                "key uses the plaintext letter '{}' more than once — a substitution alphabet \
                 maps each cipher letter to a different plaintext letter",
                c.to_ascii_lowercase()
            ));
        }
        seen[p as usize] = true;
        out[i] = p;
    }
    Ok(out)
}

fn key_string(key: &[u8; 26]) -> String {
    key.iter()
        .map(|&p| if p == UNSET { '?' } else { (b'a' + p) as char })
        .collect()
}

/// plain letter → cipher letter, derived from a cipher → plain key.
fn invert(key: &[u8; 26]) -> [u8; 26] {
    let mut inv = [UNSET; 26];
    for (c, &p) in key.iter().enumerate() {
        if p != UNSET {
            inv[p as usize] = c as u8;
        }
    }
    inv
}

/// Render the text with `map` applied to every ASCII letter. `map` is indexed
/// by the source letter; `UNSET` entries are passed through unchanged.
fn apply(parsed: &Parsed, map: &[u8; 26], keep_layout: bool) -> String {
    let mut out = String::with_capacity(parsed.chars.len() + 16);
    if keep_layout {
        for &c in &parsed.chars {
            if c.is_ascii_alphabetic() {
                let src = c.to_ascii_uppercase() as u8 - b'A';
                let dst = map[src as usize];
                if dst == UNSET {
                    out.push(c);
                } else if c.is_ascii_uppercase() {
                    out.push((b'A' + dst) as char);
                } else {
                    out.push((b'a' + dst) as char);
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    // Grouped output: uppercase letters only, five to a block.
    let mut n = 0usize;
    for &s in &parsed.stream {
        let dst = map[s as usize];
        if n > 0 && n % 5 == 0 {
            out.push(' ');
        }
        out.push(if dst == UNSET {
            (b'a' + s) as char
        } else {
            (b'A' + dst) as char
        });
        n += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Cribs — "X=e, QVW=the" locks
// ---------------------------------------------------------------------------

struct Cribs {
    /// cipher index → plain index
    lock: [u8; 26],
}

fn parse_cribs(cribs: &str) -> Result<Cribs, String> {
    let mut lock = [UNSET; 26];
    let mut plain_owner = [UNSET; 26];
    for entry in cribs
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let (lhs, rhs) = entry.split_once('=').ok_or_else(|| {
            format!(
                "crib '{entry}' must look like CIPHER=plain — a single pair (X=e) or two \
                 equal-length words (QVW=the)"
            )
        })?;
        let lhs: Vec<char> = lhs.trim().chars().filter(|c| !c.is_whitespace()).collect();
        let rhs: Vec<char> = rhs.trim().chars().filter(|c| !c.is_whitespace()).collect();
        if lhs.is_empty() || rhs.is_empty() {
            return Err(format!(
                "crib '{entry}' has an empty side — write it as CIPHER=plain, e.g. X=e"
            ));
        }
        if lhs.len() != rhs.len() {
            return Err(format!(
                "crib '{entry}' pairs {} cipher letters with {} plaintext letters — both sides \
                 must be the same length",
                lhs.len(),
                rhs.len()
            ));
        }
        for (c, p) in lhs.iter().zip(rhs.iter()) {
            if !c.is_ascii_alphabetic() || !p.is_ascii_alphabetic() {
                return Err(format!(
                    "crib '{entry}' contains a non-letter — cribs map letters A-Z to letters A-Z"
                ));
            }
            let ci = (c.to_ascii_uppercase() as u8 - b'A') as usize;
            let pi = p.to_ascii_uppercase() as u8 - b'A';
            if lock[ci] != UNSET && lock[ci] != pi {
                return Err(format!(
                    "crib conflict: cipher letter {} is locked to both '{}' and '{}'",
                    (b'A' + ci as u8) as char,
                    (b'a' + lock[ci]) as char,
                    p.to_ascii_lowercase()
                ));
            }
            if plain_owner[pi as usize] != UNSET && plain_owner[pi as usize] != ci as u8 {
                return Err(format!(
                    "crib conflict: plaintext '{}' is claimed by cipher letters {} and {}",
                    p.to_ascii_lowercase(),
                    (b'A' + plain_owner[pi as usize]) as char,
                    (b'A' + ci as u8) as char
                ));
            }
            lock[ci] = pi;
            plain_owner[pi as usize] = ci as u8;
        }
    }
    Ok(Cribs { lock })
}

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

/// Observed counts of each cipher letter.
fn counts(stream: &[u8]) -> [usize; 26] {
    let mut c = [0usize; 26];
    for &b in stream {
        c[b as usize] += 1;
    }
    c
}

/// Cipher letters ordered by descending frequency, ties broken alphabetically
/// so the result is deterministic.
fn by_frequency(counts: &[usize; 26]) -> Vec<u8> {
    let mut order: Vec<u8> = (0u8..26).collect();
    order.sort_by(|&a, &b| counts[b as usize].cmp(&counts[a as usize]).then(a.cmp(&b)));
    order
}

/// English letters ordered by descending frequency: e t a o i n s h r d l ...
fn english_order() -> Vec<u8> {
    let mut order: Vec<u8> = (0u8..26).collect();
    order.sort_by(|&a, &b| {
        UNIGRAM_PCT[b as usize]
            .partial_cmp(&UNIGRAM_PCT[a as usize])
            .unwrap()
            .then(a.cmp(&b))
    });
    order
}

/// Frequency-matched starting key that honours every crib lock.
fn seed_key(counts: &[usize; 26], lock: &[u8; 26]) -> [u8; 26] {
    let mut key = *lock;
    let mut used = [false; 26];
    for &p in lock.iter() {
        if p != UNSET {
            used[p as usize] = true;
        }
    }
    let mut free_plain: Vec<u8> = english_order()
        .into_iter()
        .filter(|&p| !used[p as usize])
        .collect();
    for c in by_frequency(counts) {
        if key[c as usize] == UNSET {
            key[c as usize] = free_plain.remove(0);
        }
    }
    key
}

struct Solution {
    key: [u8; 26],
    score: f64,
}

fn hill_climb(
    model: &Model,
    stream: &[u8],
    words: &[(usize, usize)],
    lock: &[u8; 26],
    restarts: usize,
) -> Solution {
    let free: Vec<usize> = (0..26).filter(|&i| lock[i] == UNSET).collect();
    let counts = counts(stream);
    let base = seed_key(&counts, lock);
    let mut rng = Lcg(0x5EED_C0DE_u64 ^ stream.len() as u64);
    let mut buf = String::with_capacity(16);
    let mut plain = vec![0u8; stream.len()];

    let mut best = Solution {
        key: base,
        score: f64::NEG_INFINITY,
    };

    for restart in 0..restarts.max(1) {
        let mut key = base;
        if restart > 0 {
            // Fisher-Yates over the unlocked slots only, so crib locks survive.
            for i in (1..free.len()).rev() {
                let j = rng.below(i + 1);
                key.swap(free[i], free[j]);
            }
        }
        let mut score = decode_and_score(model, stream, words, &key, &mut plain, &mut buf);
        for _ in 0..MAX_PASSES {
            let mut improved = false;
            for a in 0..free.len() {
                for b in (a + 1)..free.len() {
                    key.swap(free[a], free[b]);
                    let s = decode_and_score(model, stream, words, &key, &mut plain, &mut buf);
                    if s > score {
                        score = s;
                        improved = true;
                    } else {
                        key.swap(free[a], free[b]);
                    }
                }
            }
            if !improved {
                break;
            }
        }
        if score > best.score {
            best = Solution { key, score };
        }
    }
    best
}

fn decode_and_score(
    model: &Model,
    stream: &[u8],
    words: &[(usize, usize)],
    key: &[u8; 26],
    plain: &mut [u8],
    buf: &mut String,
) -> f64 {
    for (o, &c) in plain.iter_mut().zip(stream.iter()) {
        *o = key[c as usize];
    }
    model.score(plain, words, buf)
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

/// Bigram log10-fitness per letter of a decoded stream — the standard,
/// length-independent quality measure quoted on the page.
fn fitness_per_letter(model: &Model, stream: &[u8], key: &[u8; 26]) -> f64 {
    if stream.len() < 2 {
        return 0.0;
    }
    let plain: Vec<u8> = stream
        .iter()
        .map(|&c| {
            if key[c as usize] == UNSET {
                c
            } else {
                key[c as usize]
            }
        })
        .collect();
    model.bigram_score(&plain) / (plain.len() - 1) as f64
}

/// Measured against this scoring table: ordinary English prose lands around
/// -2.35 per letter and a wrong key around -3.70. The bands below sit between
/// those two references and are pinned by
/// `fitness_bands_match_the_documented_references`.
fn confidence(fit: f64) -> &'static str {
    if fit >= -2.70 {
        "high — reads as English"
    } else if fit >= -3.10 {
        "medium — mostly right, expect a few swapped letters"
    } else {
        "low — too short, not a simple substitution, or not English"
    }
}

fn key_block(label_a: &str, label_b: &str, key: &[u8; 26]) -> String {
    format!(
        "{label_a}  ABCDEFGHIJKLMNOPQRSTUVWXYZ\n{label_b}  {}",
        key_string(key)
    )
}

fn index_of_coincidence(counts: &[usize; 26], n: usize) -> f64 {
    if n < 2 {
        return 0.0;
    }
    let num: f64 = counts
        .iter()
        .map(|&c| (c * c.saturating_sub(1)) as f64)
        .sum();
    num / (n * (n - 1)) as f64
}

fn bar(pct: f64) -> String {
    "#".repeat(((pct / 1.5).round() as usize).min(20))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run one substitution-cipher job. See the module docs for the four modes.
pub fn run(
    text: &str,
    mode: &str,
    key: &str,
    cribs: &str,
    effort: &str,
    keep_layout: bool,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err(
            "text is empty — paste the ciphertext to work on (letters A-Z are substituted, \
             everything else is left alone)"
                .into(),
        );
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!(
            "text is {} characters, over the {MAX_TEXT_CHARS} character limit — solve it in \
             sections",
            text.chars().count()
        ));
    }
    let parsed = parse(text);
    if parsed.stream.is_empty() {
        return Err("text has no A-Z letters to substitute".into());
    }
    let cribs = parse_cribs(cribs)?;
    let model = Model::new();
    let counts = counts(&parsed.stream);
    let distinct = counts.iter().filter(|&&c| c > 0).count();

    match mode {
        "solve" => {
            // Only the search window is capped; the answer is applied to the
            // whole text.
            let window = parsed.stream.len().min(SCORE_WINDOW);
            let win_words: Vec<(usize, usize)> = parsed
                .words
                .iter()
                .copied()
                .filter(|&(_, b)| b <= window)
                .collect();
            let restarts = restarts_for(effort)?;
            let sol = hill_climb(
                &model,
                &parsed.stream[..window],
                &win_words,
                &cribs.lock,
                restarts,
            );
            let fit = fitness_per_letter(&model, &parsed.stream[..window], &sol.key);
            let locked = cribs.lock.iter().filter(|&&p| p != UNSET).count();
            let mut notes = vec![
                "Only A-Z is substituted — digits, punctuation and spacing pass through unchanged."
                    .to_string(),
            ];
            if parsed.stream.len() > window {
                notes.push(format!(
                    "The key was searched on the first {window} letters and then applied to all \
                     {} — that is plenty of statistics for one alphabet.",
                    parsed.stream.len()
                ));
            }
            if distinct < 20 {
                notes.push(format!(
                    "Only {distinct} of 26 cipher letters appear, so the rest of the key is a \
                     guess from letter frequency alone."
                ));
            }
            if fit < -2.70 {
                notes.push(
                    "Fitness is below the English band: raise effort, add a crib such as X=e, \
                     or check the text really is a simple substitution."
                        .to_string(),
                );
            }
            Ok(format!(
                "Automatic solve — hill-climbing on English letter statistics\n\n\
                 Plaintext\n{}\n\n\
                 Key (cipher -> plain)\n{}\n\n\
                 Fitness {:.2} per letter (English prose ~ -2.35, wrong key ~ -3.70) · \
                 confidence: {}\n\
                 {} letters · {} distinct · {} words · effort {} ({} restarts) · \
                 {} letters locked by cribs\n\n\
                 Notes\n- {}",
                apply(&parsed, &sol.key, keep_layout),
                key_block("cipher", "plain ", &sol.key),
                fit,
                confidence(fit),
                parsed.stream.len(),
                distinct,
                parsed.words.len(),
                effort,
                restarts,
                locked,
                notes.join("\n- ")
            ))
        }
        "decode" | "encode" => {
            if key.trim().is_empty() {
                return Err(format!(
                    "mode {mode} needs a key — pass 26 letters giving the plaintext letter that \
                     each cipher letter A-Z stands for, e.g. \
                     zebrascdfghijklmnopqtuvwxy (use ? for unknown), or switch to mode=solve"
                ));
            }
            let k = parse_key(key)?;
            let unassigned = k.iter().filter(|&&p| p == UNSET).count();
            let map = if mode == "decode" { k } else { invert(&k) };
            let fit = fitness_per_letter(&model, &parsed.stream, &map);
            let body = apply(&parsed, &map, keep_layout);
            let unknown_note = if unassigned == 0 {
                String::new()
            } else {
                format!(
                    "\n{unassigned} cipher letters are unassigned (?) and were left as-is, \
                     in lower case."
                )
            };
            if mode == "decode" {
                Ok(format!(
                    "Decoded with the key you supplied\n\n\
                     Plaintext\n{body}\n\n\
                     Key (cipher -> plain)\n{}\n\n\
                     Fitness {fit:.2} per letter (English prose ~ -2.35, wrong key ~ -3.70) · \
                     confidence: {}\n\
                     {} letters · {} distinct · {} words{unknown_note}",
                    key_block("cipher", "plain ", &k),
                    confidence(fit),
                    parsed.stream.len(),
                    distinct,
                    parsed.words.len(),
                ))
            } else {
                Ok(format!(
                    "Encoded with the key you supplied\n\n\
                     Ciphertext\n{body}\n\n\
                     Key (plain -> cipher)\n{}\n\n\
                     {} letters · {} distinct · {} words{unknown_note}",
                    key_block("plain ", "cipher", &map),
                    parsed.stream.len(),
                    distinct,
                    parsed.words.len(),
                ))
            }
        }
        "analyze" => {
            let n = parsed.stream.len();
            let ic = index_of_coincidence(&counts, n);
            let mut rows: Vec<(u8, usize)> = (0u8..26).map(|c| (c, counts[c as usize])).collect();
            rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let mut freq = String::new();
            for (c, k) in rows.iter().filter(|r| r.1 > 0) {
                let pct = *k as f64 * 100.0 / n as f64;
                freq.push_str(&format!(
                    "{}  {:>5}  {:>5.2}%  {}\n",
                    (b'A' + c) as char,
                    k,
                    pct,
                    bar(pct)
                ));
            }
            let mut bigrams: HashMap<(u8, u8), usize> = HashMap::new();
            for w in parsed.stream.windows(2) {
                *bigrams.entry((w[0], w[1])).or_insert(0) += 1;
            }
            let mut bg: Vec<((u8, u8), usize)> = bigrams.into_iter().filter(|e| e.1 > 1).collect();
            bg.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            bg.truncate(10);
            let top_bigrams = if bg.is_empty() {
                "(none repeated)".to_string()
            } else {
                bg.iter()
                    .map(|((a, b), k)| {
                        format!("{}{} {}", (b'A' + a) as char, (b'A' + b) as char, k)
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            let seed = seed_key(&counts, &cribs.lock);
            Ok(format!(
                "Frequency analysis\n\n\
                 {n} letters · {distinct} distinct · {} words · {} other characters\n\
                 Index of coincidence {ic:.4} (English ~ 0.0667, random ~ 0.0385) — \
                 {}\n\n\
                 Letter frequency\n{freq}\n\
                 Repeated bigrams\n{top_bigrams}\n\n\
                 Frequency-matched starting key (cipher -> plain)\n{}\n\n\
                 Feed this key back in with mode=decode to test it by hand, or run mode=solve \
                 to let the hill-climber refine it.",
                parsed.words.len(),
                parsed.chars.len() - n,
                if ic >= 0.055 {
                    "consistent with a one-alphabet substitution"
                } else {
                    "flatter than English — this may be a polyalphabetic cipher, not a simple substitution"
                },
                key_block("cipher", "plain ", &seed),
            ))
        }
        other => Err(format!(
            "mode must be one of solve, decode, encode, analyze (got '{other}')"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CIPHER: &str = "GSVJFRXPYILDMULCQFNKHLEVIGSVOZABWLTGSVJFRXPYILDMULCQFNKHLEVIGSVOZABWLT";

    fn encode_with(plain: &str, keyword_key: &str) -> String {
        // keyword_key is a plain -> cipher alphabet.
        plain
            .chars()
            .map(|c| {
                if c.is_ascii_alphabetic() {
                    let i = (c.to_ascii_lowercase() as u8 - b'a') as usize;
                    keyword_key.as_bytes()[i] as char
                } else {
                    c
                }
            })
            .collect()
    }

    #[test]
    fn solve_recovers_a_real_cryptogram() {
        // Atbash is a monoalphabetic substitution, so it is fair game as a
        // fixture; the solver knows nothing about it.
        let plain = "the quick brown fox jumps over the lazy dog while the government of the \
                     people by the people and for the people shall not perish from the earth \
                     because every letter must appear often enough to be counted";
        let atbash: String = (0..26).map(|i| (b'z' - i) as char).collect();
        let ct = encode_with(plain, &atbash);
        let out = run(&ct, "solve", "", "", "standard", true).unwrap();
        assert!(
            out.contains("the quic")
                && out.contains("brown fox jumps over")
                && out.contains("people and for the people"),
            "solver did not recover readable plaintext:\n{out}"
        );
        assert!(out.contains("confidence: high"), "{out}");
    }

    #[test]
    fn solve_is_deterministic() {
        let a = run(CIPHER, "solve", "", "", "quick", true).unwrap();
        let b = run(CIPHER, "solve", "", "", "quick", true).unwrap();
        assert_eq!(a, b, "the same input must always produce the same key");
    }

    #[test]
    fn cribs_lock_letters_in_place() {
        let out = run(CIPHER, "solve", "", "G=t, S=h, V=e", "quick", true).unwrap();
        let line = out
            .lines()
            .find(|l| l.starts_with("plain "))
            .expect("key block");
        let k = line.trim_start_matches("plain ").trim();
        assert_eq!(k.as_bytes()[(b'G' - b'A') as usize], b't', "{out}");
        assert_eq!(k.as_bytes()[(b'S' - b'A') as usize], b'h', "{out}");
        assert_eq!(k.as_bytes()[(b'V' - b'A') as usize], b'e', "{out}");
    }

    #[test]
    fn decode_applies_a_supplied_key() {
        let atbash: String = (0..26).map(|i| (b'z' - i) as char).collect();
        let out = run(
            "Gsv jfrxp yildm ulc.",
            "decode",
            &atbash,
            "",
            "standard",
            true,
        )
        .unwrap();
        assert!(out.contains("The quick brown fox."), "{out}");
        assert!(out.contains("Decoded with the key you supplied"), "{out}");
    }

    #[test]
    fn encode_is_the_inverse_of_decode() {
        let atbash: String = (0..26).map(|i| (b'z' - i) as char).collect();
        let enc = run(
            "The quick brown fox.",
            "encode",
            &atbash,
            "",
            "standard",
            true,
        )
        .unwrap();
        assert!(enc.contains("Gsv jfrxp yildm ulc."), "{enc}");
    }

    #[test]
    fn unknown_key_letters_pass_through() {
        let mut k: Vec<char> = (0..26).map(|i| (b'z' - i) as char).collect();
        k[0] = '?';
        let key: String = k.into_iter().collect();
        let out = run("Abc", "decode", &key, "", "standard", true).unwrap();
        assert!(out.contains("Ayx"), "{out}");
        assert!(out.contains("1 cipher letters are unassigned"), "{out}");
    }

    #[test]
    fn grouped_output_drops_layout() {
        let atbash: String = (0..26).map(|i| (b'z' - i) as char).collect();
        let out = run("Gsv jfrxp!", "decode", &atbash, "", "standard", false).unwrap();
        assert!(out.contains("THEQU ICK"), "{out}");
    }

    #[test]
    fn analyze_reports_frequencies_and_ic() {
        let out = run(CIPHER, "analyze", "", "", "standard", true).unwrap();
        assert!(out.contains("Frequency analysis"), "{out}");
        assert!(out.contains("Index of coincidence"), "{out}");
        assert!(out.contains("Frequency-matched starting key"), "{out}");
    }

    #[test]
    fn empty_text_is_an_error() {
        let e = run("   ", "solve", "", "", "standard", true).unwrap_err();
        assert!(e.contains("text is empty"), "{e}");
    }

    #[test]
    fn text_without_letters_is_an_error() {
        let e = run("1234 5678!", "solve", "", "", "standard", true).unwrap_err();
        assert!(e.contains("no A-Z letters"), "{e}");
    }

    #[test]
    fn unknown_mode_is_an_error() {
        let e = run("abc", "crack", "", "", "standard", true).unwrap_err();
        assert!(
            e.contains("mode must be one of solve, decode, encode, analyze"),
            "{e}"
        );
    }

    #[test]
    fn unknown_effort_is_an_error() {
        let e = run("abcdef", "solve", "", "", "turbo", true).unwrap_err();
        assert!(e.contains("effort must be one of"), "{e}");
    }

    #[test]
    fn decode_without_a_key_is_an_error() {
        let e = run("abc", "decode", "", "", "standard", true).unwrap_err();
        assert!(e.contains("needs a key"), "{e}");
    }

    #[test]
    fn short_key_is_an_error() {
        let e = run("abc", "decode", "abc", "", "standard", true).unwrap_err();
        assert!(e.contains("key must be 26 characters"), "{e}");
    }

    #[test]
    fn duplicate_key_letter_is_an_error() {
        let e = run(
            "abc",
            "decode",
            "aabcdefghijklmnopqrstuvwxy",
            "",
            "standard",
            true,
        )
        .unwrap_err();
        assert!(e.contains("more than once"), "{e}");
    }

    #[test]
    fn non_letter_in_key_is_an_error() {
        let e = run(
            "abc",
            "decode",
            "1bcdefghijklmnopqrstuvwxyz",
            "",
            "standard",
            true,
        )
        .unwrap_err();
        assert!(e.contains("must be a letter A-Z or ?"), "{e}");
    }

    #[test]
    fn malformed_crib_is_an_error() {
        let e = run("abcdef", "solve", "", "XY", "quick", true).unwrap_err();
        assert!(e.contains("must look like CIPHER=plain"), "{e}");
    }

    #[test]
    fn uneven_crib_is_an_error() {
        let e = run("abcdef", "solve", "", "QVW=then", "quick", true).unwrap_err();
        assert!(e.contains("both sides must be the same length"), "{e}");
    }

    #[test]
    fn conflicting_cribs_are_an_error() {
        let e = run("abcdef", "solve", "", "X=e, X=t", "quick", true).unwrap_err();
        assert!(e.contains("locked to both"), "{e}");
    }

    #[test]
    fn reused_plaintext_crib_is_an_error() {
        let e = run("abcdef", "solve", "", "X=e, Q=e", "quick", true).unwrap_err();
        assert!(e.contains("claimed by cipher letters"), "{e}");
    }

    #[test]
    fn oversized_text_is_an_error() {
        let big = "a".repeat(MAX_TEXT_CHARS + 1);
        let e = run(&big, "analyze", "", "", "standard", true).unwrap_err();
        assert!(e.contains("over the"), "{e}");
    }

    #[test]
    fn word_cribs_expand_to_letter_locks() {
        let c = parse_cribs("QVW=the").unwrap();
        assert_eq!(c.lock[(b'Q' - b'A') as usize], b't' - b'a');
        assert_eq!(c.lock[(b'V' - b'A') as usize], b'h' - b'a');
        assert_eq!(c.lock[(b'W' - b'A') as usize], b'e' - b'a');
    }

    /// Calibration guard for the fitness bands quoted on the page: real English
    /// must land in the "high" band and a scrambled key must not.
    #[test]
    fn fitness_bands_match_the_documented_references() {
        let model = Model::new();
        let english = "it was the best of times it was the worst of times it was the age of \
                       wisdom it was the age of foolishness";
        let p = parse(english);
        let identity: [u8; 26] = std::array::from_fn(|i| i as u8);
        let fit = fitness_per_letter(&model, &p.stream, &identity);
        assert!(
            fit > -2.70,
            "English prose scored {fit}, outside the 'high' band"
        );
        let rotated: [u8; 26] = std::array::from_fn(|i| ((i + 13) % 26) as u8);
        let bad = fitness_per_letter(&model, &p.stream, &rotated);
        assert!(
            bad < -3.10,
            "a wrong key scored {bad}, inside the 'medium' band"
        );
    }
}

#[cfg(test)]
mod demo {
    use super::*;
    const PLAIN: &str = "The greatest glory in living lies not in never falling, but in rising every time we fall. What we know matters, but who we are matters more. A person is not measured by the moments of comfort, but by the moments of challenge.";
    #[test]
    fn make_demo() {
        // ZEBRAS keyword alphabet, plain -> cipher.
        let k = "zebrascdfghijklmnopqtuvwxy";
        // decode key (cipher -> plain) = inverse
        let mut inv = [0u8; 26];
        for (i, c) in k.bytes().enumerate() {
            inv[(c - b'a') as usize] = i as u8;
        }
        let invs: String = inv.iter().map(|&p| (b'a' + p) as char).collect();
        let ct: String = PLAIN
            .chars()
            .map(|c| {
                if c.is_ascii_alphabetic() {
                    let i = (c.to_ascii_lowercase() as u8 - b'a') as usize;
                    let o = k.as_bytes()[i];
                    if c.is_ascii_uppercase() {
                        o.to_ascii_uppercase() as char
                    } else {
                        o as char
                    }
                } else {
                    c
                }
            })
            .collect();
        println!("DEMO_CT<<{ct}>>");
        println!("DEMO_DECODE_KEY<<{invs}>>");
        for eff in ["quick", "standard", "thorough"] {
            let out = run(&ct, "solve", "", "", eff, true).unwrap();
            let pt = out.lines().nth(3).unwrap();
            println!("DEMO_SOLVE[{eff}]<<{pt}>>");
            println!(
                "DEMO_FIT[{eff}]<<{}>>",
                out.lines().find(|l| l.starts_with("Fitness")).unwrap()
            );
        }
        println!(
            "FULL_SOLVE<<<{}>>>",
            run(&ct, "solve", "", "", "quick", true).unwrap()
        );
        println!(
            "FULL_DECODE<<<{}>>>",
            run(&ct, "decode", &invs, "", "standard", true).unwrap()
        );
        println!(
            "FULL_ANALYZE<<<{}>>>",
            run(
                "Qda coazqapq cilox fk ifufkc ifap klq fk kauao sziifkc.",
                "analyze",
                "",
                "",
                "standard",
                true
            )
            .unwrap()
        );
        println!(
            "FULL_ENCODE<<<{}>>>",
            run(
                "Meet me at the old bridge at dawn.",
                "encode",
                &invs,
                "",
                "standard",
                true
            )
            .unwrap()
        );
        println!(
            "FULL_GROUP<<<{}>>>",
            run(
                "Qda coazqapq cilox.",
                "decode",
                &invs,
                "",
                "standard",
                false
            )
            .unwrap()
        );
    }
}
