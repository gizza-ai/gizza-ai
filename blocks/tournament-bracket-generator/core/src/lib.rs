//! tournament-bracket-generator core — pure compute, shared by the chat skill block and the web page.
//!
//! Builds a knockout bracket from a participant list.
//!
//! **Sizing.** A knockout bracket only works on a power of two, so the field is rounded up to the
//! next one (6 players → an 8-slot bracket) and the spare slots become byes. Under the default
//! `standard` seeding the bracket is laid out with the classic recursive 1-vs-N pairing
//! (1v8, 4v5, 2v7, 3v6 for eight slots), which puts the top and bottom halves on opposite sides —
//! seeds 1 and 2 can only meet in the final — and hands the byes to the highest seeds, because the
//! lowest seed numbers are the missing ones.
//!
//! **Double elimination.** The winners bracket is that same single-elimination tree. Losers drop
//! into a second tree that alternates *minor* rounds (survivors play each other) and *major* rounds
//! (survivors meet the players just knocked out of the winners bracket, entered in reverse order so
//! a rematch is pushed as late as possible). The two bracket champions meet in the grand final, with
//! an optional reset match for the case where the losers-bracket finalist wins it and both sides end
//! on one loss.
//!
//! Nothing here is random unless `seeding = random` is asked for, and even then the draw comes from
//! a seeded splitmix64 PRNG — the same input always produces a byte-identical bracket.

use std::collections::HashSet;

/// A printable bracket has to stay a printable bracket; 64 is a 64-slot tree.
pub const MAX_PARTICIPANTS: usize = 64;
/// Two is the smallest thing you can knock out.
pub const MIN_PARTICIPANTS: usize = 2;
/// Rendered for an empty bracket slot.
pub const BYE: &str = "BYE";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BracketType {
    /// One loss and you are out.
    Single,
    /// A second chance in a losers bracket before elimination.
    Double,
}

impl BracketType {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "single" | "single-elimination" | "single_elimination" | "knockout" | "1" => {
                Ok(BracketType::Single)
            }
            "double" | "double-elimination" | "double_elimination" | "2" => Ok(BracketType::Double),
            other => Err(format!(
                "unknown bracket_type '{other}': expected 'single' or 'double'"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Seeding {
    /// Entered order is the seed order, placed 1-vs-N across the bracket.
    Standard,
    /// Entered order fills the bracket slots top to bottom (entry 1 plays entry 2).
    Ordered,
    /// Deterministic shuffle of the entered order, then standard placement.
    Random,
}

impl Seeding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard" | "seeded" | "seed" => Ok(Seeding::Standard),
            "ordered" | "as-entered" | "as_entered" | "manual" => Ok(Seeding::Ordered),
            "random" | "draw" | "shuffle" => Ok(Seeding::Random),
            other => Err(format!(
                "unknown seeding '{other}': expected 'standard', 'ordered' or 'random'"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Text,
    Markdown,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "txt" | "plain" => Ok(OutputFormat::Text),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output_format '{other}': expected 'text', 'markdown', 'csv' or 'json'"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub bracket_type: BracketType,
    pub seeding: Seeding,
    pub format: OutputFormat,
    pub third_place_match: bool,
    pub grand_final_reset: bool,
    pub tournament_name: String,
    pub include_summary: bool,
    pub seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bracket_type: BracketType::Single,
            seeding: Seeding::Standard,
            format: OutputFormat::Text,
            third_place_match: false,
            grand_final_reset: true,
            tournament_name: String::new(),
            include_summary: true,
            seed: 0,
        }
    }
}

/// Which tree a match belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    Main,
    Winners,
    Losers,
    GrandFinal,
    Consolation,
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Section::Main => "main",
            Section::Winners => "winners",
            Section::Losers => "losers",
            Section::GrandFinal => "grand_final",
            Section::Consolation => "consolation",
        }
    }
}

/// An unresolved reference to whoever ends up in a bracket slot.
#[derive(Clone, Debug)]
enum Ref {
    /// Index into the seeded participant list.
    Seed(usize),
    Bye,
    Winner(usize),
    Loser(usize),
}

/// A slot once it has been resolved as far as it can be without playing the tournament.
#[derive(Clone, Debug, PartialEq)]
enum Side {
    Player { name: String, seed: usize },
    Bye,
    Pending(String),
}

