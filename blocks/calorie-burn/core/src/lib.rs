//! calorie-burn core — pure compute, shared by the chat skill block and the web page.
//!
//! Estimates the energy cost of one bout of activity from its metabolic
//! equivalent (MET), the body weight and the elapsed time, using the standard
//! form `kcal/min = MET x 3.5 x kg / 200`. One MET is the resting rate, about
//! 1 kcal per kg of body weight per hour, equivalently an oxygen uptake of
//! 3.5 ml/kg/min.
//!
//! Alongside the session total it reports the per-minute and per-hour rate, the
//! MET-minutes of activity volume (what public-health guidance is written in),
//! the implied oxygen uptake, a body-fat equivalent at 7700 kcal/kg, and the
//! same session priced against eight reference activities. `basis = "net"`
//! subtracts the ~1 MET the body would have spent at rest, which is the honest
//! number for calories *added* by the session. No I/O; deterministic.

/// Grams of body fat per kcal, from the conventional 7700 kcal per kg.
const KCAL_PER_KG_FAT: f64 = 7700.0;
/// The lower bound of the usual 500-1000 MET-min/week public-health target.
const WEEKLY_MET_MIN_TARGET: f64 = 500.0;
/// Oxygen uptake of one MET, in ml per kg per minute.
const ML_O2_PER_MET: f64 = 3.5;
/// International-pound definition, used for both weight conversions.
const KG_PER_LB: f64 = 0.453_592_37;

const WEIGHT_UNITS: [&str; 2] = ["kg", "lb"];
const DURATION_UNITS: [&str; 2] = ["minutes", "hours"];
const BASES: [&str; 2] = ["gross", "net"];
const OUTPUTS: [&str; 3] = ["summary", "table", "json"];

/// Accepted `activity` value that switches the MET value over to `met`.
const CUSTOM: &str = "custom";

const WEIGHT_KG_MIN: f64 = 20.0;
const WEIGHT_KG_MAX: f64 = 300.0;
const DURATION_MIN_MINUTES: f64 = 0.5;
const DURATION_MAX_MINUTES: f64 = 1440.0;
const MET_MIN: f64 = 0.5;
const MET_MAX: f64 = 30.0;

/// One entry of the activity table: the `activity` value, its display label and
/// its metabolic equivalent. Values are the standard Compendium of Physical
/// Activities figures; for a band of intensities the mid-band value is used and
/// the label names the intensity, so `met` can override it.
struct Activity {
    key: &'static str,
    label: &'static str,
    met: f64,
}

