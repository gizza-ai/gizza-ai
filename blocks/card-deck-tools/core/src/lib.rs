//! card-deck-tools core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Shuffles a standard playing-card deck and either lists the shuffled order,
//! deals hands round-robin to N players, or draws N cards off the top. The deck
//! can be 1–8 stacked decks with 0–2 jokers each.
//!
//! Randomness is a tiny in-house seeded PRNG (splitmix64) driving an unbiased
//! Fisher–Yates shuffle, so every surface — chat, CLI, page, tests — produces the
//! same deck for the same `seed`; change `seed` for a different shuffle. No OS
//! RNG / `getrandom`, and deliberately NOT cryptographically secure: this is
//! reproducibility, not secrecy.

/// splitmix64 — a tiny deterministic PRNG. `next_u64()` yields a u64 stream.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid a zero state producing a degenerate stream.
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform integer in `[0, n)` via rejection sampling — no modulo bias, so
    /// every deck permutation stays equally likely.
    fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        let limit = u64::MAX - (u64::MAX % n);
        loop {
            let v = self.next_u64();
            if v < limit {
                return v % n;
            }
        }
    }
}

/// Turn a seed string into the PRNG state: a plain whole number is used as-is,
/// anything else is hashed (FNV-1a 64) so `"table-3"` is a valid seed too.
fn seed_to_u64(seed: &str) -> u64 {
    let s = seed.trim();
    if let Ok(n) = s.parse::<u64>() {
        return n;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Build order within one deck: A,2..10,J,Q,K per suit.
const RANKS_SHORT: [&str; 13] = [
    "A", "2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K",
];
const RANKS_SYMBOL: [&str; 13] = [
    "A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K",
];
const RANKS_LONG: [&str; 13] = [
    "Ace", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten", "Jack", "Queen",
    "King",
];
/// Rank names used inside poker-ranking labels ("Pair of 7s", "Ace-high").
const RANKS_LABEL: [&str; 13] = [
    "Ace", "2", "3", "4", "5", "6", "7", "8", "9", "10", "Jack", "Queen", "King",
];
const SUITS_SHORT: [&str; 4] = ["S", "H", "D", "C"];
const SUITS_SYMBOL: [&str; 4] = ["\u{2660}", "\u{2665}", "\u{2666}", "\u{2663}"];
const SUITS_LONG: [&str; 4] = ["Spades", "Hearts", "Diamonds", "Clubs"];

const JOKER: u8 = 255;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Card {
    /// 0..=12 (A,2..10,J,Q,K) or `JOKER`.
    rank: u8,
    /// 0..=3 (S,H,D,C); 0 for a joker.
    suit: u8,
}

impl Card {
    fn is_joker(&self) -> bool {
        self.rank == JOKER
    }
    /// Ace-high ordering value: 2..=14 for pips/faces, 15 for a joker.
    fn sort_rank(&self) -> u8 {
        match self.rank {
            JOKER => 15,
            0 => 14,
            r => r + 1,
        }
    }
    fn render(&self, notation: Notation) -> String {
        match (notation, self.is_joker()) {
            (Notation::Long, true) => "Joker".into(),
            (_, true) => "JK".into(),
            (Notation::Short, false) => format!(
                "{}{}",
                RANKS_SHORT[self.rank as usize], SUITS_SHORT[self.suit as usize]
            ),
            (Notation::Symbol, false) => format!(
                "{}{}",
                RANKS_SYMBOL[self.rank as usize], SUITS_SYMBOL[self.suit as usize]
            ),
            (Notation::Long, false) => format!(
                "{} of {}",
                RANKS_LONG[self.rank as usize], SUITS_LONG[self.suit as usize]
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Notation {
    Short,
    Symbol,
    Long,
}

impl Notation {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "short" => Ok(Notation::Short),
            "symbol" => Ok(Notation::Symbol),
            "long" => Ok(Notation::Long),
            other => Err(format!(
                "notation must be one of short, symbol, long — got \"{other}\""
            )),
        }
    }
    /// Long names need a comma to stay readable; the compact notations don't.
    fn sep(&self) -> &'static str {
        if *self == Notation::Long {
            ", "
        } else {
            " "
        }
    }
    /// Cards per line when listing a whole deck.
    fn per_line(&self) -> usize {
        if *self == Notation::Long {
            1
        } else {
            13
        }
    }
}

pub const MAX_DECKS: usize = 8;
pub const MAX_JOKERS_PER_DECK: usize = 2;
pub const MAX_PLAYERS: usize = 52;
pub const MAX_CARDS_PER_PLAYER: usize = 52;
/// 8 decks × (52 + 2 jokers) — the largest deck this tool will build.
pub const MAX_CARDS: usize = MAX_DECKS * (52 + MAX_JOKERS_PER_DECK);

/// Build the ordered (unshuffled) deck: each deck contributes S,H,D,C × A..K,
/// then its jokers.
fn build_deck(decks: usize, jokers: usize) -> Vec<Card> {
    let mut deck = Vec::with_capacity(decks * (52 + jokers));
    for _ in 0..decks {
        for suit in 0..4u8 {
            for rank in 0..13u8 {
                deck.push(Card { rank, suit });
            }
        }
        for _ in 0..jokers {
            deck.push(Card {
                rank: JOKER,
                suit: 0,
            });
        }
    }
    deck
}

/// Unbiased Fisher–Yates: walk from the top of the deck down, swapping each card
/// with a uniformly chosen card at or below it.
fn shuffle(deck: &mut [Card], rng: &mut Rng) {
    for i in (1..deck.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        deck.swap(i, j);
    }
}

/// Sort a hand for display: highest rank first, then suit in S,H,D,C order.
fn sort_hand(hand: &mut [Card]) {
    hand.sort_by(|a, b| b.sort_rank().cmp(&a.sort_rank()).then(a.suit.cmp(&b.suit)));
}

/// "1 deck (52 cards)" / "2 decks, 1 joker each (106 cards)".
fn deck_desc(decks: usize, jokers: usize, total: usize) -> String {
    let d = if decks == 1 {
        "1 deck".to_string()
    } else {
        format!("{decks} decks")
    };
    let j = if jokers == 0 {
        String::new()
    } else {
        let noun = if jokers == 1 { "joker" } else { "jokers" };
        let each = if decks == 1 { "" } else { " each" };
        format!(", {jokers} {noun}{each}")
    };
    format!("{d}{j} ({total} cards)")
}

fn render_row(cards: &[Card], notation: Notation) -> String {
    cards
        .iter()
        .map(|c| c.render(notation))
        .collect::<Vec<_>>()
        .join(notation.sep())
}

// ---------------------------------------------------------------------------
// Poker ranking (optional annotation)
// ---------------------------------------------------------------------------

/// Score exactly five cards as `(category, tiebreakers)`; higher compares better.
/// Categories: 8 straight flush … 0 high card.
fn score5(cards: &[Card]) -> (u8, Vec<u8>) {
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.sort_rank()).collect();
    ranks.sort_unstable_by(|a, b| b.cmp(a));
    let flush = cards.iter().all(|c| c.suit == cards[0].suit);

    // Group by rank, ordered by count then rank — the standard tiebreak order.
    let mut counts: Vec<(u8, u8)> = Vec::new();
    for &r in &ranks {
        match counts.iter_mut().find(|e| e.0 == r) {
            Some(e) => e.1 += 1,
            None => counts.push((r, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    let ordered: Vec<u8> = counts.iter().map(|e| e.0).collect();
    let shape: Vec<u8> = counts.iter().map(|e| e.1).collect();

    // A straight needs five distinct ranks; the wheel (A-2-3-4-5) plays low.
    let straight_high = if counts.len() == 5 {
        if ranks[0] - ranks[4] == 4 {
            Some(ranks[0])
        } else if ranks == [14, 5, 4, 3, 2] {
            Some(5)
        } else {
            None
        }
    } else {
        None
    };

    match (flush, straight_high, shape.as_slice()) {
        (true, Some(h), _) => (8, vec![h]),
        (_, _, [4, 1]) => (7, ordered),
        (_, _, [3, 2]) => (6, ordered),
        (true, None, _) => (5, ranks),
        (false, Some(h), _) => (4, vec![h]),
        (_, _, [3, 1, 1]) => (3, ordered),
        (_, _, [2, 2, 1]) => (2, ordered),
        (_, _, [2, 1, 1, 1]) => (1, ordered),
        _ => (0, ranks),
    }
}

/// Human label for a scored five-card hand.
fn label(score: &(u8, Vec<u8>)) -> String {
    let name = |v: u8| -> &'static str {
        // sort_rank 2..=14 maps back to the build-order index.
        let idx = if v == 14 { 0 } else { (v - 1) as usize };
        RANKS_LABEL[idx]
    };
    let t = &score.1;
    match score.0 {
        8 if t[0] == 14 => "Royal flush".to_string(),
        8 => format!("Straight flush, {}-high", name(t[0])),
        7 => format!("Four of a kind, {}s", name(t[0])),
        6 => format!("Full house, {}s over {}s", name(t[0]), name(t[1])),
        5 => format!("Flush, {}-high", name(t[0])),
        4 => format!("Straight, {}-high", name(t[0])),
        3 => format!("Three of a kind, {}s", name(t[0])),
        2 => format!("Two pair, {}s and {}s", name(t[0]), name(t[1])),
        1 => format!("Pair of {}s", name(t[0])),
        _ => format!("High card, {}", name(t[0])),
    }
}

/// Best five-card poker ranking of a 5–7 card hand, or `None` when the hand
/// can't be ranked (wrong size, or it contains a joker — jokers have no rank).
fn best_ranking(hand: &[Card]) -> Option<String> {
    if hand.len() < 5 || hand.len() > 7 || hand.iter().any(|c| c.is_joker()) {
        return None;
    }
    let n = hand.len();
    let mut best: Option<(u8, Vec<u8>)> = None;
    // Every 5-subset of at most 7 cards — 21 combinations at the worst.
    for a in 0..n {
        for b in (a + 1)..n {
            for c in (b + 1)..n {
                for d in (c + 1)..n {
                    for e in (d + 1)..n {
                        let five = [hand[a], hand[b], hand[c], hand[d], hand[e]];
                        let s = score5(&five);
                        if best.as_ref().is_none_or(|cur| s > *cur) {
                            best = Some(s);
                        }
                    }
                }
            }
        }
    }
    best.as_ref().map(label)
}

/// Annotation appended after a hand when `evaluate` is on.
fn annotate(hand: &[Card]) -> String {
    match best_ranking(hand) {
        Some(l) => format!(" \u{2014} {l}"),
        None if hand.iter().any(|c| c.is_joker()) => {
            " \u{2014} not ranked (contains a joker)".to_string()
        }
        None => " \u{2014} not ranked (needs 5–7 cards)".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Shuffle a deck and list it, deal it round-robin, or draw off the top.
///
/// `mode` is `shuffle` | `deal` | `draw`. `players`/`cards_per_player` apply to
/// `deal`, `count`/`replacement` to `draw`. The deck is `decks` stacked decks of
/// 52 plus `jokers` jokers each. `seed` makes the result reproducible.
#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: &str,
    players: usize,
    cards_per_player: usize,
    count: usize,
    decks: usize,
    jokers: usize,
    seed: &str,
    notation: &str,
    replacement: bool,
    sort_hands: bool,
    evaluate: bool,
) -> Result<String, String> {
    let mode_norm = mode.trim().to_ascii_lowercase();
    if !matches!(mode_norm.as_str(), "shuffle" | "deal" | "draw") {
        return Err(format!(
            "mode must be one of shuffle, deal, draw — got \"{}\"",
            mode.trim()
        ));
    }
    let notation = Notation::parse(notation)?;

    if !(1..=MAX_DECKS).contains(&decks) {
        return Err(format!(
            "decks must be between 1 and {MAX_DECKS} — got {decks}"
        ));
    }
    if jokers > MAX_JOKERS_PER_DECK {
        return Err(format!(
            "jokers must be between 0 and {MAX_JOKERS_PER_DECK} per deck — got {jokers}"
        ));
    }

    let mut deck = build_deck(decks, jokers);
    let total = deck.len();
    let mut rng = Rng::new(seed_to_u64(seed));
    shuffle(&mut deck, &mut rng);

    let seed_label = {
        let s = seed.trim();
        if s.is_empty() { "42" } else { s }
    };
    let desc = deck_desc(decks, jokers, total);
    let mut out = String::new();

    match mode_norm.as_str() {
        "shuffle" => {
            out.push_str(&format!("Shuffled deck — {desc} · seed \"{seed_label}\"\n\n"));
            let rows: Vec<String> = deck
                .chunks(notation.per_line())
                .map(|row| render_row(row, notation))
                .collect();
            out.push_str(&rows.join("\n"));
        }
        "deal" => {
            if !(1..=MAX_PLAYERS).contains(&players) {
                return Err(format!(
                    "players must be between 1 and {MAX_PLAYERS} — got {players}"
                ));
            }
            if !(1..=MAX_CARDS_PER_PLAYER).contains(&cards_per_player) {
                return Err(format!(
                    "cards_per_player must be between 1 and {MAX_CARDS_PER_PLAYER} — got {cards_per_player}"
                ));
            }
            let needed = players * cards_per_player;
            if needed > total {
                return Err(format!(
                    "need {needed} cards ({players} players × {cards_per_player} each) but the deck holds {total} — raise decks, or lower players/cards_per_player"
                ));
            }
            out.push_str(&format!(
                "Deal — {players} players × {cards_per_player} cards · {desc} · seed \"{seed_label}\"\n\n"
            ));
            // Round-robin, exactly as a dealer does it: one card to each player
            // per pass, taken off the top of the shuffled deck.
            let mut hands: Vec<Vec<Card>> = vec![Vec::with_capacity(cards_per_player); players];
            let mut top = 0usize;
            for _ in 0..cards_per_player {
                for hand in hands.iter_mut() {
                    hand.push(deck[top]);
                    top += 1;
                }
            }
            let lines: Vec<String> = hands
                .iter_mut()
                .enumerate()
                .map(|(i, hand)| {
                    if sort_hands {
                        sort_hand(hand);
                    }
                    let note = if evaluate { annotate(hand) } else { String::new() };
                    format!(
                        "Player {}: {}{}",
                        i + 1,
                        render_row(hand, notation),
                        note
                    )
                })
                .collect();
            out.push_str(&lines.join("\n"));
            out.push_str(&format!("\n\nRemaining in deck: {}", total - needed));
        }
        _ => {
            if !(1..=MAX_CARDS).contains(&count) {
                return Err(format!(
                    "count must be between 1 and {MAX_CARDS} — got {count}"
                ));
            }
            if !replacement && count > total {
                return Err(format!(
                    "cannot draw {count} cards without replacement from a {total}-card deck — raise decks, lower count, or turn on replacement"
                ));
            }
            out.push_str(&format!(
                "Draw — {count} cards · {desc} · seed \"{seed_label}\"\n\n"
            ));
            let mut drawn: Vec<Card> = if replacement {
                // Each draw is an independent pick from the full deck, so the
                // same card can come up more than once.
                (0..count)
                    .map(|_| deck[rng.below(total as u64) as usize])
                    .collect()
            } else {
                deck[..count].to_vec()
            };
            if sort_hands {
                sort_hand(&mut drawn);
            }
            let note = if evaluate {
                annotate(&drawn)
            } else {
                String::new()
            };
            out.push_str(&render_row(&drawn, notation));
            out.push_str(&note);
            if replacement {
                out.push_str(&format!(
                    "\n\nRemaining in deck: {total} (drawn with replacement)"
                ));
            } else {
                out.push_str(&format!("\n\nRemaining in deck: {}", total - count));
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deal5() -> String {
        run("deal", 4, 5, 5, 1, 0, "42", "short", false, false, false).unwrap()
    }

    #[test]
    fn deal_is_reproducible_and_conserves_cards() {
        let a = deal5();
        assert_eq!(a, deal5(), "same seed must give the same deal");
        assert!(a.starts_with("Deal — 4 players × 5 cards · 1 deck (52 cards) · seed \"42\"\n\n"));
        assert!(a.ends_with("Remaining in deck: 32"));
        // 20 dealt cards, all distinct within a single deck.
        let cards: Vec<&str> = a
            .lines()
            .filter(|l| l.starts_with("Player "))
            .flat_map(|l| l.split(": ").nth(1).unwrap().split(' '))
            .collect();
        assert_eq!(cards.len(), 20);
        let mut uniq = cards.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), 20, "no duplicate cards from one deck");
    }

    #[test]
    fn a_different_seed_gives_a_different_deal() {
        assert_ne!(
            deal5(),
            run("deal", 4, 5, 5, 1, 0, "7", "short", false, false, false).unwrap()
        );
    }

    #[test]
    fn shuffle_lists_every_card_exactly_once() {
        let out = run("shuffle", 4, 5, 5, 1, 0, "42", "short", false, false, false).unwrap();
        let body = out.split("\n\n").nth(1).unwrap();
        let mut cards: Vec<&str> = body.split_whitespace().collect();
        assert_eq!(cards.len(), 52);
        cards.sort_unstable();
        cards.dedup();
        assert_eq!(cards.len(), 52);
        // 13 per line → 4 lines.
        assert_eq!(body.lines().count(), 4);
    }

    #[test]
    fn draw_takes_off_the_top_of_the_same_shuffle() {
        let drawn = run("draw", 4, 5, 3, 1, 0, "42", "short", false, false, false).unwrap();
        let shuffled = run("shuffle", 4, 5, 5, 1, 0, "42", "short", false, false, false).unwrap();
        let top3: Vec<&str> = shuffled
            .split("\n\n")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .take(3)
            .collect();
        let body = drawn.split("\n\n").nth(1).unwrap();
        assert_eq!(body, top3.join(" "));
        assert!(drawn.ends_with("Remaining in deck: 49"));
    }

    #[test]
    fn jokers_and_multiple_decks_grow_the_deck() {
        let out = run("shuffle", 4, 5, 5, 2, 1, "42", "short", false, false, false).unwrap();
        assert!(out.starts_with("Shuffled deck — 2 decks, 1 joker each (106 cards) · seed \"42\""));
        let body = out.split("\n\n").nth(1).unwrap();
        assert_eq!(body.split_whitespace().count(), 106);
        assert_eq!(body.split_whitespace().filter(|c| *c == "JK").count(), 2);
    }

    #[test]
    fn notations_render_the_same_deck_differently() {
        let short = run("draw", 4, 5, 1, 1, 0, "42", "short", false, false, false).unwrap();
        let symbol = run("draw", 4, 5, 1, 1, 0, "42", "symbol", false, false, false).unwrap();
        let long = run("draw", 4, 5, 1, 1, 0, "42", "long", false, false, false).unwrap();
        assert_eq!(short.split("\n\n").nth(1).unwrap(), "4H");
        assert_eq!(symbol.split("\n\n").nth(1).unwrap(), "4\u{2665}");
        assert_eq!(long.split("\n\n").nth(1).unwrap(), "Four of Hearts");
    }

    #[test]
    fn draw_with_replacement_keeps_the_deck_full() {
        // 60 cards is more than a single deck holds, so it only works with
        // replacement — without it the draw is rejected.
        assert!(run("draw", 4, 5, 60, 1, 0, "42", "short", false, false, false).is_err());
        let out = run("draw", 4, 5, 60, 1, 0, "42", "short", true, false, false).unwrap();
        let body = out.split("\n\n").nth(1).unwrap();
        assert_eq!(body.split(' ').count(), 60);
        assert!(out.ends_with("Remaining in deck: 52 (drawn with replacement)"));
        // With replacement, repeats are expected — 60 draws from 52 cards.
        let mut uniq: Vec<&str> = body.split(' ').collect();
        uniq.sort_unstable();
        uniq.dedup();
        assert!(uniq.len() < 60, "replacement draw produced no repeats");
    }

    #[test]
    fn sorted_hands_run_high_to_low() {
        let out = run("deal", 1, 5, 5, 1, 0, "42", "short", false, true, false).unwrap();
        let hand: Vec<&str> = out
            .lines()
            .find(|l| l.starts_with("Player 1"))
            .unwrap()
            .split(": ")
            .nth(1)
            .unwrap()
            .split(' ')
            .collect();
        let val = |c: &str| match &c[..1] {
            "A" => 14,
            "K" => 13,
            "Q" => 12,
            "J" => 11,
            "T" => 10,
            d => d.parse::<u8>().unwrap(),
        };
        for w in hand.windows(2) {
            assert!(val(w[0]) >= val(w[1]), "{hand:?} is not high-to-low");
        }
    }

    #[test]
    fn poker_rankings_are_labelled() {
        let sf = [
            Card { rank: 9, suit: 1 },  // 10H
            Card { rank: 10, suit: 1 }, // JH
            Card { rank: 11, suit: 1 }, // QH
            Card { rank: 12, suit: 1 }, // KH
            Card { rank: 0, suit: 1 },  // AH
        ];
        assert_eq!(best_ranking(&sf).unwrap(), "Royal flush");
        let wheel = [
            Card { rank: 0, suit: 0 },
            Card { rank: 1, suit: 1 },
            Card { rank: 2, suit: 2 },
            Card { rank: 3, suit: 3 },
            Card { rank: 4, suit: 0 },
        ];
        assert_eq!(best_ranking(&wheel).unwrap(), "Straight, 5-high");
        let boat = [
            Card { rank: 6, suit: 0 },
            Card { rank: 6, suit: 1 },
            Card { rank: 6, suit: 2 },
            Card { rank: 11, suit: 0 },
            Card { rank: 11, suit: 1 },
        ];
        assert_eq!(best_ranking(&boat).unwrap(), "Full house, 7s over Queens");
        let pair = [
            Card { rank: 6, suit: 0 },
            Card { rank: 6, suit: 1 },
            Card { rank: 2, suit: 2 },
            Card { rank: 11, suit: 0 },
            Card { rank: 9, suit: 1 },
        ];
        assert_eq!(best_ranking(&pair).unwrap(), "Pair of 7s");
        // Best-of-seven picks the flush hiding in the extra cards.
        let seven = [
            Card { rank: 0, suit: 2 },
            Card { rank: 3, suit: 2 },
            Card { rank: 5, suit: 2 },
            Card { rank: 8, suit: 2 },
            Card { rank: 11, suit: 2 },
            Card { rank: 11, suit: 0 },
            Card { rank: 2, suit: 1 },
        ];
        assert_eq!(best_ranking(&seven).unwrap(), "Flush, Ace-high");
        // Jokers have no rank, and four cards is too few.
        assert!(best_ranking(&[sf[0], sf[1], sf[2], sf[3]]).is_none());
        let with_joker = [
            sf[0],
            sf[1],
            sf[2],
            sf[3],
            Card {
                rank: JOKER,
                suit: 0,
            },
        ];
        assert_eq!(
            annotate(&with_joker),
            " — not ranked (contains a joker)"
        );
    }

    #[test]
    fn evaluate_annotates_every_dealt_hand() {
        let out = run("deal", 2, 5, 5, 1, 0, "42", "short", false, true, true).unwrap();
        for line in out.lines().filter(|l| l.starts_with("Player ")) {
            assert!(line.contains(" — "), "missing ranking on {line:?}");
        }
    }

    #[test]
    fn a_text_seed_is_hashed_and_stable() {
        let a = run("deal", 2, 5, 5, 1, 0, "table-3", "short", false, false, false).unwrap();
        assert_eq!(
            a,
            run("deal", 2, 5, 5, 1, 0, " table-3 ", "short", false, false, false).unwrap()
        );
        assert!(a.contains("seed \"table-3\""));
        assert_ne!(
            a,
            run("deal", 2, 5, 5, 1, 0, "table-4", "short", false, false, false).unwrap()
        );
    }

    #[test]
    fn dealing_more_cards_than_the_deck_holds_is_an_error() {
        let err = run("deal", 4, 15, 5, 1, 0, "42", "short", false, false, false).unwrap_err();
        assert_eq!(
            err,
            "need 60 cards (4 players × 15 each) but the deck holds 52 — raise decks, or lower players/cards_per_player"
        );
    }

    #[test]
    fn the_exact_deck_size_still_deals() {
        let out = run("deal", 4, 13, 5, 1, 0, "42", "short", false, false, false).unwrap();
        assert!(out.ends_with("Remaining in deck: 0"));
    }

    #[test]
    fn bad_enum_and_range_inputs_are_rejected() {
        assert_eq!(
            run("cut", 4, 5, 5, 1, 0, "42", "short", false, false, false).unwrap_err(),
            "mode must be one of shuffle, deal, draw — got \"cut\""
        );
        assert_eq!(
            run("deal", 4, 5, 5, 1, 0, "42", "emoji", false, false, false).unwrap_err(),
            "notation must be one of short, symbol, long — got \"emoji\""
        );
        assert_eq!(
            run("deal", 4, 5, 5, 9, 0, "42", "short", false, false, false).unwrap_err(),
            "decks must be between 1 and 8 — got 9"
        );
        assert_eq!(
            run("deal", 4, 5, 5, 1, 3, "42", "short", false, false, false).unwrap_err(),
            "jokers must be between 0 and 2 per deck — got 3"
        );
        assert_eq!(
            run("draw", 4, 5, 53, 1, 0, "42", "short", false, false, false).unwrap_err(),
            "cannot draw 53 cards without replacement from a 52-card deck — raise decks, lower count, or turn on replacement"
        );
    }

    #[test]
    fn the_shuffle_is_unbiased_across_seeds() {
        // Every card should reach the top slot over enough seeds; a biased
        // Fisher–Yates (the classic off-by-one) pins card 0 far too often.
        let mut tops = std::collections::HashSet::new();
        for seed in 0..400u32 {
            let out = run(
                "draw",
                1,
                1,
                1,
                1,
                0,
                &seed.to_string(),
                "short",
                false,
                false,
                false,
            )
            .unwrap();
            tops.insert(out.split("\n\n").nth(1).unwrap().to_string());
        }
        assert!(tops.len() > 40, "only {} distinct top cards", tops.len());
    }
}
