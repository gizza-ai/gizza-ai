//! round-robin-scheduler core — pure compute, shared by the chat skill block and the web page.
//!
//! Builds a balanced round-robin fixture list from a list of participants using the classic
//! **circle method**: one participant is held fixed while the rest rotate one position per round,
//! so after `n - 1` rounds (n even) every participant has met every other exactly once. An odd
//! count gets a phantom `BYE` entry, which turns into a rotating rest slot — `n` rounds, one
//! participant sitting out each round, everybody resting exactly once.
//!
//! Home/away is balanced greedily in schedule order (the side with fewer home fixtures so far
//! hosts, ties keep circle order), so no participant hosts far more often than it travels. A
//! double round robin appends a mirrored second leg with home and away swapped, which makes the
//! home/away split exact.
//!
//! Everything here is deterministic: the same input always yields byte-identical output, including
//! the optional `seed` draw order, which uses a fixed local PRNG rather than system randomness.

use std::collections::HashSet;

/// A schedule has to stay printable and the circle method quadratic, so cap the roster.
pub const MAX_PARTICIPANTS: usize = 64;
/// A pair is the smallest thing that can play a round robin.
pub const MIN_PARTICIPANTS: usize = 2;
/// Courts/venues are labels for parallel slots within one round; more than this is not a schedule.
pub const MAX_COURTS: usize = 32;
/// Rendered for the resting participant of an odd-sized round.
pub const BYE: &str = "BYE";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScheduleType {
    /// Every pair meets once.
    Single,
    /// Every pair meets twice, the second leg mirrored (home and away swapped).
    Double,
}

impl ScheduleType {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "single" | "single-round-robin" | "1" | "once" => Ok(ScheduleType::Single),
            "double" | "double-round-robin" | "2" | "twice" | "home-and-away" => {
                Ok(ScheduleType::Double)
            }
            other => Err(format!(
                "unknown schedule_type '{other}': expected single or double"
            )),
        }
    }

    /// How many times each pair meets.
    fn legs(self) -> usize {
        match self {
            ScheduleType::Single => 1,
            ScheduleType::Double => 2,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// Round-by-round listing, one match per line.
    Text,
    /// A GitHub pipe table with a Round column.
    Markdown,
    /// `round,match,home,away[,court]`, RFC-4180 quoted.
    Csv,
    /// A flat array of fixture objects.
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
                "unknown output_format '{other}': expected text, markdown, csv, or json"
            )),
        }
    }
}

/// Everything the renderer needs beyond the roster itself.
#[derive(Clone, Debug)]
pub struct Options {
    pub schedule_type: ScheduleType,
    pub format: OutputFormat,
    /// A count ("4") or a comma-separated list of venue names; empty disables court assignment.
    pub courts: String,
    /// Number given to the first round; later rounds count up from it.
    pub start_round: i64,
    /// Show the resting participant of each odd-sized round.
    pub include_byes: bool,
    /// Prepend a summary block (text and markdown only).
    pub include_summary: bool,
    /// 0 keeps the entered order; anything else deterministically shuffles the draw.
    pub seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            schedule_type: ScheduleType::Single,
            format: OutputFormat::Text,
            courts: String::new(),
            start_round: 1,
            include_byes: true,
            include_summary: true,
            seed: 0,
        }
    }
}

/// One fixture. `away == None` marks the round's bye (the participant resting).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
    pub round: i64,
    /// 1-based within the round; `None` for a bye, which is not a match.
    pub number: Option<usize>,
    pub home: String,
    pub away: Option<String>,
    pub court: Option<String>,
}

/// The structured schedule, ahead of any rendering.
#[derive(Clone, Debug)]
pub struct Schedule {
    pub participants: Vec<String>,
    pub rounds: usize,
    pub matches: usize,
    pub matches_per_participant: usize,
    pub byes_per_participant: usize,
    pub fixtures: Vec<Fixture>,
}

/// Parse + schedule + render in one call: the entry point every surface uses.
pub fn generate(participants: &str, opts: &Options) -> Result<String, String> {
    let schedule = build(participants, opts)?;
    Ok(render(&schedule, opts))
}

