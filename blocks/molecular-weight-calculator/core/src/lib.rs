//! molecular-weight-calculator core — pure, deterministic chemistry math shared
//! by the chat skill block and the standalone web page. No wafer/wasm-bindgen
//! deps, no I/O, no network, no ML.
//!
//! Parses a **chemical (molecular) formula** and returns its molar mass, the
//! monoisotopic (exact) mass, the atom/element counts and the per-element mass
//! composition (with mass percent). The parser understands:
//!
//! - element symbols (`H`, `He`, `Na`, `Cl`, …) with optional subscript counts;
//! - nested groups in `()`, `[]` or `{}` with a group multiplier — e.g.
//!   `Ca(OH)2`, `K4[Fe(CN)6]`;
//! - hydrate / dot notation with a leading coefficient — `CuSO4·5H2O`,
//!   `CuSO4.5H2O`, `Na2CO3*10H2O` (the `·`, `.`, `•` and `*` separators all work);
//! - a leading whole-number coefficient on a segment — `2H2O`.
//!
//! Standard atomic weights are IUPAC (abridged) values; the monoisotopic mass
//! uses the exact mass of each element's most abundant natural isotope. SMILES
//! is intentionally **not** supported (a full SMILES engine is out of scope) —
//! a SMILES-looking input errors with a clear message.

use serde::Serialize;

/// Default number of decimal places for the reported masses.
pub const DEFAULT_DECIMALS: u32 = 4;
/// Upper bound on the configurable decimal places.
pub const MAX_DECIMALS: u32 = 10;

/// Per-element row of the mass composition.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementComposition {
    /// Element symbol, e.g. "C".
    pub symbol: String,
    /// Element name, e.g. "Carbon".
    pub name: String,
    /// How many atoms of this element are in the molecule.
    pub count: u64,
    /// Standard atomic weight (g/mol), rounded to the requested decimals.
    pub atomic_weight: f64,
    /// Total mass this element contributes (count × atomic weight), rounded.
    pub mass: f64,
    /// Share of the total molar mass, in percent (2 dp).
    pub percent: f64,
}

/// The full analysis of one formula.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    /// The formula exactly as entered (trimmed).
    pub formula: String,
    /// The same composition re-written in Hill notation (C, then H, then the
    /// remaining elements alphabetically; all alphabetical when there is no C).
    pub hill_formula: String,
    /// Average molar mass in g/mol, rounded to the requested decimals.
    pub molar_mass: f64,
    /// Monoisotopic (exact) mass in Da — sum of each element's most abundant
    /// isotope mass — rounded to the requested decimals.
    pub monoisotopic_mass: f64,
    /// Unit of `molar_mass` — always "g/mol".
    pub mass_unit: String,
    /// Total number of atoms in one molecule.
    pub atom_count: u64,
    /// Number of distinct elements.
    pub element_count: u64,
    /// Per-element mass composition, in first-appearance order.
    pub composition: Vec<ElementComposition>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// (symbol, name, standard atomic weight, monoisotopic mass) for elements 1–92.