const ACTIVITIES: [Activity; 59] = [
    // Everyday and household
    Activity { key: "sleeping", label: "Sleeping", met: 0.95 },
    Activity { key: "sitting-desk-work", label: "Sitting, desk work", met: 1.5 },
    Activity { key: "standing-light-work", label: "Standing, light work", met: 2.0 },
    Activity { key: "cooking", label: "Cooking and washing up", met: 3.3 },
    Activity { key: "house-cleaning", label: "House cleaning, general", met: 3.5 },
    Activity { key: "gardening", label: "Gardening, general", met: 3.8 },
    Activity { key: "mowing-lawn", label: "Mowing the lawn, walking mower", met: 5.0 },
    Activity { key: "moving-furniture", label: "Moving furniture and boxes", met: 5.8 },
    Activity { key: "shoveling-snow", label: "Shovelling snow by hand", met: 6.0 },
    Activity { key: "stairs-climbing", label: "Climbing stairs, continuous", met: 8.8 },
    // Walking and hiking
    Activity { key: "walking-slow", label: "Walking, slow (2 mph / 3.2 km/h)", met: 2.8 },
    Activity { key: "walking-moderate", label: "Walking, moderate (3 mph / 4.8 km/h)", met: 3.5 },
    Activity { key: "walking-brisk", label: "Walking, brisk (4 mph / 6.4 km/h)", met: 5.0 },
    Activity { key: "walking-uphill", label: "Walking uphill, moderate pace", met: 6.3 },
    Activity { key: "hiking", label: "Hiking, cross-country", met: 6.0 },
    Activity { key: "backpacking", label: "Backpacking with a loaded pack", met: 7.0 },
    // Running
    Activity { key: "jogging", label: "Jogging, general", met: 7.0 },
    Activity { key: "running-6mph", label: "Running, 6 mph / 9.7 km/h (10 min/mi)", met: 9.8 },
    Activity { key: "running-8mph", label: "Running, 8 mph / 12.9 km/h (7.5 min/mi)", met: 11.8 },
    Activity { key: "running-10mph", label: "Running, 10 mph / 16.1 km/h (6 min/mi)", met: 14.5 },
    Activity { key: "running-trail", label: "Running, trail or cross-country", met: 9.0 },
    // Cycling
    Activity { key: "cycling-leisure", label: "Cycling, leisurely (under 10 mph)", met: 4.0 },
    Activity { key: "cycling-moderate", label: "Cycling, moderate (12-14 mph)", met: 8.0 },
    Activity { key: "cycling-vigorous", label: "Cycling, vigorous (16-19 mph)", met: 12.0 },
    Activity { key: "mountain-biking", label: "Mountain biking", met: 8.5 },
    Activity { key: "indoor-cycling-class", label: "Indoor cycling class, vigorous", met: 8.5 },
    // Swimming and water
    Activity { key: "swimming-leisure", label: "Swimming, leisurely", met: 6.0 },
    Activity { key: "swimming-laps-moderate", label: "Swimming laps, freestyle moderate", met: 8.3 },
    Activity { key: "swimming-laps-vigorous", label: "Swimming laps, freestyle vigorous", met: 9.8 },
    Activity { key: "water-aerobics", label: "Water aerobics", met: 5.5 },
    Activity { key: "kayaking", label: "Kayaking or canoeing", met: 5.0 },
    Activity { key: "rowing-water", label: "Rowing on water, moderate", met: 7.0 },
    // Gym and studio
    Activity { key: "weight-training-light", label: "Weight training, light or moderate", met: 3.5 },
    Activity { key: "weight-training-vigorous", label: "Weight training, vigorous", met: 6.0 },
    Activity { key: "circuit-training", label: "Circuit training, general", met: 7.2 },
    Activity { key: "calisthenics-vigorous", label: "Calisthenics, vigorous", met: 8.0 },
    Activity { key: "rowing-machine", label: "Rowing machine, moderate", met: 7.0 },
    Activity { key: "elliptical", label: "Elliptical trainer, moderate", met: 5.0 },
    Activity { key: "stair-stepper", label: "Stair stepper machine", met: 9.0 },
    Activity { key: "jump-rope", label: "Jumping rope, moderate", met: 11.0 },
    Activity { key: "yoga-hatha", label: "Yoga, hatha", met: 2.5 },
    Activity { key: "pilates", label: "Pilates, general", met: 3.0 },
    Activity { key: "stretching", label: "Stretching or mobility work", met: 2.3 },
    Activity { key: "tai-chi", label: "Tai chi", met: 3.0 },
    // Sports
    Activity { key: "basketball-game", label: "Basketball, game", met: 8.0 },
    Activity { key: "soccer-casual", label: "Football or soccer, casual", met: 7.0 },
    Activity { key: "soccer-competitive", label: "Football or soccer, competitive", met: 10.0 },
    Activity { key: "tennis-singles", label: "Tennis, singles", met: 8.0 },
    Activity { key: "tennis-doubles", label: "Tennis, doubles", met: 6.0 },
    Activity { key: "volleyball", label: "Volleyball, recreational", met: 4.0 },
    Activity { key: "badminton", label: "Badminton, social", met: 5.5 },
    Activity { key: "table-tennis", label: "Table tennis", met: 4.0 },
    Activity { key: "golf-walking", label: "Golf, walking and carrying clubs", met: 4.8 },
    Activity { key: "bowling", label: "Bowling", met: 3.8 },
    Activity { key: "dancing-social", label: "Dancing, social or ballroom", met: 4.5 },
    Activity { key: "dancing-aerobic", label: "Dancing, aerobic or high-impact", met: 7.3 },
    Activity { key: "martial-arts", label: "Martial arts or kickboxing", met: 10.3 },
    Activity { key: "boxing-bag", label: "Boxing, punching bag", met: 5.5 },
    Activity { key: "rock-climbing", label: "Rock climbing, ascending", met: 8.0 },
];