/// Parse the roster and lay out the fixtures, without rendering.
pub fn build(participants: &str, opts: &Options) -> Result<Schedule, String> {
    if opts.start_round < 1 {
        return Err(format!(
            "start_round must be 1 or greater, got {}",
            opts.start_round
        ));
    }
    let mut names = parse_participants(participants)?;
    if opts.seed != 0 {
        shuffle(&mut names, opts.seed);
    }
    let courts = parse_courts(&opts.courts)?;

    let n = names.len();
    // The circle method needs an even table; an odd roster borrows a phantom seat, which becomes
    // the rotating bye. `None` is that phantom seat.
    let odd = n % 2 == 1;
    let mut seats: Vec<Option<usize>> = (0..n).map(Some).collect();
    if odd {
        seats.push(None);
    }
    let table = seats.len();
    let rounds_per_leg = table - 1;

    // Lay out one leg as unordered pairs, keeping the bye of each round beside it.
    let mut leg: Vec<(Vec<(usize, usize)>, Option<usize>)> = Vec::with_capacity(rounds_per_leg);
    for _ in 0..rounds_per_leg {
        let mut pairs = Vec::with_capacity(table / 2);
        let mut bye = None;
        for i in 0..table / 2 {
            match (seats[i], seats[table - 1 - i]) {
                (Some(a), Some(b)) => pairs.push((a, b)),
                (Some(a), None) | (None, Some(a)) => bye = Some(a),
                (None, None) => unreachable!("at most one phantom seat exists"),
            }
        }
        leg.push((pairs, bye));
        // Hold seat 0 and rotate the rest one place, which is what makes the pairings exhaustive.
        seats[1..].rotate_right(1);
    }

    // Greedy home/away balance in schedule order: whoever has hosted less so far hosts next.
    let mut home_count = vec![0usize; n];
    let mut oriented: Vec<(Vec<(usize, usize)>, Option<usize>)> = Vec::with_capacity(leg.len());
    for (pairs, bye) in &leg {
        let mut round = Vec::with_capacity(pairs.len());
        for &(a, b) in pairs {
            let (home, away) = if home_count[a] <= home_count[b] {
                (a, b)
            } else {
                (b, a)
            };
            home_count[home] += 1;
            round.push((home, away));
        }
        oriented.push((round, *bye));
    }

    balance_home_away(&mut oriented, &mut home_count, n);

    let legs = opts.schedule_type.legs();
    let mut fixtures = Vec::new();
    let mut matches = 0usize;
    for leg_index in 0..legs {
        for (round_index, (pairs, bye)) in oriented.iter().enumerate() {
            let round_no = opts.start_round + (leg_index * rounds_per_leg + round_index) as i64;
            for (i, &(home, away)) in pairs.iter().enumerate() {
                // The return leg is the mirror image, so the split comes out exactly even.
                let (home, away) = if leg_index % 2 == 1 {
                    (away, home)
                } else {
                    (home, away)
                };
                matches += 1;
                fixtures.push(Fixture {
                    round: round_no,
                    number: Some(i + 1),
                    home: names[home].clone(),
                    away: Some(names[away].clone()),
                    court: courts.as_ref().map(|c| c[i % c.len()].clone()),
                });
            }
            if let Some(b) = bye {
                if opts.include_byes {
                    fixtures.push(Fixture {
                        round: round_no,
                        number: None,
                        home: names[*b].clone(),
                        away: None,
                        court: None,
                    });
                }
            }
        }
    }

    Ok(Schedule {
        rounds: rounds_per_leg * legs,
        matches,
        matches_per_participant: (n - 1) * legs,
        byes_per_participant: if odd { legs } else { 0 },
        participants: names,
        fixtures,
    })
}