/// Standard atomic weights are IUPAC abridged values; for elements with no
/// stable isotope the mass number of a common/long-lived isotope is used (in
/// line with IUPAC bracket notation). The monoisotopic mass is the exact mass
/// of the most abundant natural isotope.
const ELEMENTS: &[(&str, &str, f64, f64)] = &[
    ("H", "Hydrogen", 1.008, 1.0078250319),
    ("He", "Helium", 4.002602, 4.0026032541),
    ("Li", "Lithium", 6.94, 7.0160034366),
    ("Be", "Beryllium", 9.0121831, 9.012183065),
    ("B", "Boron", 10.81, 11.00930536),
    ("C", "Carbon", 12.011, 12.0),
    ("N", "Nitrogen", 14.007, 14.0030740052),
    ("O", "Oxygen", 15.999, 15.9949146221),
    ("F", "Fluorine", 18.998403163, 18.9984031627),
    ("Ne", "Neon", 20.1797, 19.9924401762),
    ("Na", "Sodium", 22.98976928, 22.989769282),
    ("Mg", "Magnesium", 24.305, 23.985041697),
    ("Al", "Aluminium", 26.9815385, 26.98153853),
    ("Si", "Silicon", 28.085, 27.9769265327),
    ("P", "Phosphorus", 30.973761998, 30.97376151),
    ("S", "Sulfur", 32.06, 31.9720711744),
    ("Cl", "Chlorine", 35.45, 34.968852682),
    ("Ar", "Argon", 39.948, 39.9623831237),
    ("K", "Potassium", 39.0983, 38.9637064864),
    ("Ca", "Calcium", 40.078, 39.962590863),
    ("Sc", "Scandium", 44.955908, 44.95590828),
    ("Ti", "Titanium", 47.867, 47.94794198),
    ("V", "Vanadium", 50.9415, 50.94395704),
    ("Cr", "Chromium", 51.9961, 51.94050623),
    ("Mn", "Manganese", 54.938044, 54.93804391),
    ("Fe", "Iron", 55.845, 55.93493633),
    ("Co", "Cobalt", 58.933194, 58.93319429),
    ("Ni", "Nickel", 58.6934, 57.93534241),
    ("Cu", "Copper", 63.546, 62.92959772),
    ("Zn", "Zinc", 65.38, 63.92914201),
    ("Ga", "Gallium", 69.723, 68.9255735),
    ("Ge", "Germanium", 72.630, 73.921177761),
    ("As", "Arsenic", 74.921595, 74.92159457),
    ("Se", "Selenium", 78.971, 79.9165218),
    ("Br", "Bromine", 79.904, 78.9183376),
    ("Kr", "Krypton", 83.798, 83.9114977282),
    ("Rb", "Rubidium", 85.4678, 84.9117897379),
    ("Sr", "Strontium", 87.62, 87.9056125),
    ("Y", "Yttrium", 88.90584, 88.9058403),
    ("Zr", "Zirconium", 91.224, 89.9046977),
    ("Nb", "Niobium", 92.90637, 92.906373),
    ("Mo", "Molybdenum", 95.95, 97.90540482),
    ("Tc", "Technetium", 98.0, 97.9072124),
    ("Ru", "Ruthenium", 101.07, 101.9043441),
    ("Rh", "Rhodium", 102.90550, 102.905498),
    ("Pd", "Palladium", 106.42, 105.9034804),
    ("Ag", "Silver", 107.8682, 106.9050916),
    ("Cd", "Cadmium", 112.414, 113.90336509),
    ("In", "Indium", 114.818, 114.903878776),
    ("Sn", "Tin", 118.710, 119.90220163),
    ("Sb", "Antimony", 121.760, 120.903812),
    ("Te", "Tellurium", 127.60, 129.906222748),
    ("I", "Iodine", 126.90447, 126.9044719),
    ("Xe", "Xenon", 131.293, 131.9041550856),
    ("Cs", "Caesium", 132.90545196, 132.905451961),
    ("Ba", "Barium", 137.327, 137.905247),
    ("La", "Lanthanum", 138.90547, 138.9063563),
    ("Ce", "Cerium", 140.116, 139.9054431),
    ("Pr", "Praseodymium", 140.90766, 140.9076576),
    ("Nd", "Neodymium", 144.242, 141.907729),
    ("Pm", "Promethium", 145.0, 144.9127559),
    ("Sm", "Samarium", 150.36, 151.9197397),
    ("Eu", "Europium", 151.964, 152.921238),
    ("Gd", "Gadolinium", 157.25, 157.9241123),
    ("Tb", "Terbium", 158.92535, 158.9253547),
    ("Dy", "Dysprosium", 162.500, 163.9291819),
    ("Ho", "Holmium", 164.93033, 164.9303288),
    ("Er", "Erbium", 167.259, 165.9302995),
    ("Tm", "Thulium", 168.93422, 168.9342179),
    ("Yb", "Ytterbium", 173.045, 173.9388664),
    ("Lu", "Lutetium", 174.9668, 174.9407752),
    ("Hf", "Hafnium", 178.49, 179.946557),
    ("Ta", "Tantalum", 180.94788, 180.9479958),
    ("W", "Tungsten", 183.84, 183.95093092),
    ("Re", "Rhenium", 186.207, 186.9557501),
    ("Os", "Osmium", 190.23, 191.961477),
    ("Ir", "Iridium", 192.217, 192.9629216),
    ("Pt", "Platinum", 195.084, 194.9647917),
    ("Au", "Gold", 196.966569, 196.96656879),
    ("Hg", "Mercury", 200.592, 201.9706434),
    ("Tl", "Thallium", 204.38, 204.9744278),
    ("Pb", "Lead", 207.2, 207.9766525),
    ("Bi", "Bismuth", 208.98040, 208.9803991),
    ("Po", "Polonium", 209.0, 208.9824308),
    ("At", "Astatine", 210.0, 209.9871479),
    ("Rn", "Radon", 222.0, 222.0175782),
    ("Fr", "Francium", 223.0, 223.019736),
    ("Ra", "Radium", 226.0, 226.0254103),
    ("Ac", "Actinium", 227.0, 227.0277523),
    ("Th", "Thorium", 232.0377, 232.0380558),
    ("Pa", "Protactinium", 231.03588, 231.0358842),
    ("U", "Uranium", 238.02891, 238.0507884),
];