impl Side {
    fn label(&self) -> String {
        match self {
            Side::Player { name, seed } => format!("{name} ({seed})"),
            Side::Bye => BYE.to_string(),
            Side::Pending(s) => s.clone(),
        }
    }
    fn plain(&self) -> String {
        match self {
            Side::Player { name, .. } => name.clone(),
            Side::Bye => BYE.to_string(),
            Side::Pending(s) => s.clone(),
        }
    }
    fn seed(&self) -> Option<usize> {
        match self {
            Side::Player { seed, .. } => Some(*seed),
            _ => None,
        }
    }
    fn is_bye(&self) -> bool {
        matches!(self, Side::Bye)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Two real sides — someone has to play it.
    Played,
    /// One side is an empty slot; the other walks through.
    Bye,
    /// Both sides are empty slots; nothing to print.
    Empty,
    /// The grand-final reset — played only if the losers-bracket finalist wins the grand final.
    Conditional,
}

struct Node {
    section: Section,
    round: usize,
    round_name: String,
    next_round_name: Option<String>,
    a: Ref,
    b: Ref,
    a_side: Side,
    b_side: Side,
    winner: Side,
    loser: Side,
    kind: Kind,
    number: Option<usize>,
}

/// The full bracket, before rendering.
struct Bracket {
    players: Vec<String>,
    size: usize,
    byes: usize,
    nodes: Vec<Node>,
    winners_rounds: usize,
    losers_rounds: usize,
}

/// Bracket slot order for a power-of-two field: 1v(N), then recursively split so the
/// top two seeds can only meet in the final. `seed_order(8) == [1,8,4,5,2,7,3,6]`.
fn seed_order(size: usize) -> Vec<usize> {
    let mut order = vec![1usize];
    let mut s = 1usize;
    while s < size {
        s *= 2;
        let mut next = Vec::with_capacity(s);
        for &x in &order {
            next.push(x);
            next.push(s + 1 - x);
        }
        order = next;
    }
    order
}

/// splitmix64 — a tiny, fully deterministic PRNG so a "random" draw is still reproducible.
fn shuffle(items: &mut [String], seed: u64) {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    if items.len() < 2 {
        return;
    }
    for i in (1..items.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

/// Strip a leading list marker (`-`, `*`, `•`, `1.`, `1)`) from a pasted line.
fn strip_marker(s: &str) -> &str {
    let t = s.trim();
    for m in ['-', '*', '•'] {
        if let Some(rest) = t.strip_prefix(m) {
            return rest.trim();
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let rest = &t[digits..];
        if let Some(r) = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')')) {
            return r.trim();
        }
    }
    t
}

/// Turn the pasted roster into a clean, unique participant list.
///
/// Accepts one name per line, a single comma-separated line, or a bare count (`16` → `Team 1`…).
/// `#` comments and blank lines are dropped and `-`/`*`/`1.` list markers are stripped.
pub fn parse_participants(input: &str) -> Result<Vec<String>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "no participants: enter one team or player per line, or a count such as '16'".into(),
        );
    }
    if let Ok(count) = trimmed.parse::<usize>() {
        if !(MIN_PARTICIPANTS..=MAX_PARTICIPANTS).contains(&count) {
            return Err(format!(
                "a bare participant count must be between {MIN_PARTICIPANTS} and {MAX_PARTICIPANTS}, got {count}"
            ));
        }
        return Ok((1..=count).map(|i| format!("Team {i}")).collect());
    }

    let lines: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let raw: Vec<String> = if lines.len() == 1 && lines[0].contains(',') {
        lines[0].split(',').map(|s| s.trim().to_string()).collect()
    } else {
        lines.iter().map(|l| strip_marker(l).to_string()).collect()
    };

    let mut names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for name in raw {
        let name = name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        if !seen.insert(name.to_lowercase()) {
            return Err(format!(
                "duplicate participant '{name}': every team or player must have a distinct name (comparison ignores case)"
            ));
        }
        names.push(name);
    }

    if names.len() < MIN_PARTICIPANTS {
        return Err(format!(
            "need at least {MIN_PARTICIPANTS} participants to build a bracket, got {}",
            names.len()
        ));
    }
    if names.len() > MAX_PARTICIPANTS {
        return Err(format!(
            "too many participants: {} entered, the limit is {MAX_PARTICIPANTS}",
            names.len()
        ));
    }
    Ok(names)
}