/// The comparison block priced in every output: a spread from sedentary to hard
/// so the chosen activity has context without another run of the tool.
const COMPARISON: [&str; 8] = [
    "sitting-desk-work",
    "walking-moderate",
    "walking-brisk",
    "cycling-moderate",
    "jogging",
    "running-6mph",
    "swimming-laps-moderate",
    "weight-training-vigorous",
];

/// Every accepted `activity` value, in descriptor order (`custom` last).
pub fn activity_keys() -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = ACTIVITIES.iter().map(|a| a.key).collect();
    keys.push(CUSTOM);
    keys
}

/// `(value, label, met)` for every table entry — used to build the page's
/// friendly select labels and the docs without duplicating the MET numbers.
pub fn activity_table() -> Vec<(&'static str, &'static str, f64)> {
    ACTIVITIES.iter().map(|a| (a.key, a.label, a.met)).collect()
}

fn find_activity(key: &str) -> Option<&'static Activity> {
    ACTIVITIES.iter().find(|a| a.key == key)
}

/// Trim a whole-number float so `30` prints as `30`, not `30.0`.
fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// One decimal place, with a trailing `.0` trimmed.
fn fmt1(v: f64) -> String {
    fmt_num(round1(v))
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn whole(v: f64) -> i64 {
    v.round() as i64
}

fn check_choice(name: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "expected {name} to be one of {}, got `{value}`",
            allowed.join(", ")
        ))
    }
}

fn check_finite(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("expected a number for {name}, got `{value}`"))
    }
}

/// Minutes of activity, whatever unit they arrived in.
fn to_minutes(duration: f64, unit: &str) -> f64 {
    if unit == "hours" {
        duration * 60.0
    } else {
        duration
    }
}

/// Body weight in kilograms, whatever unit it arrived in.
fn to_kg(weight: f64, unit: &str) -> f64 {
    if unit == "lb" {
        weight * KG_PER_LB
    } else {
        weight
    }
}

/// kcal per minute for one MET value at one body weight.
fn kcal_per_minute(met: f64, kg: f64) -> f64 {
    met * ML_O2_PER_MET * kg / 200.0
}

/// The MET actually charged for the session: `net` credits back the ~1 MET of
/// resting metabolism the body would have spent anyway, floored at zero so a
/// sub-resting activity reports 0 rather than a negative burn.
fn effective_met(met: f64, basis: &str) -> f64 {
    if basis == "net" {
        (met - 1.0).max(0.0)
    } else {
        met
    }
}

fn basis_text(basis: &str, met: f64) -> String {
    if basis == "net" {
        format!(
            "net — only the energy above resting metabolism, so {} - 1 = {} MET is charged",
            fmt_num(met),
            fmt_num(effective_met(met, basis))
        )
    } else {
        "gross — all energy used during the session, resting metabolism included".to_string()
    }
}

/// Everything the renderers need, computed once.
struct Computed {
    activity_key: String,
    label: String,
    met: f64,
    met_source: &'static str,
    kg: f64,
    lb: f64,
    minutes: f64,
    per_minute: f64,
    total: f64,
    per_hour: f64,
    met_minutes: f64,
    sessions_to_target: f64,
    vo2: f64,
    oxygen_litres: f64,
    fat_grams: f64,
}

/// `(label, met, kcal)` for one comparison row.
fn comparison_rows(kg: f64, minutes: f64, basis: &str) -> Vec<(&'static str, f64, i64)> {
    COMPARISON
        .iter()
        .filter_map(|key| find_activity(key))
        .map(|a| {
            let kcal = kcal_per_minute(effective_met(a.met, basis), kg) * minutes;
            (a.label, a.met, whole(kcal))
        })
        .collect()
}

fn weight_text(weight: f64, unit: &str, kg: f64, lb: f64) -> String {
    if unit == "lb" {
        format!("{} lb ({} kg)", fmt_num(weight), fmt1(kg))
    } else {
        format!("{} kg ({} lb)", fmt_num(weight), fmt1(lb))
    }
}