/// Look up an element by symbol, returning (name, atomic weight, monoisotopic).
fn lookup(symbol: &str) -> Option<(&'static str, f64, f64)> {
    ELEMENTS
        .iter()
        .find(|(s, ..)| *s == symbol)
        .map(|(_, name, w, m)| (*name, *w, *m))
}

/// Ordered element→count accumulator (preserves first-appearance order for a
/// stable, human-friendly composition table).
#[derive(Default)]
struct Counts {
    order: Vec<String>,
    map: std::collections::HashMap<String, u64>,
}

impl Counts {
    fn add(&mut self, symbol: &str, n: u64) -> Result<(), String> {
        if !self.map.contains_key(symbol) {
            self.order.push(symbol.to_string());
        }
        let entry = self.map.entry(symbol.to_string()).or_insert(0);
        *entry = entry
            .checked_add(n)
            .ok_or_else(|| "formula is too large (atom count overflow)".to_string())?;
        Ok(())
    }

    /// Fold another set of counts into this one, each multiplied by `mult`.
    fn merge(&mut self, other: &Counts, mult: u64) -> Result<(), String> {
        for symbol in &other.order {
            let n = other.map[symbol]
                .checked_mul(mult)
                .ok_or_else(|| "formula is too large (atom count overflow)".to_string())?;
            self.add(symbol, n)?;
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

fn closing_for(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    }
}

/// Read an element symbol at `pos`: one uppercase letter + at most one lowercase.
fn read_symbol(chars: &[char], pos: &mut usize) -> String {
    let mut s = String::new();
    s.push(chars[*pos]);
    *pos += 1;
    if *pos < chars.len() && chars[*pos].is_ascii_lowercase() {
        s.push(chars[*pos]);
        *pos += 1;
    }
    s
}

/// Read consecutive ASCII digits as a count; `None` when there is no number.
fn read_number(chars: &[char], pos: &mut usize) -> Result<Option<u64>, String> {
    let start = *pos;
    while *pos < chars.len() && chars[*pos].is_ascii_digit() {
        *pos += 1;
    }
    if *pos == start {
        return Ok(None);
    }
    let text: String = chars[start..*pos].iter().collect();
    text.parse::<u64>()
        .map(Some)
        .map_err(|_| format!("count '{text}' is too large"))
}

/// Parse a bracket-balanced sequence of elements/groups. When `close` is set,
/// stop (and consume) at that closing bracket; otherwise parse to the end.
fn parse_sequence(chars: &[char], pos: &mut usize, close: Option<char>) -> Result<Counts, String> {
    let mut counts = Counts::default();
    while *pos < chars.len() {
        let c = chars[*pos];
        if Some(c) == close {
            *pos += 1;
            return Ok(counts);
        }
        match c {
            '(' | '[' | '{' => {
                let want = closing_for(c);
                *pos += 1;
                let inner = parse_sequence(chars, pos, Some(want))?;
                let mult = read_number(chars, pos)?.unwrap_or(1);
                counts.merge(&inner, mult)?;
            }
            ')' | ']' | '}' => {
                return Err(format!("unbalanced '{c}' — check the brackets in the formula"));
            }
            c if c.is_ascii_uppercase() => {
                let symbol = read_symbol(chars, pos);
                let n = read_number(chars, pos)?.unwrap_or(1);
                if lookup(&symbol).is_none() {
                    return Err(format!("unknown element symbol '{symbol}'"));
                }
                counts.add(&symbol, n)?;
            }
            c if c.is_whitespace() => {
                *pos += 1;
            }
            c => {
                let hint = if c.is_ascii_lowercase()
                    || matches!(c, '=' | '#' | '@' | '/' | '\\' | '+' | '-')
                {
                    " — SMILES notation is not supported; enter a molecular formula such as C6H6 for benzene"
                } else {
                    ""
                };
                return Err(format!("unexpected character '{c}' in formula{hint}"));
            }
        }
    }
    if close.is_some() {
        return Err("unbalanced brackets — a group was never closed".to_string());
    }
    Ok(counts)
}

/// Parse a full formula (including hydrate/dot notation) into element counts.
fn parse_formula(formula: &str) -> Result<Counts, String> {
    let mut total = Counts::default();
    for segment in formula.split(|c| matches!(c, '.' | '·' | '•' | '*')) {
        let seg = segment.trim();
        if seg.is_empty() {
            continue;
        }
        let chars: Vec<char> = seg.chars().collect();
        let mut pos = 0usize;
        // A leading whole number multiplies the whole segment (e.g. 5H2O).
        let coeff = read_number(&chars, &mut pos)?.unwrap_or(1);
        let counts = parse_sequence(&chars, &mut pos, None)?;
        if counts.is_empty() {
            return Err(format!("segment '{seg}' has no elements"));
        }
        total.merge(&counts, coeff)?;
    }
    if total.is_empty() {
        return Err("no elements found — enter a formula like H2O, C6H12O6 or Ca(OH)2".to_string());
    }
    Ok(total)
}

fn round_to(x: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (x * factor).round() / factor
}

/// Build the Hill-system formula string from the combined counts.
fn hill_formula(counts: &Counts) -> String {
    let mut ordered: Vec<String> = Vec::new();
    if counts.map.contains_key("C") {
        ordered.push("C".to_string());
        if counts.map.contains_key("H") {
            ordered.push("H".to_string());
        }
        let mut rest: Vec<String> = counts
            .map
            .keys()
            .filter(|s| s.as_str() != "C" && s.as_str() != "H")
            .cloned()
            .collect();
        rest.sort();
        ordered.extend(rest);
    } else {
        let mut all: Vec<String> = counts.map.keys().cloned().collect();
        all.sort();
        ordered.extend(all);
    }
    let mut out = String::new();
    for symbol in ordered {
        let n = counts.map[&symbol];
        out.push_str(&symbol);
        if n != 1 {
            out.push_str(&n.to_string());
        }
    }
    out
}

/// Format a rounded mass for the summary line (no trailing `.0` noise beyond the
/// requested precision — `serde`/`format!` already print the shortest form).
fn fmt_mass(x: f64) -> String {
    format!("{x}")
}

/// Analyze `formula`, rounding the masses to `decimals` (clamped to
/// `MAX_DECIMALS`). Errors on an empty/invalid formula or an unknown element.
pub fn analyze(formula: &str, decimals: u32) -> Result<Analysis, String> {
    let decimals = decimals.min(MAX_DECIMALS);
    let trimmed = formula.trim();
    if trimmed.is_empty() {
        return Err("enter a chemical formula, e.g. H2O, C6H12O6 or Ca(OH)2".to_string());
    }

    let counts = parse_formula(trimmed)?;

    let mut molar = 0.0f64;
    let mut mono = 0.0f64;
    let mut atom_count = 0u64;
    // (symbol, name, weight, count)
    let mut rows: Vec<(String, &'static str, f64, u64)> = Vec::new();
    for symbol in &counts.order {
        let n = counts.map[symbol];
        // parse_sequence already validated the symbol, so lookup succeeds.
        let (name, weight, monoiso) = lookup(symbol).unwrap();
        molar += weight * n as f64;
        mono += monoiso * n as f64;
        atom_count += n;
        rows.push((symbol.clone(), name, weight, n));
    }

    if molar <= 0.0 {
        return Err("could not compute a positive molar mass for that formula".to_string());
    }

    let composition: Vec<ElementComposition> = rows
        .iter()
        .map(|(symbol, name, weight, n)| {
            let mass = weight * *n as f64;
            ElementComposition {
                symbol: symbol.clone(),
                name: (*name).to_string(),
                count: *n,
                atomic_weight: round_to(*weight, decimals),
                mass: round_to(mass, decimals),
                percent: round_to(mass / molar * 100.0, 2),
            }
        })
        .collect();

    let molar_mass = round_to(molar, decimals);
    let monoisotopic_mass = round_to(mono, decimals);
    let hill = hill_formula(&counts);
    let element_count = counts.order.len() as u64;
    let summary = format!(
        "{hill}: molar mass {} g/mol, monoisotopic mass {} Da ({atom_count} atoms, {element_count} element{})",
        fmt_mass(molar_mass),
        fmt_mass(monoisotopic_mass),
        if element_count == 1 { "" } else { "s" },
    );

    Ok(Analysis {
        formula: trimmed.to_string(),
        hill_formula: hill,
        molar_mass,
        monoisotopic_mass,
        mass_unit: "g/mol".to_string(),
        atom_count,
        element_count,
        composition,
        summary,
    })
}

/// Same as [`analyze`] but returns pretty-printed JSON (for the web page / CLI).
pub fn analyze_json(formula: &str, decimals: u32) -> Result<String, String> {
    let res = analyze(formula, decimals)?;
    serde_json::to_string_pretty(&res).map_err(|e| format!("serialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_masses_and_composition() {
        let a = analyze("H2O", DEFAULT_DECIMALS).unwrap();
        assert_eq!(a.molar_mass, 18.015);
        assert_eq!(a.monoisotopic_mass, 18.0106);
        assert_eq!(a.atom_count, 3);
        assert_eq!(a.element_count, 2);
        assert_eq!(a.hill_formula, "H2O");
        assert_eq!(a.mass_unit, "g/mol");
        assert_eq!(a.composition[0].symbol, "H");
        assert_eq!(a.composition[0].count, 2);
        assert_eq!(a.composition[0].percent, 11.19);
        assert_eq!(a.composition[1].symbol, "O");
        assert_eq!(a.composition[1].percent, 88.81);
        // Percentages sum to 100 (2 dp).
        let total: f64 = a.composition.iter().map(|c| c.percent).sum();
        assert!((total - 100.0).abs() < 0.02, "percents summed to {total}");
    }

    #[test]
    fn sodium_chloride() {
        let a = analyze("NaCl", DEFAULT_DECIMALS).unwrap();
        assert_eq!(a.molar_mass, 58.4398);
        assert_eq!(a.atom_count, 2);
        assert_eq!(a.hill_formula, "ClNa");
    }

    #[test]
    fn glucose_hill_and_mass() {
        let a = analyze("C6H12O6", DEFAULT_DECIMALS).unwrap();
        assert_eq!(a.molar_mass, 180.156);
        assert_eq!(a.atom_count, 24);
        assert_eq!(a.element_count, 3);
        assert_eq!(a.hill_formula, "C6H12O6");
    }

    #[test]
    fn caffeine() {
        let a = analyze("C8H10N4O2", DEFAULT_DECIMALS).unwrap();
        assert_eq!(a.molar_mass, 194.194);
        assert_eq!(a.hill_formula, "C8H10N4O2");
    }

    #[test]
    fn parentheses_group_multiplier() {
        // Ca(OH)2 = Ca + 2·(O + H) = 40.078 + 2·17.007 = 74.092
        let a = analyze("Ca(OH)2", DEFAULT_DECIMALS).unwrap();
        assert_eq!(a.molar_mass, 74.092);
        assert_eq!(a.atom_count, 5);
        assert_eq!(a.element_count, 3);
    }

    #[test]
    fn nested_brackets() {
        // K4[Fe(CN)6] = 4 K + Fe + 6 C + 6 N
        let a = analyze("K4[Fe(CN)6]", DEFAULT_DECIMALS).unwrap();
        assert_eq!(a.atom_count, 4 + 1 + 6 + 6);
        assert_eq!(a.element_count, 4);
    }

    #[test]
    fn hydrate_dot_notation() {
        // CuSO4·5H2O: 159.602 + 5·18.015 = 249.677
        let dot = analyze("CuSO4·5H2O", DEFAULT_DECIMALS).unwrap();
        let period = analyze("CuSO4.5H2O", DEFAULT_DECIMALS).unwrap();
        let star = analyze("CuSO4*5H2O", DEFAULT_DECIMALS).unwrap();
        assert_eq!(dot.molar_mass, 249.677);
        assert_eq!(period.molar_mass, 249.677);
        assert_eq!(star.molar_mass, 249.677);
        assert_eq!(dot.atom_count, 21);
        // O is 4 (sulfate) + 5 (water) = 9; H is 10.
        assert_eq!(dot.hill_formula, "CuH10O9S");
    }

    #[test]
    fn leading_coefficient() {
        let one = analyze("H2O", DEFAULT_DECIMALS).unwrap();
        let three = analyze("3H2O", DEFAULT_DECIMALS).unwrap();
        assert_eq!(three.atom_count, one.atom_count * 3);
        assert_eq!(round_to(three.molar_mass, 3), round_to(one.molar_mass * 3.0, 3));
    }

    #[test]
    fn decimals_are_configurable() {
        let a = analyze("H2O", 1).unwrap();
        assert_eq!(a.molar_mass, 18.0);
        let b = analyze("H2O", 6).unwrap();
        assert_eq!(b.monoisotopic_mass, 18.010565);
    }

    #[test]
    fn json_shape() {
        let s = analyze_json("H2O", DEFAULT_DECIMALS).unwrap();
        assert!(s.contains("\"molar_mass\": 18.015"));
        assert!(s.contains("\"composition\""));
    }

    #[test]
    fn empty_is_error() {
        assert!(analyze("   ", DEFAULT_DECIMALS).is_err());
    }

    #[test]
    fn unknown_element_is_error() {
        let e = analyze("Xz2", DEFAULT_DECIMALS).unwrap_err();
        assert!(e.contains("unknown element"), "got: {e}");
    }

    #[test]
    fn unbalanced_brackets_is_error() {
        assert!(analyze("Ca(OH2", DEFAULT_DECIMALS).is_err());
        assert!(analyze("CaOH)2", DEFAULT_DECIMALS).is_err());
    }

    #[test]
    fn smiles_is_rejected_with_hint() {
        let e = analyze("c1ccccc1", DEFAULT_DECIMALS).unwrap_err();
        assert!(e.contains("SMILES"), "got: {e}");
    }
}