fn single_round_name(round: usize, size: usize) -> String {
    let entrants = size >> (round - 1);
    match entrants {
        2 => "Final".to_string(),
        4 => "Semifinals".to_string(),
        8 => "Quarterfinals".to_string(),
        _ => format!("Round of {entrants}"),
    }
}

fn winners_round_name(round: usize, rounds: usize) -> String {
    if round == rounds {
        "Winners Final".to_string()
    } else if round + 1 == rounds {
        "Winners Semifinals".to_string()
    } else {
        format!("Winners Round {round}")
    }
}

fn losers_round_name(round: usize, rounds: usize) -> String {
    if round == rounds {
        "Losers Final".to_string()
    } else if round + 1 == rounds {
        "Losers Semifinal".to_string()
    } else {
        format!("Losers Round {round}")
    }
}

fn build(players: Vec<String>, opts: &Options) -> Bracket {
    let n = players.len();
    let size = n.next_power_of_two().max(2);
    let byes = size - n;
    let rounds = size.trailing_zeros() as usize;
    let double = opts.bracket_type == BracketType::Double;

    // Fill the bracket slots.
    let slots: Vec<Ref> = match opts.seeding {
        Seeding::Ordered => (0..size)
            .map(|i| if i < n { Ref::Seed(i) } else { Ref::Bye })
            .collect(),
        Seeding::Standard | Seeding::Random => seed_order(size)
            .into_iter()
            .map(|s| if s <= n { Ref::Seed(s - 1) } else { Ref::Bye })
            .collect(),
    };

    let mut nodes: Vec<Node> = Vec::new();
    let name_of = |round: usize| -> String {
        if double {
            winners_round_name(round, rounds)
        } else {
            single_round_name(round, size)
        }
    };
    let section = if double { Section::Winners } else { Section::Main };

    // Winners / main tree.
    let mut wb_rounds: Vec<Vec<usize>> = Vec::new();
    let mut layer: Vec<Ref> = slots;
    for round in 1..=rounds {
        let mut ids = Vec::new();
        let mut winners = Vec::new();
        for pair in layer.chunks(2) {
            let idx = nodes.len();
            nodes.push(Node {
                section,
                round,
                round_name: name_of(round),
                next_round_name: if round < rounds {
                    Some(name_of(round + 1))
                } else {
                    None
                },
                a: pair[0].clone(),
                b: pair[1].clone(),
                a_side: Side::Bye,
                b_side: Side::Bye,
                winner: Side::Bye,
                loser: Side::Bye,
                kind: Kind::Empty,
                number: None,
            });
            ids.push(idx);
            winners.push(Ref::Winner(idx));
        }
        wb_rounds.push(ids);
        layer = winners;
    }
    let wb_final = *wb_rounds[rounds - 1].first().expect("one final match");

    let mut losers_rounds = 0usize;
    if double {
        losers_rounds = 2 * rounds.saturating_sub(1);
        let mut feed: Vec<Ref> = wb_rounds[0].iter().map(|&i| Ref::Loser(i)).collect();
        let mut lb_round = 0usize;
        for j in 1..rounds {
            // Minor round: the survivors of the losers bracket play each other.
            lb_round += 1;
            let mut survivors = Vec::new();
            for pair in feed.chunks(2) {
                let idx = nodes.len();
                nodes.push(Node {
                    section: Section::Losers,
                    round: lb_round,
                    round_name: losers_round_name(lb_round, losers_rounds),
                    next_round_name: Some(losers_round_name(lb_round + 1, losers_rounds)),
                    a: pair[0].clone(),
                    b: pair[1].clone(),
                    a_side: Side::Bye,
                    b_side: Side::Bye,
                    winner: Side::Bye,
                    loser: Side::Bye,
                    kind: Kind::Empty,
                    number: None,
                });
                survivors.push(Ref::Winner(idx));
            }
            // Major round: they meet the players just dropped by the winners bracket. The drops
            // enter in reverse order so an immediate rematch is pushed as late as possible.
            lb_round += 1;
            let mut drops: Vec<Ref> = wb_rounds[j].iter().map(|&i| Ref::Loser(i)).collect();
            drops.reverse();
            let mut next_feed = Vec::new();
            for (drop, survivor) in drops.into_iter().zip(survivors.into_iter()) {
                let idx = nodes.len();
                nodes.push(Node {
                    section: Section::Losers,
                    round: lb_round,
                    round_name: losers_round_name(lb_round, losers_rounds),
                    next_round_name: if lb_round < losers_rounds {
                        Some(losers_round_name(lb_round + 1, losers_rounds))
                    } else {
                        Some("Grand Final".to_string())
                    },
                    a: drop,
                    b: survivor,
                    a_side: Side::Bye,
                    b_side: Side::Bye,
                    winner: Side::Bye,
                    loser: Side::Bye,
                    kind: Kind::Empty,
                    number: None,
                });
                next_feed.push(Ref::Winner(idx));
            }
            feed = next_feed;
        }
        // With a two-slot bracket there is no losers tree at all: the only loser is the finalist.
        let lb_champion = feed.into_iter().next().unwrap_or(Ref::Loser(wb_final));
        let gf = nodes.len();
        nodes.push(Node {
            section: Section::GrandFinal,
            round: 1,
            round_name: "Grand Final".to_string(),
            next_round_name: None,
            a: Ref::Winner(wb_final),
            b: lb_champion,
            a_side: Side::Bye,
            b_side: Side::Bye,
            winner: Side::Bye,
            loser: Side::Bye,
            kind: Kind::Empty,
            number: None,
        });
        if opts.grand_final_reset {
            nodes.push(Node {
                section: Section::GrandFinal,
                round: 2,
                round_name: "Grand Final Reset".to_string(),
                next_round_name: None,
                a: Ref::Winner(gf),
                b: Ref::Loser(gf),
                a_side: Side::Bye,
                b_side: Side::Bye,
                winner: Side::Bye,
                loser: Side::Bye,
                kind: Kind::Conditional,
                number: None,
            });
        }
    } else if opts.third_place_match && rounds >= 2 {
        let semis = &wb_rounds[rounds - 2];
        if semis.len() == 2 {
            nodes.push(Node {
                section: Section::Consolation,
                round: 1,
                round_name: "Third-Place Match".to_string(),
                next_round_name: None,
                a: Ref::Loser(semis[0]),
                b: Ref::Loser(semis[1]),
                a_side: Side::Bye,
                b_side: Side::Bye,
                winner: Side::Bye,
                loser: Side::Bye,
                kind: Kind::Empty,
                number: None,
            });
        }
    }

    // Resolve every slot in build order — each node only refers to earlier ones.
    let mut number = 0usize;
    for i in 0..nodes.len() {
        let a_side = resolve(&nodes[i].a, &nodes, &players);
        let b_side = resolve(&nodes[i].b, &nodes, &players);
        let conditional = nodes[i].kind == Kind::Conditional;
        let (kind, winner, loser) = if conditional {
            number += 1;
            (
                Kind::Conditional,
                Side::Pending(format!("Winner of M{number}")),
                Side::Pending(format!("Loser of M{number}")),
            )
        } else if a_side.is_bye() && b_side.is_bye() {
            (Kind::Empty, Side::Bye, Side::Bye)
        } else if a_side.is_bye() {
            (Kind::Bye, b_side.clone(), Side::Bye)
        } else if b_side.is_bye() {
            (Kind::Bye, a_side.clone(), Side::Bye)
        } else {
            number += 1;
            (
                Kind::Played,
                Side::Pending(format!("Winner of M{number}")),
                Side::Pending(format!("Loser of M{number}")),
            )
        };
        let node = &mut nodes[i];
        node.a_side = a_side;
        node.b_side = b_side;
        node.number = if matches!(kind, Kind::Played | Kind::Conditional) {
            Some(number)
        } else {
            None
        };
        node.kind = kind;
        node.winner = winner;
        node.loser = loser;
    }

    Bracket {
        players,
        size,
        byes,
        nodes,
        winners_rounds: rounds,
        losers_rounds,
    }
}