fn duration_text(duration: f64, unit: &str, minutes: f64) -> String {
    if unit == "hours" {
        format!("{} h ({} min)", fmt_num(duration), fmt1(minutes))
    } else {
        format!("{} min", fmt_num(duration))
    }
}

fn render_summary(c: &Computed, weight: f64, weight_unit: &str, duration: f64, duration_unit: &str, basis: &str) -> String {
    let rows = comparison_rows(c.kg, c.minutes, basis);
    let width = rows
        .iter()
        .map(|(label, _, _)| label.chars().count())
        .max()
        .unwrap_or(24);
    let compare: Vec<String> = rows
        .iter()
        .map(|(label, met, kcal)| {
            format!(
                "  {label:<width$}  {:>5} MET  {:>6} kcal",
                fmt_num(*met),
                kcal
            )
        })
        .collect();

    format!(
        "Calories burned — {label}\n\
         \n\
         Body weight {weight_text} · duration {duration_text} · MET {met} ({met_source})\n\
         Basis: {basis_text}\n\
         \n\
         Energy burned        {total} kcal\n\
         Per minute           {per_minute} kcal/min\n\
         Per hour             {per_hour} kcal/h\n\
         Activity volume      {met_minutes} MET-minutes — {sessions} such sessions reach the 500 MET-min/week minimum\n\
         Oxygen uptake        {vo2} ml/kg/min — {litres} L of oxygen over the session\n\
         Body-fat equivalent  {fat} g — at 7700 kcal per kg of body fat\n\
         \n\
         Same {duration_short} at {weight_short} for comparison\n\
         {compare}",
        label = c.label,
        weight_text = weight_text(weight, weight_unit, c.kg, c.lb),
        duration_text = duration_text(duration, duration_unit, c.minutes),
        met = fmt_num(c.met),
        met_source = c.met_source,
        basis_text = basis_text(basis, c.met),
        total = whole(c.total),
        per_minute = fmt1(c.per_minute),
        per_hour = whole(c.per_hour),
        met_minutes = fmt1(c.met_minutes),
        sessions = fmt1(c.sessions_to_target),
        vo2 = fmt1(c.vo2),
        litres = fmt1(c.oxygen_litres),
        fat = whole(c.fat_grams),
        duration_short = format!("{} min", fmt1(c.minutes)),
        weight_short = format!("{} kg", fmt1(c.kg)),
        compare = compare.join("\n"),
    )
}