/// Even out hosting duties until nobody hosts more than one game above anyone else.
///
/// Greedy orientation alone can strand a participant two hosts clear of another (a swap of that
/// single fixture is blocked when it is the *visitor* who is over-hosting). Treat the fixture list
/// as a digraph — an edge `home → away` per match — and walk a path from the busiest host to the
/// quietest, reversing every fixture along it: the endpoints move one step toward each other and
/// everyone in between keeps their tally. Each pass strictly narrows the spread, so this ends.
fn balance_home_away(
    oriented: &mut [(Vec<(usize, usize)>, Option<usize>)],
    home_count: &mut [usize],
    n: usize,
) {
    // Where each unordered pair's fixture lives, so a path step can rewrite it in place.
    let mut loc: Vec<Vec<Option<(usize, usize)>>> = vec![vec![None; n]; n];
    for (ri, (pairs, _)) in oriented.iter().enumerate() {
        for (si, &(home, away)) in pairs.iter().enumerate() {
            loc[home][away] = Some((ri, si));
            loc[away][home] = Some((ri, si));
        }
    }

    loop {
        let max = home_count.iter().copied().max().unwrap_or(0);
        let min = home_count.iter().copied().min().unwrap_or(0);
        if max - min <= 1 {
            return;
        }
        let src = home_count.iter().position(|&c| c == max).unwrap();
        let dst = home_count.iter().position(|&c| c == min).unwrap();

        // Breadth-first walk over "u currently hosts v" edges, from the busiest to the quietest.
        let mut prev: Vec<Option<usize>> = vec![None; n];
        let mut seen = vec![false; n];
        seen[src] = true;
        let mut queue = std::collections::VecDeque::from([src]);
        while let Some(u) = queue.pop_front() {
            if u == dst {
                break;
            }
            for (pairs, _) in oriented.iter() {
                for &(home, away) in pairs {
                    if home == u && !seen[away] {
                        seen[away] = true;
                        prev[away] = Some(u);
                        queue.push_back(away);
                    }
                }
            }
        }
        if !seen[dst] {
            // No chain of hosts links the two; the orientation is as even as this draw allows.
            return;
        }

        let mut node = dst;
        while let Some(from) = prev[node] {
            let (ri, si) = loc[from][node].expect("every pair has a fixture");
            oriented[ri].0[si] = (node, from);
            home_count[from] -= 1;
            home_count[node] += 1;
            node = from;
        }
    }
}

/// Accept the roster in any of the shapes people actually paste.
fn parse_participants(input: &str) -> Result<Vec<String>, String> {
    // One per line is the documented shape; a single comma-separated line is the common alternative.
    let raw: Vec<&str> = if input.contains('\n') || input.contains('\r') {
        input.lines().collect()
    } else {
        input.split([',', ';']).collect()
    };

    let mut names: Vec<String> = Vec::new();
    for entry in raw {
        let cleaned = strip_list_marker(entry.trim());
        // Blank lines pad pasted lists; `#` lets a roster carry comments.
        if cleaned.is_empty() || cleaned.starts_with('#') {
            continue;
        }
        names.push(cleaned.to_string());
    }

    // "8" on its own means "eight unnamed teams" — the shape every competitor's count box takes.
    if names.len() == 1 {
        if let Ok(count) = names[0].parse::<usize>() {
            if count < MIN_PARTICIPANTS {
                return Err(format!(
                    "need at least {MIN_PARTICIPANTS} participants, got a count of {count}"
                ));
            }
            if count > MAX_PARTICIPANTS {
                return Err(format!(
                    "at most {MAX_PARTICIPANTS} participants are supported, got a count of {count}"
                ));
            }
            return Ok((1..=count).map(|i| format!("Team {i}")).collect());
        }
    }

    if names.len() < MIN_PARTICIPANTS {
        return Err(format!(
            "need at least {MIN_PARTICIPANTS} participants, got {} — enter one name per line, a comma-separated list, or a plain count like 8",
            names.len()
        ));
    }
    if names.len() > MAX_PARTICIPANTS {
        return Err(format!(
            "at most {MAX_PARTICIPANTS} participants are supported, got {}",
            names.len()
        ));
    }

    let mut seen: HashSet<String> = HashSet::new();
    for name in &names {
        if !seen.insert(name.to_lowercase()) {
            return Err(format!(
                "duplicate participant '{name}': names must be unique (comparison ignores case)"
            ));
        }
    }
    Ok(names)
}