fn resolve(r: &Ref, nodes: &[Node], players: &[String]) -> Side {
    match r {
        Ref::Bye => Side::Bye,
        Ref::Seed(i) => Side::Player {
            name: players[*i].clone(),
            seed: i + 1,
        },
        Ref::Winner(i) => nodes[*i].winner.clone(),
        Ref::Loser(i) => nodes[*i].loser.clone(),
    }
}

impl Bracket {
    fn played(&self) -> usize {
        self.nodes
            .iter()
            .filter(|n| n.kind == Kind::Played)
            .count()
    }
    fn has_reset(&self) -> bool {
        self.nodes.iter().any(|n| n.kind == Kind::Conditional)
    }
    fn visible(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| n.kind != Kind::Empty)
    }
}

fn summary_lines(b: &Bracket, opts: &Options) -> Vec<String> {
    let mut out = Vec::new();
    if !opts.tournament_name.trim().is_empty() {
        out.push(opts.tournament_name.trim().to_string());
    }
    let kind = match opts.bracket_type {
        BracketType::Single => "Single elimination",
        BracketType::Double => "Double elimination",
    };
    let mut parts = vec![
        kind.to_string(),
        format!("{} participants", b.players.len()),
        format!("{}-slot bracket", b.size),
        format!("{} byes", b.byes),
    ];
    match opts.bracket_type {
        BracketType::Single => parts.push(format!("{} rounds", b.winners_rounds)),
        BracketType::Double => {
            parts.push(format!("{} winners rounds", b.winners_rounds));
            parts.push(format!("{} losers rounds", b.losers_rounds));
        }
    }
    let played = b.played();
    parts.push(if b.has_reset() {
        format!("{played} matches (+1 if the bracket resets)")
    } else {
        format!("{played} matches")
    });
    out.push(parts.join(" · "));
    let seeds: Vec<String> = b
        .players
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{} {}", i + 1, p))
        .collect();
    out.push(format!("Seeds: {}", seeds.join(" · ")));
    out
}