fn render_table(c: &Computed, weight: f64, weight_unit: &str, duration: f64, duration_unit: &str, basis: &str) -> String {
    let compare: Vec<String> = comparison_rows(c.kg, c.minutes, basis)
        .iter()
        .map(|(label, met, kcal)| format!("| {label} | {} | {kcal} |", fmt_num(*met)))
        .collect();

    format!(
        "## Calories burned\n\
         \n\
         | Field | Value |\n\
         | --- | --- |\n\
         | Activity | {label} |\n\
         | MET | {met} ({met_source}) |\n\
         | Body weight | {weight_text} |\n\
         | Duration | {duration_text} |\n\
         | Basis | {basis} |\n\
         | Energy burned | {total} kcal |\n\
         | Per minute | {per_minute} kcal/min |\n\
         | Per hour | {per_hour} kcal/h |\n\
         | Activity volume | {met_minutes} MET-minutes |\n\
         | Oxygen uptake | {vo2} ml/kg/min ({litres} L total) |\n\
         | Body-fat equivalent | {fat} g |\n\
         \n\
         ## Same {duration_short} at {weight_short}\n\
         \n\
         | Activity | MET | kcal |\n\
         | --- | --- | --- |\n\
         {compare}",
        label = c.label,
        met = fmt_num(c.met),
        met_source = c.met_source,
        weight_text = weight_text(weight, weight_unit, c.kg, c.lb),
        duration_text = duration_text(duration, duration_unit, c.minutes),
        total = whole(c.total),
        per_minute = fmt1(c.per_minute),
        per_hour = whole(c.per_hour),
        met_minutes = fmt1(c.met_minutes),
        vo2 = fmt1(c.vo2),
        litres = fmt1(c.oxygen_litres),
        fat = whole(c.fat_grams),
        duration_short = format!("{} min", fmt1(c.minutes)),
        weight_short = format!("{} kg", fmt1(c.kg)),
        compare = compare.join("\n"),
    )
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_json(c: &Computed, basis: &str) -> String {
    let compare: Vec<String> = comparison_rows(c.kg, c.minutes, basis)
        .iter()
        .map(|(label, met, kcal)| {
            format!(
                "    {{ \"label\": \"{}\", \"met\": {}, \"calories\": {} }}",
                json_escape(label),
                fmt_num(*met),
                kcal
            )
        })
        .collect();

    format!(
        "{{\n\
         \x20 \"activity\": \"{activity}\",\n\
         \x20 \"activity_label\": \"{label}\",\n\
         \x20 \"met\": {met},\n\
         \x20 \"met_source\": \"{met_source}\",\n\
         \x20 \"met_charged\": {charged},\n\
         \x20 \"basis\": \"{basis}\",\n\
         \x20 \"weight_kg\": {kg},\n\
         \x20 \"weight_lb\": {lb},\n\
         \x20 \"duration_minutes\": {minutes},\n\
         \x20 \"calories\": {total},\n\
         \x20 \"kcal_per_minute\": {per_minute},\n\
         \x20 \"kcal_per_hour\": {per_hour},\n\
         \x20 \"met_minutes\": {met_minutes},\n\
         \x20 \"sessions_to_500_met_minutes\": {sessions},\n\
         \x20 \"vo2_ml_per_kg_per_min\": {vo2},\n\
         \x20 \"oxygen_litres\": {litres},\n\
         \x20 \"fat_grams\": {fat},\n\
         \x20 \"comparison\": [\n\
         {compare}\n\
         \x20 ]\n\
         }}",
        activity = json_escape(&c.activity_key),
        label = json_escape(&c.label),
        met = fmt_num(c.met),
        met_source = c.met_source,
        charged = fmt_num(effective_met(c.met, basis)),
        basis = basis,
        kg = fmt1(c.kg),
        lb = fmt1(c.lb),
        minutes = fmt1(c.minutes),
        total = whole(c.total),
        per_minute = fmt1(c.per_minute),
        per_hour = whole(c.per_hour),
        met_minutes = fmt1(c.met_minutes),
        sessions = fmt1(c.sessions_to_target),
        vo2 = fmt1(c.vo2),
        litres = fmt1(c.oxygen_litres),
        fat = whole(c.fat_grams),
        compare = compare.join(",\n"),
    )
}

/// Estimate the calories burned by one bout of activity.
///
/// * `weight` — body weight in `weight_unit`; 20-300 kg (44.1-661.4 lb) once converted.
/// * `weight_unit` — `kg` | `lb`.
/// * `duration` — session length in `duration_unit`; 0.5-1440 minutes once converted.
/// * `duration_unit` — `minutes` | `hours`.
/// * `activity` — an activity-table value, or `custom` to supply `met` yourself.
/// * `met` — metabolic equivalent override, 0.5-30, or 0 to use the activity's value.
/// * `basis` — `gross` (all energy used) | `net` (energy above resting metabolism).
/// * `output` — `summary` | `table` | `json`.
pub fn run(
    weight: f64,
    weight_unit: &str,
    duration: f64,
    duration_unit: &str,
    activity: &str,
    met: f64,
    basis: &str,
    output: &str,
) -> Result<String, String> {
    check_choice("weight_unit", weight_unit, &WEIGHT_UNITS)?;
    check_choice("duration_unit", duration_unit, &DURATION_UNITS)?;
    check_choice("basis", basis, &BASES)?;
    check_choice("output", output, &OUTPUTS)?;
    if activity != CUSTOM && find_activity(activity).is_none() {
        return Err(format!(
            "unknown activity `{activity}`. Pass one of the {} listed activity values, or \
             `custom` together with a met value. Close matches are easiest to find by \
             prefix, e.g. running-6mph, cycling-moderate, walking-brisk.",
            ACTIVITIES.len()
        ));
    }
    check_finite("weight", weight)?;
    check_finite("duration", duration)?;
    check_finite("met", met)?;

    let kg = to_kg(weight, weight_unit);
    if kg < WEIGHT_KG_MIN || kg > WEIGHT_KG_MAX {
        return Err(format!(
            "expected body weight between {} and {} kg ({} and {} lb), got {} {weight_unit} ({} kg)",
            fmt_num(WEIGHT_KG_MIN),
            fmt_num(WEIGHT_KG_MAX),
            fmt1(WEIGHT_KG_MIN / KG_PER_LB),
            fmt1(WEIGHT_KG_MAX / KG_PER_LB),
            fmt_num(weight),
            fmt1(kg)
        ));
    }

    let minutes = to_minutes(duration, duration_unit);
    if minutes < DURATION_MIN_MINUTES || minutes > DURATION_MAX_MINUTES {
        return Err(format!(
            "expected duration between {} and {} minutes (up to 24 hours), got {} {duration_unit} ({} min)",
            fmt_num(DURATION_MIN_MINUTES),
            fmt_num(DURATION_MAX_MINUTES),
            fmt_num(duration),
            fmt1(minutes)
        ));
    }

    if met != 0.0 && (met < MET_MIN || met > MET_MAX) {
        return Err(format!(
            "expected met between {} and {} (or 0 to use the activity's own value), got {}",
            fmt_num(MET_MIN),
            fmt_num(MET_MAX),
            fmt_num(met)
        ));
    }
    if activity == CUSTOM && met == 0.0 {
        return Err(format!(
            "activity `custom` needs its own met value: pass met between {} and {}, \
             e.g. met=7.5 for a hard effort",
            fmt_num(MET_MIN),
            fmt_num(MET_MAX)
        ));
    }

    let (label, table_met) = match find_activity(activity) {
        Some(a) => (a.label.to_string(), a.met),
        None => ("Custom activity".to_string(), 0.0),
    };
    let overridden = met != 0.0;
    let met_value = if overridden { met } else { table_met };
    let met_source = if overridden {
        "your own MET value"
    } else {
        "Compendium value for this activity"
    };
    let label = if overridden && activity != CUSTOM {
        format!("{label} at MET {}", fmt_num(met_value))
    } else {
        label
    };

    let charged = effective_met(met_value, basis);
    let per_minute = kcal_per_minute(charged, kg);
    let total = per_minute * minutes;
    let met_minutes = met_value * minutes;
    let c = Computed {
        activity_key: activity.to_string(),
        label,
        met: met_value,
        met_source,
        kg,
        lb: kg / KG_PER_LB,
        minutes,
        per_minute,
        total,
        per_hour: per_minute * 60.0,
        met_minutes,
        sessions_to_target: if met_minutes > 0.0 {
            WEEKLY_MET_MIN_TARGET / met_minutes
        } else {
            0.0
        },
        vo2: met_value * ML_O2_PER_MET,
        oxygen_litres: met_value * ML_O2_PER_MET * kg * minutes / 1000.0,
        fat_grams: total / KCAL_PER_KG_FAT * 1000.0,
    };

    Ok(match output {
        "json" => render_json(&c, basis),
        "table" => render_table(&c, weight, weight_unit, duration, duration_unit, basis),
        _ => render_summary(&c, weight, weight_unit, duration, duration_unit, basis),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_run(output: &str) -> String {
        run(70.0, "kg", 30.0, "minutes", "walking-moderate", 0.0, "gross", output).unwrap()
    }

    /// Happy path: 70 kg walking 30 min at MET 3.5 is 3.5 x 3.5 x 70 / 200 =
    /// 4.29 kcal/min, so 129 kcal for the session.
    #[test]
    fn walking_thirty_minutes_at_seventy_kilos() {
        let out = default_run("summary");
        assert!(out.contains("Calories burned — Walking, moderate (3 mph / 4.8 km/h)"), "{out}");
        assert!(out.contains("Body weight 70 kg (154.3 lb) · duration 30 min · MET 3.5"), "{out}");
        assert!(out.contains("Energy burned        129 kcal"), "{out}");
        assert!(out.contains("Per minute           4.3 kcal/min"), "{out}");
        assert!(out.contains("Per hour             257 kcal/h"), "{out}");
        assert!(out.contains("Activity volume      105 MET-minutes"), "{out}");
        assert!(out.contains("Oxygen uptake        12.3 ml/kg/min — 25.7 L"), "{out}");
        assert!(out.contains("Body-fat equivalent  17 g"), "{out}");
    }

    /// Error path: an activity value that is not in the table names the fix.
    #[test]
    fn unknown_activity_is_rejected() {
        let err = run(70.0, "kg", 30.0, "minutes", "quidditch", 0.0, "gross", "summary").unwrap_err();
        assert!(err.contains("unknown activity `quidditch`"), "{err}");
        assert!(err.contains("`custom` together with a met value"), "{err}");
    }

    #[test]
    fn custom_activity_requires_a_met_value() {
        let err = run(70.0, "kg", 30.0, "minutes", "custom", 0.0, "gross", "summary").unwrap_err();
        assert!(err.contains("activity `custom` needs its own met value"), "{err}");
        let ok = run(70.0, "kg", 30.0, "minutes", "custom", 7.5, "gross", "summary").unwrap();
        assert!(ok.contains("Custom activity"), "{ok}");
        assert!(ok.contains("MET 7.5 (your own MET value)"), "{ok}");
    }

    /// A MET override on a named activity keeps the activity's name and says so.
    #[test]
    fn met_override_relabels_a_named_activity() {
        let out = run(70.0, "kg", 30.0, "minutes", "walking-moderate", 4.5, "gross", "summary").unwrap();
        assert!(out.contains("Walking, moderate (3 mph / 4.8 km/h) at MET 4.5"), "{out}");
        assert!(out.contains("MET 4.5 (your own MET value)"), "{out}");
        // 4.5 x 3.5 x 70 / 200 x 30 = 165.4 kcal
        assert!(out.contains("Energy burned        165 kcal"), "{out}");
    }

    /// Net basis charges MET - 1, so the same walk drops from 129 to 92 kcal.
    #[test]
    fn net_basis_credits_back_resting_metabolism() {
        let out = run(70.0, "kg", 30.0, "minutes", "walking-moderate", 0.0, "net", "summary").unwrap();
        assert!(out.contains("net — only the energy above resting metabolism, so 3.5 - 1 = 2.5 MET is charged"), "{out}");
        assert!(out.contains("Energy burned        92 kcal"), "{out}");
        // MET-minutes stay defined on the gross MET.
        assert!(out.contains("Activity volume      105 MET-minutes"), "{out}");
    }

    /// A sub-resting activity on the net basis floors at zero rather than
    /// reporting a negative burn.
    #[test]
    fn sub_resting_activity_on_net_basis_is_zero() {
        let out = run(70.0, "kg", 60.0, "minutes", "sleeping", 0.0, "net", "summary").unwrap();
        assert!(out.contains("Energy burned        0 kcal"), "{out}");
        assert!(out.contains("= 0 MET is charged"), "{out}");
    }

    #[test]
    fn pounds_and_hours_convert_before_the_arithmetic() {
        let out = run(154.0, "lb", 1.0, "hours", "running-6mph", 0.0, "gross", "summary").unwrap();
        assert!(out.contains("Body weight 154 lb (69.9 kg)"), "{out}");
        assert!(out.contains("duration 1 h (60 min)"), "{out}");
        // 9.8 x 3.5 x 69.85 / 200 x 60 = 718.9 kcal
        assert!(out.contains("Energy burned        719 kcal"), "{out}");
    }

    #[test]
    fn out_of_range_weight_and_duration_are_rejected() {
        let err = run(10.0, "kg", 30.0, "minutes", "jogging", 0.0, "gross", "summary").unwrap_err();
        assert!(err.contains("expected body weight between 20 and 300 kg"), "{err}");
        let err = run(70.0, "kg", 25.0, "hours", "jogging", 0.0, "gross", "summary").unwrap_err();
        assert!(err.contains("expected duration between 0.5 and 1440 minutes"), "{err}");
        let err = run(70.0, "kg", 30.0, "minutes", "jogging", 44.0, "gross", "summary").unwrap_err();
        assert!(err.contains("expected met between 0.5 and 30"), "{err}");
    }

    #[test]
    fn bad_choices_are_rejected_with_the_allowed_set() {
        for (name, call) in [
            ("weight_unit", run(70.0, "stone", 30.0, "minutes", "jogging", 0.0, "gross", "summary")),
            ("duration_unit", run(70.0, "kg", 30.0, "days", "jogging", 0.0, "gross", "summary")),
            ("basis", run(70.0, "kg", 30.0, "minutes", "jogging", 0.0, "total", "summary")),
            ("output", run(70.0, "kg", 30.0, "minutes", "jogging", 0.0, "gross", "yaml")),
        ] {
            let err = call.unwrap_err();
            assert!(err.contains(&format!("expected {name} to be one of")), "{name}: {err}");
        }
    }

    #[test]
    fn table_output_is_markdown_with_a_comparison_block() {
        let out = default_run("table");
        assert!(out.starts_with("## Calories burned\n"), "{out}");
        assert!(out.contains("| Energy burned | 129 kcal |"), "{out}");
        assert!(out.contains("| Body weight | 70 kg (154.3 lb) |"), "{out}");
        assert!(out.contains("## Same 30 min at 70 kg"), "{out}");
        assert!(out.contains("| Running, 6 mph / 9.7 km/h (10 min/mi) | 9.8 | 360 |"), "{out}");
    }

    #[test]
    fn json_output_carries_every_field() {
        let out = default_run("json");
        for needle in [
            "\"activity\": \"walking-moderate\"",
            "\"met\": 3.5",
            "\"met_charged\": 3.5",
            "\"basis\": \"gross\"",
            "\"weight_kg\": 70",
            "\"weight_lb\": 154.3",
            "\"duration_minutes\": 30",
            "\"calories\": 129",
            "\"kcal_per_minute\": 4.3",
            "\"kcal_per_hour\": 257",
            "\"met_minutes\": 105",
            "\"sessions_to_500_met_minutes\": 4.8",
            "\"vo2_ml_per_kg_per_min\": 12.3",
            "\"oxygen_litres\": 25.7",
            "\"fat_grams\": 17",
            "\"comparison\": [",
        ] {
            assert!(out.contains(needle), "missing {needle} in {out}");
        }
        assert_eq!(out.matches("\"calories\":").count(), 1 + COMPARISON.len());
    }

    /// Every advertised activity value must price a session — a value in the
    /// dropdown that the core rejects would be a broken option.
    #[test]
    fn every_activity_value_runs() {
        for key in activity_keys() {
            let met = if key == CUSTOM { 6.0 } else { 0.0 };
            for basis in BASES {
                for output in OUTPUTS {
                    run(70.0, "kg", 45.0, "minutes", key, met, basis, output)
                        .unwrap_or_else(|e| panic!("activity={key} basis={basis} output={output}: {e}"));
                }
            }
        }
    }

    /// The bounds the descriptor advertises must be accepted at the boundary,
    /// in every unit the boundary is reachable in.
    #[test]
    fn advertised_bounds_are_accepted_at_the_boundary() {
        run(20.0, "kg", 0.5, "minutes", "jogging", 0.5, "gross", "summary").unwrap();
        run(300.0, "kg", 1440.0, "minutes", "jogging", 30.0, "gross", "summary").unwrap();
        run(660.0, "lb", 24.0, "hours", "jogging", 0.0, "net", "json").unwrap();
        run(44.1, "lb", 0.5, "hours", "sleeping", 0.0, "gross", "table").unwrap();
    }

    /// Duplicate keys in the activity table would shadow an entry in the
    /// dropdown and quietly make it unreachable.
    #[test]
    fn activity_keys_are_unique_and_sane() {
        let keys = activity_keys();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "duplicate activity key");
        assert_eq!(keys.len(), ACTIVITIES.len() + 1);
        for (key, label, met) in activity_table() {
            assert!(!label.is_empty(), "{key} has no label");
            assert!((MET_MIN..=MET_MAX).contains(&met), "{key} MET {met} is outside the advertised range");
        }
    }
}