/// Drop the `-`, `*`, `1.`, `1)` decoration that survives a copy-paste from a numbered list.
fn strip_list_marker(line: &str) -> &str {
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("• "));
    if let Some(rest) = rest {
        return rest.trim();
    }
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return line;
    }
    let after = &line[digits.len()..];
    for sep in [". ", ") ", "- "] {
        if let Some(rest) = after.strip_prefix(sep) {
            return rest.trim();
        }
    }
    line
}

/// `""` → no courts, `"4"` → Court 1..4, `"North, South"` → those names.
fn parse_courts(spec: &str) -> Result<Option<Vec<String>>, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec == "0" {
        return Ok(None);
    }
    if let Ok(count) = spec.parse::<usize>() {
        if count > MAX_COURTS {
            return Err(format!(
                "at most {MAX_COURTS} courts are supported, got {count}"
            ));
        }
        return Ok(Some((1..=count).map(|i| format!("Court {i}")).collect()));
    }
    let names: Vec<String> = spec
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if names.is_empty() {
        return Ok(None);
    }
    if names.len() > MAX_COURTS {
        return Err(format!(
            "at most {MAX_COURTS} courts are supported, got {}",
            names.len()
        ));
    }
    Ok(Some(names))
}

/// A tiny fixed PRNG so `seed` reorders the draw reproducibly on every surface.
fn shuffle(names: &mut [String], seed: u64) {
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    for i in (1..names.len()).rev() {
        names.swap(i, next() % (i + 1));
    }
}

fn render(schedule: &Schedule, opts: &Options) -> String {
    match opts.format {
        OutputFormat::Text => render_text(schedule, opts),
        OutputFormat::Markdown => render_markdown(schedule, opts),
        OutputFormat::Csv => render_csv(schedule),
        OutputFormat::Json => render_json(schedule),
    }
}

fn summary_lines(schedule: &Schedule, opts: &Options) -> Vec<(&'static str, String)> {
    let kind = match opts.schedule_type {
        ScheduleType::Single => "single round robin",
        ScheduleType::Double => "double round robin (home and away)",
    };
    let mut rows = vec![
        ("Format", kind.to_string()),
        ("Participants", schedule.participants.len().to_string()),
        ("Rounds", schedule.rounds.to_string()),
        ("Matches", schedule.matches.to_string()),
        (
            "Matches per participant",
            schedule.matches_per_participant.to_string(),
        ),
    ];
    if schedule.byes_per_participant > 0 {
        rows.push((
            "Byes per participant",
            schedule.byes_per_participant.to_string(),
        ));
    }
    rows
}

fn render_text(schedule: &Schedule, opts: &Options) -> String {
    let mut out = String::new();
    if opts.include_summary {
        for (label, value) in summary_lines(schedule, opts) {
            out.push_str(&format!("{label}: {value}\n"));
        }
        out.push('\n');
    }
    let mut current: Option<i64> = None;
    for f in &schedule.fixtures {
        if current != Some(f.round) {
            if current.is_some() {
                out.push('\n');
            }
            out.push_str(&format!("Round {}\n", f.round));
            current = Some(f.round);
        }
        match (&f.away, f.number) {
            (Some(away), Some(number)) => {
                out.push_str(&format!("  {number}. {} vs {away}", f.home));
                if let Some(court) = &f.court {
                    out.push_str(&format!(" — {court}"));
                }
                out.push('\n');
            }
            _ => out.push_str(&format!("  Bye: {}\n", f.home)),
        }
    }
    out.trim_end().to_string()
}