fn render_text(b: &Bracket, opts: &Options) -> String {
    let mut out: Vec<String> = Vec::new();
    if opts.include_summary {
        out.extend(summary_lines(b, opts));
        out.push(String::new());
    }
    let mut last_section: Option<Section> = None;
    let mut last_round: Option<(Section, usize)> = None;
    for node in b.visible() {
        if last_section != Some(node.section) {
            if let Some(header) = match node.section {
                Section::Winners => Some("WINNERS BRACKET"),
                Section::Losers => Some("LOSERS BRACKET"),
                Section::GrandFinal => Some("GRAND FINAL"),
                Section::Consolation => Some("THIRD-PLACE MATCH"),
                Section::Main => None,
            } {
                if !out.is_empty() && !out.last().map(String::is_empty).unwrap_or(true) {
                    out.push(String::new());
                }
                out.push(header.to_string());
                out.push(String::new());
            }
            last_section = Some(node.section);
            last_round = None;
        }
        let show_round_header = !matches!(node.section, Section::GrandFinal | Section::Consolation);
        if show_round_header && last_round != Some((node.section, node.round)) {
            if last_round.is_some() {
                out.push(String::new());
            }
            out.push(if node.section == Section::Main {
                format!("Round {} — {}", node.round, node.round_name)
            } else {
                node.round_name.clone()
            });
            last_round = Some((node.section, node.round));
        }
        match node.kind {
            Kind::Bye => {
                let walker = if node.a_side.is_bye() {
                    &node.b_side
                } else {
                    &node.a_side
                };
                let dest = node
                    .next_round_name
                    .as_deref()
                    .map(|r| format!(" to {r}"))
                    .unwrap_or_default();
                out.push(format!("      {} — bye{}", walker.label(), dest));
            }
            Kind::Conditional => out.push(format!(
                "  M{}  {} vs {} — reset, played only if the losers-bracket finalist wins M{}",
                node.number.unwrap_or(0),
                node.a_side.label(),
                node.b_side.label(),
                node.number.unwrap_or(0) - 1
            )),
            _ => out.push(format!(
                "  M{}  {} vs {}",
                node.number.unwrap_or(0),
                node.a_side.label(),
                node.b_side.label()
            )),
        }
    }
    out.join("\n")
}

fn render_markdown(b: &Bracket, opts: &Options) -> String {
    let mut out: Vec<String> = Vec::new();
    if opts.include_summary {
        let mut lines = summary_lines(b, opts);
        if !opts.tournament_name.trim().is_empty() {
            let name = lines.remove(0);
            out.push(format!("# {name}"));
            out.push(String::new());
        }
        for line in lines {
            out.push(line);
            out.push(String::new());
        }
    }
    out.push("| Match | Bracket | Round | Side A | Side B |".to_string());
    out.push("| --- | --- | --- | --- | --- |".to_string());
    for node in b.visible() {
        let number = node
            .number
            .map(|n| format!("M{n}"))
            .unwrap_or_else(|| "—".to_string());
        out.push(format!(
            "| {} | {} | {} | {} | {} |",
            number,
            section_label(node.section),
            node.round_name,
            node.a_side.label(),
            node.b_side.label()
        ));
    }
    out.join("\n")
}

fn section_label(s: Section) -> &'static str {
    match s {
        Section::Main => "Main",
        Section::Winners => "Winners",
        Section::Losers => "Losers",
        Section::GrandFinal => "Grand final",
        Section::Consolation => "Consolation",
    }
}

fn status_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Bye => "bye",
        Kind::Conditional => "reset",
        _ => "match",
    }
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(b: &Bracket) -> String {
    let mut out =
        vec!["match,bracket,round,round_name,side_a,side_a_seed,side_b,side_b_seed,status"
            .to_string()];
    for node in b.visible() {
        let number = node
            .number
            .map(|n| format!("M{n}"))
            .unwrap_or_else(String::new);
        out.push(format!(
            "{},{},{},{},{},{},{},{},{}",
            csv_field(&number),
            node.section.key(),
            node.round,
            csv_field(&node.round_name),
            csv_field(&node.a_side.plain()),
            node.a_side
                .seed()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            csv_field(&node.b_side.plain()),
            node.b_side
                .seed()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            status_of(node.kind)
        ));
    }
    out.join("\n")
}

fn json_string(s: &str) -> String {
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

fn render_json(b: &Bracket) -> String {
    let rows: Vec<String> = b
        .visible()
        .map(|node| {
            let number = node
                .number
                .map(|n| json_string(&format!("M{n}")))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"match\":{},\"bracket\":{},\"round\":{},\"round_name\":{},\"side_a\":{},\"side_a_seed\":{},\"side_b\":{},\"side_b_seed\":{},\"status\":{}}}",
                number,
                json_string(node.section.key()),
                node.round,
                json_string(&node.round_name),
                json_string(&node.a_side.plain()),
                node.a_side.seed().map(|s| s.to_string()).unwrap_or_else(|| "null".to_string()),
                json_string(&node.b_side.plain()),
                node.b_side.seed().map(|s| s.to_string()).unwrap_or_else(|| "null".to_string()),
                json_string(status_of(node.kind)),
            )
        })
        .collect();
    format!("[\n  {}\n]", rows.join(",\n  "))
}