fn render_markdown(schedule: &Schedule, opts: &Options) -> String {
    let with_courts = schedule.fixtures.iter().any(|f| f.court.is_some());
    let mut out = String::new();
    if opts.include_summary {
        for (label, value) in summary_lines(schedule, opts) {
            out.push_str(&format!("- **{label}:** {value}\n"));
        }
        out.push('\n');
    }
    if with_courts {
        out.push_str("| Round | # | Home | Away | Court |\n| --- | --- | --- | --- | --- |\n");
    } else {
        out.push_str("| Round | # | Home | Away |\n| --- | --- | --- | --- |\n");
    }
    for f in &schedule.fixtures {
        let number = f.number.map(|n| n.to_string()).unwrap_or_default();
        let away = f.away.clone().unwrap_or_else(|| BYE.to_string());
        out.push_str(&format!(
            "| {} | {number} | {} | {away} |",
            f.round,
            escape_markdown(&f.home)
        ));
        if with_courts {
            out.push_str(&format!(" {} |", f.court.clone().unwrap_or_default()));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// A participant called `A|B` must not break the pipe table.
fn escape_markdown(s: &str) -> String {
    s.replace('|', "\\|")
}

fn render_csv(schedule: &Schedule) -> String {
    let with_courts = schedule.fixtures.iter().any(|f| f.court.is_some());
    let mut out = String::from("round,match,home,away");
    if with_courts {
        out.push_str(",court");
    }
    out.push('\n');
    for f in &schedule.fixtures {
        let number = f.number.map(|n| n.to_string()).unwrap_or_default();
        let away = f.away.clone().unwrap_or_else(|| BYE.to_string());
        out.push_str(&format!(
            "{},{number},{},{}",
            f.round,
            csv_field(&f.home),
            csv_field(&away)
        ));
        if with_courts {
            out.push_str(&format!(
                ",{}",
                csv_field(f.court.as_deref().unwrap_or_default())
            ));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_json(schedule: &Schedule) -> String {
    let with_courts = schedule.fixtures.iter().any(|f| f.court.is_some());
    let mut rows: Vec<String> = Vec::with_capacity(schedule.fixtures.len());
    for f in &schedule.fixtures {
        let number = f
            .number
            .map(|n| n.to_string())
            .unwrap_or_else(|| "null".to_string());
        let away = f
            .away
            .as_ref()
            .map(|a| json_string(a))
            .unwrap_or_else(|| json_string(BYE));
        let mut row = format!(
            "{{\"round\": {}, \"match\": {number}, \"home\": {}, \"away\": {away}",
            f.round,
            json_string(&f.home)
        );
        if with_courts {
            let court = f
                .court
                .as_ref()
                .map(|c| json_string(c))
                .unwrap_or_else(|| "null".to_string());
            row.push_str(&format!(", \"court\": {court}"));
        }
        row.push_str(", \"bye\": ");
        row.push_str(if f.away.is_none() { "true}" } else { "false}" });
        rows.push(row);
    }
    format!("[\n  {}\n]", rows.join(",\n  "))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(format: OutputFormat) -> Options {
        Options {
            format,
            include_summary: false,
            ..Options::default()
        }
    }

    #[test]
    fn four_participants_play_every_pair_once_over_three_rounds() {
        let out = generate("Alice\nBob\nCarol\nDave", &opts(OutputFormat::Text)).unwrap();
        assert_eq!(
            out,
            "Round 1\n  1. Alice vs Dave\n  2. Bob vs Carol\n\
             \n\
             Round 2\n  1. Carol vs Alice\n  2. Dave vs Bob\n\
             \n\
             Round 3\n  1. Alice vs Bob\n  2. Carol vs Dave"
        );
    }

    #[test]
    fn every_pair_appears_exactly_once() {
        let schedule = build("A\nB\nC\nD\nE\nF", &Options::default()).unwrap();
        let mut pairs: Vec<(String, String)> = schedule
            .fixtures
            .iter()
            .filter_map(|f| {
                f.away.as_ref().map(|a| {
                    let mut p = [f.home.clone(), a.clone()];
                    p.sort();
                    (p[0].clone(), p[1].clone())
                })
            })
            .collect();
        pairs.sort();
        let unique: HashSet<_> = pairs.iter().cloned().collect();
        assert_eq!(pairs.len(), 15, "6 participants make 15 matches");
        assert_eq!(unique.len(), 15, "no pair repeats");
        assert_eq!(schedule.rounds, 5);
    }

    #[test]
    fn odd_roster_gets_a_rotating_bye_so_everyone_rests_once() {
        let schedule = build("A\nB\nC\nD\nE", &Options::default()).unwrap();
        assert_eq!(schedule.rounds, 5);
        assert_eq!(schedule.matches, 10);
        assert_eq!(schedule.byes_per_participant, 1);
        let mut resting: Vec<String> = schedule
            .fixtures
            .iter()
            .filter(|f| f.away.is_none())
            .map(|f| f.home.clone())
            .collect();
        resting.sort();
        assert_eq!(resting, vec!["A", "B", "C", "D", "E"]);
    }

    #[test]
    fn byes_can_be_hidden() {
        let out = generate(
            "A\nB\nC",
            &Options {
                include_byes: false,
                include_summary: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(!out.contains("Bye"), "byes suppressed, got:\n{out}");
        assert_eq!(out.matches("Round").count(), 3);
    }

    #[test]
    fn double_round_robin_mirrors_the_second_leg() {
        let schedule = build(
            "A\nB\nC\nD",
            &Options {
                schedule_type: ScheduleType::Double,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(schedule.rounds, 6);
        assert_eq!(schedule.matches, 12);
        assert_eq!(schedule.matches_per_participant, 6);
        let first: Vec<_> = schedule.fixtures[..6]
            .iter()
            .map(|f| (f.home.clone(), f.away.clone().unwrap()))
            .collect();
        let second: Vec<_> = schedule.fixtures[6..]
            .iter()
            .map(|f| (f.away.clone().unwrap(), f.home.clone()))
            .collect();
        assert_eq!(first, second, "leg 2 is leg 1 with the venue swapped");
    }

    #[test]
    fn home_and_away_stay_balanced() {
        for n in 2..=16usize {
            let roster: Vec<String> = (1..=n).map(|i| format!("T{i}")).collect();
            let schedule = build(&roster.join("\n"), &Options::default()).unwrap();
            let mut home: Vec<usize> = roster
                .iter()
                .map(|name| {
                    schedule
                        .fixtures
                        .iter()
                        .filter(|f| f.away.is_some() && &f.home == name)
                        .count()
                })
                .collect();
            home.sort();
            assert!(
                home[home.len() - 1] - home[0] <= 1,
                "n={n}: home counts {home:?} differ by more than one"
            );
        }
    }

    #[test]
    fn courts_cycle_within_each_round() {
        let out = generate(
            "A\nB\nC\nD",
            &Options {
                courts: "2".into(),
                include_summary: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("— Court 1"), "got:\n{out}");
        assert!(out.contains("— Court 2"), "got:\n{out}");
    }

    #[test]
    fn named_venues_are_used_verbatim() {
        let out = generate(
            "A\nB\nC\nD",
            &Options {
                courts: "North Field, South Field".into(),
                include_summary: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.contains("— North Field"), "got:\n{out}");
        assert!(out.contains("— South Field"), "got:\n{out}");
    }

    #[test]
    fn start_round_offsets_the_numbering() {
        let out = generate(
            "A\nB\nC\nD",
            &Options {
                start_round: 10,
                include_summary: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(out.starts_with("Round 10\n"), "got:\n{out}");
        assert!(out.contains("Round 12\n"), "got:\n{out}");
    }

    #[test]
    fn csv_has_a_header_and_quotes_embedded_commas() {
        let out = generate("Smith, J\nB\nC", &opts(OutputFormat::Csv)).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "round,match,home,away");
        assert!(out.contains("\"Smith, J\""), "got:\n{out}");
        assert!(out.contains(",,"), "a bye row leaves the match number empty");
        assert!(out.contains(",BYE"), "got:\n{out}");
    }

    #[test]
    fn csv_adds_a_court_column_only_when_courts_are_set() {
        let plain = generate("A\nB\nC\nD", &opts(OutputFormat::Csv)).unwrap();
        assert_eq!(plain.lines().next().unwrap(), "round,match,home,away");
        let with_courts = generate(
            "A\nB\nC\nD",
            &Options {
                format: OutputFormat::Csv,
                courts: "2".into(),
                include_summary: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            with_courts.lines().next().unwrap(),
            "round,match,home,away,court"
        );
    }

    #[test]
    fn json_is_a_flat_array_of_fixtures() {
        let out = generate("A\nB\nC\nD", &opts(OutputFormat::Json)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let rows = parsed.as_array().unwrap();
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0]["round"], 1);
        assert_eq!(rows[0]["match"], 1);
        assert_eq!(rows[0]["bye"], false);
        assert!(rows[0].get("court").is_none(), "no court key without courts");
    }

    #[test]
    fn json_marks_byes_and_keeps_one_shape() {
        let out = generate("A\nB\nC", &opts(OutputFormat::Json)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let byes: Vec<_> = parsed
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["bye"] == true)
            .collect();
        assert_eq!(byes.len(), 3);
        assert_eq!(byes[0]["away"], "BYE");
        assert_eq!(byes[0]["match"], serde_json::Value::Null);
    }

    #[test]
    fn markdown_renders_a_table_with_a_round_column() {
        let out = generate("A\nB\nC\nD", &opts(OutputFormat::Markdown)).unwrap();
        assert!(out.starts_with("| Round | # | Home | Away |\n| --- |"));
        assert_eq!(out.lines().count(), 8, "header + separator + 6 matches");
    }

    #[test]
    fn summary_precedes_the_schedule() {
        let out = generate("A\nB\nC", &Options::default()).unwrap();
        assert!(out.starts_with("Format: single round robin\n"), "got:\n{out}");
        assert!(out.contains("Byes per participant: 1"), "got:\n{out}");
    }

    #[test]
    fn a_bare_count_expands_to_numbered_teams() {
        let out = generate("8", &opts(OutputFormat::Text)).unwrap();
        assert!(out.contains("Team 1"), "got:\n{out}");
        assert!(out.contains("Team 8"), "got:\n{out}");
        assert!(out.contains("Round 7\n"), "8 teams play 7 rounds");
    }

    #[test]
    fn commas_bullets_numbering_and_comments_all_parse() {
        let schedule = build(
            "# my club\n- Alice\n* Bob\n3. Carol\n\n4) Dave",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(schedule.participants, vec!["Alice", "Bob", "Carol", "Dave"]);
        let inline = build("Alice, Bob, Carol", &Options::default()).unwrap();
        assert_eq!(inline.participants, vec!["Alice", "Bob", "Carol"]);
    }

    #[test]
    fn seeding_reorders_deterministically() {
        let seeded = Options {
            seed: 42,
            ..Options::default()
        };
        let a = build("A\nB\nC\nD\nE\nF", &seeded).unwrap();
        let b = build("A\nB\nC\nD\nE\nF", &seeded).unwrap();
        assert_eq!(a.participants, b.participants, "same seed, same draw");
        assert_ne!(
            a.participants,
            build("A\nB\nC\nD\nE\nF", &Options::default())
                .unwrap()
                .participants,
            "a seed should change the draw order"
        );
        let mut sorted = a.participants.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["A", "B", "C", "D", "E", "F"], "nobody is lost");
    }

    #[test]
    fn errors() {
        let e = generate("Alice", &Options::default()).unwrap_err();
        assert!(e.contains("at least 2 participants"), "{e}");

        let e = generate("Alice\nalice\nBob", &Options::default()).unwrap_err();
        assert!(e.contains("duplicate participant 'alice'"), "{e}");

        let roster: Vec<String> = (1..=65).map(|i| format!("T{i}")).collect();
        let e = generate(&roster.join("\n"), &Options::default()).unwrap_err();
        assert!(e.contains("at most 64 participants"), "{e}");

        let e = generate("100", &Options::default()).unwrap_err();
        assert!(e.contains("at most 64 participants"), "{e}");

        let e = generate(
            "A\nB",
            &Options {
                start_round: 0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(e.contains("start_round must be 1 or greater"), "{e}");

        let e = generate(
            "A\nB",
            &Options {
                courts: "99".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(e.contains("at most 32 courts"), "{e}");

        let e = OutputFormat::parse("yaml").unwrap_err();
        assert!(e.contains("expected text, markdown, csv, or json"), "{e}");

        let e = ScheduleType::parse("triple").unwrap_err();
        assert!(e.contains("expected single or double"), "{e}");
    }
}