/// Build a bracket from a pasted roster and render it in the requested format.
pub fn generate(participants: &str, opts: &Options) -> Result<String, String> {
    let mut players = parse_participants(participants)?;
    if opts.seeding == Seeding::Random {
        shuffle(&mut players, opts.seed);
    }
    let bracket = build(players, opts);
    Ok(match opts.format {
        OutputFormat::Text => render_text(&bracket, opts),
        OutputFormat::Markdown => render_markdown(&bracket, opts),
        OutputFormat::Csv => render_csv(&bracket),
        OutputFormat::Json => render_json(&bracket),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn seed_order_is_the_standard_bracket_layout() {
        assert_eq!(seed_order(2), vec![1, 2]);
        assert_eq!(seed_order(4), vec![1, 4, 2, 3]);
        assert_eq!(seed_order(8), vec![1, 8, 4, 5, 2, 7, 3, 6]);
        assert_eq!(
            seed_order(16),
            vec![1, 16, 8, 9, 4, 13, 5, 12, 2, 15, 7, 10, 3, 14, 6, 11]
        );
    }

    #[test]
    fn four_player_single_elimination_pairs_one_v_four() {
        let out = generate("Ann\nBob\nCid\nDee", &opts()).unwrap();
        assert!(out.contains("M1  Ann (1) vs Dee (4)"), "{out}");
        assert!(out.contains("M2  Bob (2) vs Cid (3)"), "{out}");
        assert!(out.contains("M3  Winner of M1 vs Winner of M2"), "{out}");
        assert!(out.contains("Round 2 — Final"), "{out}");
        assert!(out.contains("4-slot bracket · 0 byes · 2 rounds · 3 matches"), "{out}");
    }

    #[test]
    fn byes_go_to_the_top_seeds() {
        let out = generate("A\nB\nC\nD\nE\nF", &opts()).unwrap();
        // 6 players in an 8-slot bracket → seeds 1 and 2 get the byes.
        assert!(out.contains("A (1) — bye to Semifinals"), "{out}");
        assert!(out.contains("B (2) — bye to Semifinals"), "{out}");
        assert!(out.contains("M1  D (4) vs E (5)"), "{out}");
        assert!(out.contains("M2  C (3) vs F (6)"), "{out}");
        assert!(out.contains("M3  A (1) vs Winner of M1"), "{out}");
        assert!(out.contains("8-slot bracket · 2 byes"), "{out}");
    }

    #[test]
    fn a_bare_count_expands_to_numbered_teams() {
        let players = parse_participants("8").unwrap();
        assert_eq!(players.len(), 8);
        assert_eq!(players[0], "Team 1");
        assert_eq!(players[7], "Team 8");
    }

    #[test]
    fn list_markers_comments_and_comma_lines_are_accepted() {
        assert_eq!(
            parse_participants("- Ann\n* Bob\n3. Cid\n# note\n\nDee").unwrap(),
            vec!["Ann", "Bob", "Cid", "Dee"]
        );
        assert_eq!(
            parse_participants("Ann, Bob , Cid").unwrap(),
            vec!["Ann", "Bob", "Cid"]
        );
    }

    #[test]
    fn ordered_seeding_pairs_adjacent_entries() {
        let o = Options {
            seeding: Seeding::Ordered,
            ..opts()
        };
        let out = generate("Ann\nBob\nCid\nDee", &o).unwrap();
        assert!(out.contains("M1  Ann (1) vs Bob (2)"), "{out}");
        assert!(out.contains("M2  Cid (3) vs Dee (4)"), "{out}");
    }

    #[test]
    fn random_seeding_is_reproducible_and_seed_dependent() {
        let a = Options {
            seeding: Seeding::Random,
            seed: 42,
            ..opts()
        };
        let b = Options { seed: 43, ..a.clone() };
        let roster = "Ann\nBob\nCid\nDee\nEve\nFay\nGus\nHal";
        assert_eq!(generate(roster, &a).unwrap(), generate(roster, &a).unwrap());
        assert_ne!(generate(roster, &a).unwrap(), generate(roster, &b).unwrap());
    }

    #[test]
    fn third_place_match_pairs_the_semifinal_losers() {
        let o = Options {
            third_place_match: true,
            ..opts()
        };
        let out = generate("Ann\nBob\nCid\nDee", &o).unwrap();
        assert!(out.contains("THIRD-PLACE MATCH"), "{out}");
        assert!(out.contains("M4  Loser of M1 vs Loser of M2"), "{out}");
    }

    #[test]
    fn double_elimination_has_the_full_losers_tree_and_a_reset() {
        let o = Options {
            bracket_type: BracketType::Double,
            ..opts()
        };
        let out = generate("A\nB\nC\nD\nE\nF\nG\nH", &o).unwrap();
        // 8 players: 7 winners matches + 6 losers matches + grand final = 14 = 2n - 2.
        assert!(out.contains("14 matches (+1 if the bracket resets)"), "{out}");
        assert!(out.contains("WINNERS BRACKET"), "{out}");
        assert!(out.contains("LOSERS BRACKET"), "{out}");
        assert!(out.contains("Losers Round 1"), "{out}");
        assert!(out.contains("M8  Loser of M1 vs Loser of M2"), "{out}");
        assert!(out.contains("Losers Final"), "{out}");
        assert!(out.contains("GRAND FINAL"), "{out}");
        assert!(out.contains("M14  Winner of M7 vs Winner of M13"), "{out}");
        assert!(
            out.contains("M15  Winner of M14 vs Loser of M14 — reset, played only if the losers-bracket finalist wins M14"),
            "{out}"
        );
    }

    #[test]
    fn double_elimination_reset_can_be_turned_off() {
        let o = Options {
            bracket_type: BracketType::Double,
            grand_final_reset: false,
            ..opts()
        };
        let out = generate("A\nB\nC\nD", &o).unwrap();
        assert!(!out.contains("reset"), "{out}");
        assert!(out.contains("6 matches"), "{out}");
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_match() {
        let o = Options {
            format: OutputFormat::Csv,
            ..opts()
        };
        let out = generate("A\nB\nC\nD\nE\nF", &o).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "match,bracket,round,round_name,side_a,side_a_seed,side_b,side_b_seed,status"
        );
        assert!(lines.iter().any(|l| l.starts_with("M1,main,1,Quarterfinals,D,4,E,5,match")), "{out}");
        assert!(lines.iter().any(|l| l.starts_with(",main,1,Quarterfinals,A,1,BYE,,bye")), "{out}");
    }

    #[test]
    fn json_quotes_names_and_nulls_unknown_seeds() {
        let o = Options {
            format: OutputFormat::Json,
            ..opts()
        };
        let out = generate("A,B,C,D", &o).unwrap();
        assert!(out.contains("\"match\":\"M1\""), "{out}");
        assert!(out.contains("\"side_a\":\"A\",\"side_a_seed\":1"), "{out}");
        assert!(
            out.contains("\"side_a\":\"Winner of M1\",\"side_a_seed\":null"),
            "{out}"
        );
    }

    #[test]
    fn markdown_is_a_pipe_table() {
        let o = Options {
            format: OutputFormat::Markdown,
            tournament_name: "Spring Cup".into(),
            ..opts()
        };
        let out = generate("A\nB\nC\nD", &o).unwrap();
        assert!(out.starts_with("# Spring Cup"), "{out}");
        assert!(out.contains("| Match | Bracket | Round | Side A | Side B |"), "{out}");
        assert!(out.contains("| M3 | Main | Final | Winner of M1 | Winner of M2 |"), "{out}");
    }

    #[test]
    fn summary_can_be_suppressed() {
        let o = Options {
            include_summary: false,
            ..opts()
        };
        let out = generate("A\nB\nC\nD", &o).unwrap();
        assert!(!out.contains("Seeds:"), "{out}");
        assert!(out.starts_with("Round 1 — Semifinals"), "{out}");
    }

    #[test]
    fn two_player_double_elimination_is_a_best_of_three_shape() {
        let o = Options {
            bracket_type: BracketType::Double,
            ..opts()
        };
        let out = generate("A\nB", &o).unwrap();
        assert!(out.contains("M1  A (1) vs B (2)"), "{out}");
        assert!(out.contains("M2  Winner of M1 vs Loser of M1"), "{out}");
        assert!(out.contains("M3  Winner of M2 vs Loser of M2"), "{out}");
    }

    #[test]
    fn one_participant_is_rejected() {
        let err = generate("Solo", &opts()).unwrap_err();
        assert_eq!(
            err,
            "need at least 2 participants to build a bracket, got 1"
        );
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let err = generate("Ann\nBob\nann", &opts()).unwrap_err();
        assert!(err.starts_with("duplicate participant 'ann'"), "{err}");
    }

    #[test]
    fn over_the_cap_is_rejected() {
        let roster: Vec<String> = (1..=65).map(|i| format!("P{i}")).collect();
        let err = generate(&roster.join("\n"), &opts()).unwrap_err();
        assert_eq!(err, "too many participants: 65 entered, the limit is 64");
    }

    #[test]
    fn unknown_enum_values_say_what_was_expected() {
        assert_eq!(
            BracketType::parse("triple").unwrap_err(),
            "unknown bracket_type 'triple': expected 'single' or 'double'"
        );
        assert!(Seeding::parse("swiss").unwrap_err().contains("expected 'standard', 'ordered' or 'random'"));
        assert!(OutputFormat::parse("yaml").unwrap_err().contains("expected 'text', 'markdown', 'csv' or 'json'"));
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = generate("   ", &opts()).unwrap_err();
        assert!(err.starts_with("no participants"), "{err}");
    }

    #[test]
    fn match_counts_match_the_theory_for_every_size() {
        for n in 2..=32 {
            let roster: Vec<String> = (1..=n).map(|i| format!("P{i}")).collect();
            let text = generate(
                &roster.join("\n"),
                &Options {
                    format: OutputFormat::Csv,
                    ..opts()
                },
            )
            .unwrap();
            let played = text.lines().filter(|l| l.ends_with(",match")).count();
            assert_eq!(played, n - 1, "single elimination with {n} players");

            let double = generate(
                &roster.join("\n"),
                &Options {
                    bracket_type: BracketType::Double,
                    format: OutputFormat::Csv,
                    grand_final_reset: false,
                    ..opts()
                },
            )
            .unwrap();
            let played = double.lines().filter(|l| l.ends_with(",match")).count();
            assert_eq!(played, 2 * n - 2, "double elimination with {n} players");
        }
    }
}
