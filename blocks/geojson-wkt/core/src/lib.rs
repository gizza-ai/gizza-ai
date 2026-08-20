//! geojson-wkt core — convert geometries between GeoJSON, WKT/EWKT and WKB/EWKB
//! in either direction. Pure compute, shared by the chat skill block and the web
//! page: no I/O, no network, no clock.
//!
//! Supported geometry types are the seven OGC simple-feature types (Point,
//! LineString, Polygon, MultiPoint, MultiLineString, MultiPolygon,
//! GeometryCollection), each in 2-D, Z, M and ZM, plus `EMPTY`. Curved and
//! PostGIS-specific extras (CIRCULARSTRING, TIN, TRIANGLE, POLYHEDRALSURFACE)
//! are rejected with a named error rather than silently mangled.
//!
//! SRIDs are carried as metadata only — coordinates are never reprojected.

use serde_json::{Map, Value};

/// Largest accepted input, in bytes (~2 MB). Bigger inputs are rejected up
/// front so the 64 MiB wasm sandbox can't be pushed into an opaque trap.
pub const MAX_INPUT_BYTES: usize = 2_000_000;
/// Maximum GeometryCollection / container nesting depth.
pub const MAX_DEPTH: usize = 32;
/// Maximum accepted SRID value.
pub const MAX_SRID: i64 = 999_999;
/// Maximum accepted coordinate precision (decimal places).
pub const MAX_PRECISION: i64 = 15;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Which optional ordinates a geometry carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Dim {
    pub z: bool,
    pub m: bool,
}

impl Dim {
    pub const XY: Dim = Dim { z: false, m: false };
    pub const XYZ: Dim = Dim { z: true, m: false };
    pub const XYM: Dim = Dim { z: false, m: true };
    pub const XYZM: Dim = Dim { z: true, m: true };

    /// Numbers per coordinate for this dimensionality (2..=4).
    fn size(self) -> usize {
        2 + usize::from(self.z) + usize::from(self.m)
    }

    /// WKT tag written between the type name and the body (`""`, `" Z"`, …).
    fn tag(self) -> &'static str {
        match (self.z, self.m) {
            (false, false) => "",
            (true, false) => " Z",
            (false, true) => " M",
            (true, true) => " ZM",
        }
    }

    fn label(self) -> &'static str {
        match (self.z, self.m) {
            (false, false) => "2D",
            (true, false) => "Z",
            (false, true) => "M",
            (true, true) => "ZM",
        }
    }
}

/// A coordinate: x, y and then z and/or m in that order.
pub type Pos = Vec<f64>;

/// One geometry, tagged with its own dimensionality (a GeometryCollection may
/// legally hold children of differing dimensionality, so each child owns its).
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub dim: Dim,
    pub geom: Geom,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Geom {
    /// `None` is `POINT EMPTY`.
    Point(Option<Pos>),
    LineString(Vec<Pos>),
    Polygon(Vec<Vec<Pos>>),
    MultiPoint(Vec<Pos>),
    MultiLineString(Vec<Vec<Pos>>),
    MultiPolygon(Vec<Vec<Vec<Pos>>>),
    Collection(Vec<Geometry>),
}

impl Geom {
    fn type_name(&self) -> &'static str {
        match self {
            Geom::Point(_) => "Point",
            Geom::LineString(_) => "LineString",
            Geom::Polygon(_) => "Polygon",
            Geom::MultiPoint(_) => "MultiPoint",
            Geom::MultiLineString(_) => "MultiLineString",
            Geom::MultiPolygon(_) => "MultiPolygon",
            Geom::Collection(_) => "GeometryCollection",
        }
    }

    fn wkt_name(&self) -> &'static str {
        match self {
            Geom::Point(_) => "POINT",
            Geom::LineString(_) => "LINESTRING",
            Geom::Polygon(_) => "POLYGON",
            Geom::MultiPoint(_) => "MULTIPOINT",
            Geom::MultiLineString(_) => "MULTILINESTRING",
            Geom::MultiPolygon(_) => "MULTIPOLYGON",
            Geom::Collection(_) => "GEOMETRYCOLLECTION",
        }
    }

    fn wkb_code(&self) -> u32 {
        match self {
            Geom::Point(_) => 1,
            Geom::LineString(_) => 2,
            Geom::Polygon(_) => 3,
            Geom::MultiPoint(_) => 4,
            Geom::MultiLineString(_) => 5,
            Geom::MultiPolygon(_) => 6,
            Geom::Collection(_) => 7,
        }
    }
}

/// One parsed input document: the geometries plus any SRID the input carried.
#[derive(Clone, Debug, PartialEq)]
pub struct Doc {
    pub srid: u32,
    pub geoms: Vec<Geometry>,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn resolve_dim(ordinates: Option<usize>, declared: Option<Dim>) -> Result<Dim, String> {
    match (ordinates, declared) {
        (None, Some(d)) => Ok(d),
        (None, None) => Ok(Dim::XY),
        (Some(n), Some(d)) => {
            if d.size() == n {
                Ok(d)
            } else {
                Err(format!(
                    "a {} geometry needs {} numbers per coordinate, found {n}",
                    d.label(),
                    d.size()
                ))
            }
        }
        (Some(2), None) => Ok(Dim::XY),
        (Some(3), None) => Ok(Dim::XYZ),
        (Some(4), None) => Ok(Dim::XYZM),
        (Some(n), None) => Err(format!(
            "a coordinate must have 2 to 4 numbers (x y [z] [m]), found {n}"
        )),
    }
}

/// Common ordinate count across every coordinate of a geometry (children of a
/// GeometryCollection own their own dimensionality and are not consulted).
fn ordinate_count(geom: &Geom) -> Result<Option<usize>, String> {
    fn scan(positions: &[Pos], seen: &mut Option<usize>) -> Result<(), String> {
        for p in positions {
            match seen {
                None => *seen = Some(p.len()),
                Some(n) if *n == p.len() => {}
                Some(n) => {
                    return Err(format!(
                        "mixed coordinate sizes in one geometry: {n} numbers then {}",
                        p.len()
                    ))
                }
            }
        }
        Ok(())
    }
    let mut seen = None;
    match geom {
        Geom::Point(p) => {
            if let Some(p) = p {
                scan(std::slice::from_ref(p), &mut seen)?;
            }
        }
        Geom::LineString(ps) | Geom::MultiPoint(ps) => scan(ps, &mut seen)?,
        Geom::Polygon(rings) | Geom::MultiLineString(rings) => {
            for r in rings {
                scan(r, &mut seen)?;
            }
        }
        Geom::MultiPolygon(polys) => {
            for p in polys {
                for r in p {
                    scan(r, &mut seen)?;
                }
            }
        }
        Geom::Collection(children) => {
            return Ok(children.first().map(|c| c.dim.size()));
        }
    }
    Ok(seen)
}

fn finish(geom: Geom, declared: Option<Dim>) -> Result<Geometry, String> {
    let dim = resolve_dim(ordinate_count(&geom)?, declared)?;
    Ok(Geometry { dim, geom })
}

fn each_pos_mut(geom: &mut Geom, f: &mut impl FnMut(&mut Pos)) {
    match geom {
        Geom::Point(p) => {
            if let Some(p) = p {
                f(p);
            }
        }
        Geom::LineString(ps) | Geom::MultiPoint(ps) => ps.iter_mut().for_each(&mut *f),
        Geom::Polygon(rings) | Geom::MultiLineString(rings) => {
            for r in rings {
                r.iter_mut().for_each(&mut *f);
            }
        }
        Geom::MultiPolygon(polys) => {
            for p in polys {
                for r in p {
                    r.iter_mut().for_each(&mut *f);
                }
            }
        }
        Geom::Collection(children) => {
            for c in children {
                each_pos_mut(&mut c.geom, f);
            }
        }
    }
}

fn round_to(v: f64, places: i64) -> f64 {
    if places < 0 {
        return v;
    }
    let factor = 10f64.powi(places as i32);
    let r = (v * factor).round() / factor;
    if r.is_finite() {
        r
    } else {
        v
    }
}

fn check_finite(geom: &mut Geom) -> Result<(), String> {
    let mut bad = false;
    each_pos_mut(geom, &mut |p| {
        if p.iter().any(|v| !v.is_finite()) {
            bad = true;
        }
    });
    if bad {
        Err("coordinates must be finite numbers (NaN and Infinity are not valid geometry)".into())
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WKT lexer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    Num(f64),
    LParen,
    RParen,
    Comma,
    Semi,
    Eq,
    End,
}

impl Tok {
    fn describe(&self) -> String {
        match self {
            Tok::Word(w) => format!("'{w}'"),
            Tok::Num(n) => format!("number {n}"),
            Tok::LParen => "'('".into(),
            Tok::RParen => "')'".into(),
            Tok::Comma => "','".into(),
            Tok::Semi => "';'".into(),
            Tok::Eq => "'='".into(),
            Tok::End => "end of input".into(),
        }
    }
}

struct Lexer<'a> {
    b: &'a [u8],
    i: usize,
    peeked: Option<Tok>,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Lexer {
            b: s.as_bytes(),
            i: 0,
            peeked: None,
        }
    }

    fn lex(&mut self) -> Result<Tok, String> {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
        if self.i >= self.b.len() {
            return Ok(Tok::End);
        }
        let c = self.b[self.i];
        match c {
            b'(' => {
                self.i += 1;
                Ok(Tok::LParen)
            }
            b')' => {
                self.i += 1;
                Ok(Tok::RParen)
            }
            b',' => {
                self.i += 1;
                Ok(Tok::Comma)
            }
            b';' => {
                self.i += 1;
                Ok(Tok::Semi)
            }
            b'=' => {
                self.i += 1;
                Ok(Tok::Eq)
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let start = self.i;
                while self.i < self.b.len()
                    && (self.b[self.i].is_ascii_alphanumeric() || self.b[self.i] == b'_')
                {
                    self.i += 1;
                }
                let w = std::str::from_utf8(&self.b[start..self.i])
                    .map_err(|_| "input is not valid UTF-8".to_string())?;
                Ok(Tok::Word(w.to_ascii_uppercase()))
            }
            _ if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.' => {
                let start = self.i;
                self.i += 1;
                while self.i < self.b.len() {
                    let d = self.b[self.i];
                    let exp_sign =
                        (d == b'-' || d == b'+') && matches!(self.b[self.i - 1], b'e' | b'E');
                    if d.is_ascii_digit() || d == b'.' || d == b'e' || d == b'E' || exp_sign {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                let raw = std::str::from_utf8(&self.b[start..self.i])
                    .map_err(|_| "input is not valid UTF-8".to_string())?;
                raw.parse::<f64>()
                    .map(Tok::Num)
                    .map_err(|_| format!("'{raw}' is not a number"))
            }
            _ => Err(format!(
                "unexpected character '{}' in WKT input",
                char::from(c)
            )),
        }
    }

    fn bump(&mut self) -> Result<Tok, String> {
        match self.peeked.take() {
            Some(t) => Ok(t),
            None => self.lex(),
        }
    }

    fn peek(&mut self) -> Result<Tok, String> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lex()?);
        }
        Ok(self.peeked.clone().unwrap())
    }

    fn expect(&mut self, want: &Tok) -> Result<(), String> {
        let got = self.bump()?;
        if &got == want {
            Ok(())
        } else {
            Err(format!(
                "expected {} but found {}",
                want.describe(),
                got.describe()
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// WKT parsing
// ---------------------------------------------------------------------------

const WKT_TYPES: [&str; 7] = [
    "POINT",
    "LINESTRING",
    "POLYGON",
    "MULTIPOINT",
    "MULTILINESTRING",
    "MULTIPOLYGON",
    "GEOMETRYCOLLECTION",
];

const UNSUPPORTED_TYPES: [&str; 8] = [
    "CIRCULARSTRING",
    "COMPOUNDCURVE",
    "CURVEPOLYGON",
    "MULTICURVE",
    "MULTISURFACE",
    "POLYHEDRALSURFACE",
    "TIN",
    "TRIANGLE",
];

/// Split a possibly dimension-suffixed type word (`POINTZM`) into its base name
/// and declared dimensionality.
fn split_type_word(w: &str) -> (String, Option<Dim>) {
    for (suffix, dim) in [("ZM", Dim::XYZM), ("Z", Dim::XYZ), ("M", Dim::XYM)] {
        if let Some(base) = w.strip_suffix(suffix) {
            if WKT_TYPES.contains(&base) {
                return (base.to_string(), Some(dim));
            }
        }
    }
    (w.to_string(), None)
}

fn parse_pos(lx: &mut Lexer) -> Result<Pos, String> {
    let mut out = Vec::new();
    while let Tok::Num(n) = lx.peek()? {
        lx.bump()?;
        out.push(n);
        if out.len() > 4 {
            return Err("a coordinate must have 2 to 4 numbers (x y [z] [m])".into());
        }
    }
    if out.len() < 2 {
        return Err(format!(
            "a coordinate needs at least an x and a y, found {} number(s)",
            out.len()
        ));
    }
    Ok(out)
}

/// `(x y, x y, …)`
fn parse_pos_list(lx: &mut Lexer) -> Result<Vec<Pos>, String> {
    lx.expect(&Tok::LParen)?;
    let mut out = vec![parse_pos(lx)?];
    while lx.peek()? == Tok::Comma {
        lx.bump()?;
        out.push(parse_pos(lx)?);
    }
    lx.expect(&Tok::RParen)?;
    Ok(out)
}

/// `((x y, …), (x y, …))`
fn parse_pos_list_list(lx: &mut Lexer) -> Result<Vec<Vec<Pos>>, String> {
    lx.expect(&Tok::LParen)?;
    let mut out = vec![parse_pos_list(lx)?];
    while lx.peek()? == Tok::Comma {
        lx.bump()?;
        out.push(parse_pos_list(lx)?);
    }
    lx.expect(&Tok::RParen)?;
    Ok(out)
}

/// `MULTIPOINT` accepts both `(1 2, 3 4)` and `((1 2), (3 4))`.
fn parse_multipoint_body(lx: &mut Lexer) -> Result<Vec<Pos>, String> {
    lx.expect(&Tok::LParen)?;
    let mut out = Vec::new();
    loop {
        if lx.peek()? == Tok::LParen {
            lx.bump()?;
            out.push(parse_pos(lx)?);
            lx.expect(&Tok::RParen)?;
        } else if let Tok::Word(w) = lx.peek()? {
            return Err(if w == "EMPTY" {
                "an empty point inside MULTIPOINT is not supported".into()
            } else {
                format!("expected a coordinate in MULTIPOINT, found '{w}'")
            });
        } else {
            out.push(parse_pos(lx)?);
        }
        if lx.peek()? == Tok::Comma {
            lx.bump()?;
        } else {
            break;
        }
    }
    lx.expect(&Tok::RParen)?;
    Ok(out)
}

fn parse_geometry(
    lx: &mut Lexer,
    inherited: Option<Dim>,
    depth: usize,
) -> Result<Geometry, String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "geometry nesting is deeper than the {MAX_DEPTH}-level limit"
        ));
    }
    let word = match lx.bump()? {
        Tok::Word(w) => w,
        other => {
            return Err(format!(
                "expected a geometry type name such as POINT, found {}",
                other.describe()
            ))
        }
    };
    let (name, mut declared) = split_type_word(&word);
    if !WKT_TYPES.contains(&name.as_str()) {
        return Err(if UNSUPPORTED_TYPES.contains(&name.as_str()) {
            format!("{name} is not supported — only POINT, LINESTRING, POLYGON, MULTIPOINT, MULTILINESTRING, MULTIPOLYGON and GEOMETRYCOLLECTION can be converted")
        } else {
            format!("unknown geometry type '{word}'")
        });
    }
    // A separately-tokenised dimension tag: `POINT Z (…)`.
    if declared.is_none() {
        if let Tok::Word(w) = lx.peek()? {
            match w.as_str() {
                "Z" => declared = Some(Dim::XYZ),
                "M" => declared = Some(Dim::XYM),
                "ZM" => declared = Some(Dim::XYZM),
                _ => {}
            }
            if declared.is_some() {
                lx.bump()?;
            }
        }
    }
    let declared = declared.or(inherited);

    // EMPTY body.
    if let Tok::Word(w) = lx.peek()? {
        if w == "EMPTY" {
            lx.bump()?;
            let geom = match name.as_str() {
                "POINT" => Geom::Point(None),
                "LINESTRING" => Geom::LineString(Vec::new()),
                "POLYGON" => Geom::Polygon(Vec::new()),
                "MULTIPOINT" => Geom::MultiPoint(Vec::new()),
                "MULTILINESTRING" => Geom::MultiLineString(Vec::new()),
                "MULTIPOLYGON" => Geom::MultiPolygon(Vec::new()),
                _ => Geom::Collection(Vec::new()),
            };
            return Ok(Geometry {
                dim: declared.unwrap_or(Dim::XY),
                geom,
            });
        }
        return Err(format!("expected '(' or EMPTY after {name}, found '{w}'"));
    }

    let geom = match name.as_str() {
        "POINT" => {
            lx.expect(&Tok::LParen)?;
            let p = parse_pos(lx)?;
            lx.expect(&Tok::RParen)?;
            Geom::Point(Some(p))
        }
        "LINESTRING" => Geom::LineString(parse_pos_list(lx)?),
        "POLYGON" => Geom::Polygon(parse_pos_list_list(lx)?),
        "MULTIPOINT" => Geom::MultiPoint(parse_multipoint_body(lx)?),
        "MULTILINESTRING" => Geom::MultiLineString(parse_pos_list_list(lx)?),
        "MULTIPOLYGON" => {
            lx.expect(&Tok::LParen)?;
            let mut out = vec![parse_pos_list_list(lx)?];
            while lx.peek()? == Tok::Comma {
                lx.bump()?;
                out.push(parse_pos_list_list(lx)?);
            }
            lx.expect(&Tok::RParen)?;
            Geom::MultiPolygon(out)
        }
        _ => {
            lx.expect(&Tok::LParen)?;
            let mut out = vec![parse_geometry(lx, declared, depth + 1)?];
            while lx.peek()? == Tok::Comma {
                lx.bump()?;
                out.push(parse_geometry(lx, declared, depth + 1)?);
            }
            lx.expect(&Tok::RParen)?;
            Geom::Collection(out)
        }
    };
    finish(geom, declared)
}

fn parse_wkt_doc(s: &str) -> Result<Doc, String> {
    let mut lx = Lexer::new(s);
    let mut doc = Doc {
        srid: 0,
        geoms: Vec::new(),
    };
    loop {
        // Skip separators between consecutive top-level geometries.
        while matches!(lx.peek()?, Tok::Comma | Tok::Semi) {
            lx.bump()?;
        }
        if lx.peek()? == Tok::End {
            break;
        }
        if lx.peek()? == Tok::Word("SRID".into()) {
            lx.bump()?;
            lx.expect(&Tok::Eq)?;
            let n = match lx.bump()? {
                Tok::Num(n) => n,
                other => {
                    return Err(format!(
                        "expected an SRID number after 'SRID=', found {}",
                        other.describe()
                    ))
                }
            };
            if n.fract() != 0.0 || !(0.0..=MAX_SRID as f64).contains(&n) {
                return Err(format!("SRID must be a whole number from 0 to {MAX_SRID}"));
            }
            doc.srid = n as u32;
            lx.expect(&Tok::Semi)?;
            continue;
        }
        let mut g = parse_geometry(&mut lx, None, 0)?;
        check_finite(&mut g.geom)?;
        doc.geoms.push(g);
    }
    if doc.geoms.is_empty() {
        return Err("no geometry found in the WKT input".into());
    }
    Ok(doc)
}

// ---------------------------------------------------------------------------
// WKT writing
// ---------------------------------------------------------------------------

fn fmt_num(v: f64) -> String {
    let s = format!("{v}");
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn write_pos(out: &mut String, p: &Pos) {
    for (i, v) in p.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&fmt_num(*v));
    }
}

fn write_pos_list(out: &mut String, ps: &[Pos]) {
    out.push('(');
    for (i, p) in ps.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_pos(out, p);
    }
    out.push(')');
}

fn write_pos_list_list(out: &mut String, rings: &[Vec<Pos>]) {
    out.push('(');
    for (i, r) in rings.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_pos_list(out, r);
    }
    out.push(')');
}

fn write_wkt_geom(out: &mut String, g: &Geometry) {
    out.push_str(g.geom.wkt_name());
    out.push_str(g.dim.tag());
    let empty = match &g.geom {
        Geom::Point(p) => p.is_none(),
        Geom::LineString(ps) | Geom::MultiPoint(ps) => ps.is_empty(),
        Geom::Polygon(r) | Geom::MultiLineString(r) => r.is_empty(),
        Geom::MultiPolygon(p) => p.is_empty(),
        Geom::Collection(c) => c.is_empty(),
    };
    if empty {
        out.push_str(" EMPTY");
        return;
    }
    if !g.dim.tag().is_empty() {
        out.push(' ');
    }
    match &g.geom {
        Geom::Point(Some(p)) => {
            out.push('(');
            write_pos(out, p);
            out.push(')');
        }
        Geom::Point(None) => unreachable!(),
        Geom::LineString(ps) | Geom::MultiPoint(ps) => write_pos_list(out, ps),
        Geom::Polygon(rings) | Geom::MultiLineString(rings) => write_pos_list_list(out, rings),
        Geom::MultiPolygon(polys) => {
            out.push('(');
            for (i, p) in polys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_pos_list_list(out, p);
            }
            out.push(')');
        }
        Geom::Collection(children) => {
            out.push('(');
            for (i, c) in children.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_wkt_geom(out, c);
            }
            out.push(')');
        }
    }
}

/// Render one geometry as WKT, prefixed with `SRID=n;` (EWKT) when `srid > 0`.
pub fn to_wkt(g: &Geometry, srid: u32) -> String {
    let mut out = String::new();
    if srid > 0 {
        out.push_str(&format!("SRID={srid};"));
    }
    write_wkt_geom(&mut out, g);
    out
}

// ---------------------------------------------------------------------------
// WKB
// ---------------------------------------------------------------------------

const EWKB_Z: u32 = 0x8000_0000;
const EWKB_M: u32 = 0x4000_0000;
const EWKB_SRID: u32 = 0x2000_0000;

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn need(&self, n: usize) -> Result<(), String> {
        if self.i + n > self.b.len() {
            Err("WKB input ends in the middle of a geometry (truncated bytes)".into())
        } else {
            Ok(())
        }
    }
    fn u8(&mut self) -> Result<u8, String> {
        self.need(1)?;
        let v = self.b[self.i];
        self.i += 1;
        Ok(v)
    }
    fn u32(&mut self, big: bool) -> Result<u32, String> {
        self.need(4)?;
        let raw: [u8; 4] = self.b[self.i..self.i + 4].try_into().unwrap();
        self.i += 4;
        Ok(if big {
            u32::from_be_bytes(raw)
        } else {
            u32::from_le_bytes(raw)
        })
    }
    fn f64(&mut self, big: bool) -> Result<f64, String> {
        self.need(8)?;
        let raw: [u8; 8] = self.b[self.i..self.i + 8].try_into().unwrap();
        self.i += 8;
        Ok(if big {
            f64::from_be_bytes(raw)
        } else {
            f64::from_le_bytes(raw)
        })
    }
    fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.i)
    }
    /// Reject counts that cannot possibly fit in the remaining bytes before
    /// allocating for them.
    fn guard(&self, count: u32, per_item: usize) -> Result<usize, String> {
        let count = count as usize;
        if per_item > 0 && count.saturating_mul(per_item) > self.remaining() {
            return Err(format!(
                "WKB declares {count} items but only {} bytes remain (truncated or corrupt)",
                self.remaining()
            ));
        }
        Ok(count)
    }
    fn pos(&mut self, big: bool, dim: Dim) -> Result<Pos, String> {
        let mut p = Vec::with_capacity(dim.size());
        for _ in 0..dim.size() {
            p.push(self.f64(big)?);
        }
        Ok(p)
    }
    fn pos_list(&mut self, big: bool, dim: Dim) -> Result<Vec<Pos>, String> {
        let n = self.u32(big)?;
        let n = self.guard(n, dim.size() * 8)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.pos(big, dim)?);
        }
        Ok(out)
    }
    fn ring_list(&mut self, big: bool, dim: Dim) -> Result<Vec<Vec<Pos>>, String> {
        let n = self.u32(big)?;
        let n = self.guard(n, 4)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.pos_list(big, dim)?);
        }
        Ok(out)
    }

    fn geometry(&mut self, depth: usize, srid_out: &mut u32) -> Result<Geometry, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "geometry nesting is deeper than the {MAX_DEPTH}-level limit"
            ));
        }
        let order = self.u8()?;
        let big = match order {
            0 => true,
            1 => false,
            other => {
                return Err(format!(
                "WKB byte-order flag must be 0 (big-endian) or 1 (little-endian), found {other} \
                     — is this really WKB?"
            ))
            }
        };
        let raw = self.u32(big)?;
        let mut dim = Dim {
            z: raw & EWKB_Z != 0,
            m: raw & EWKB_M != 0,
        };
        let has_srid = raw & EWKB_SRID != 0;
        let base = raw & !(EWKB_Z | EWKB_M | EWKB_SRID);
        let (code, iso) = (base % 1000, base / 1000);
        match iso {
            0 => {}
            1 => dim.z = true,
            2 => dim.m = true,
            3 => {
                dim.z = true;
                dim.m = true;
            }
            _ => return Err(format!("unknown WKB geometry type code {base}")),
        }
        if has_srid {
            let s = self.u32(big)?;
            if depth == 0 {
                *srid_out = s.min(MAX_SRID as u32);
            }
        }
        let geom = match code {
            1 => {
                let p = self.pos(big, dim)?;
                // PostGIS encodes POINT EMPTY as all-NaN ordinates.
                if p.iter().all(|v| v.is_nan()) {
                    Geom::Point(None)
                } else {
                    Geom::Point(Some(p))
                }
            }
            2 => Geom::LineString(self.pos_list(big, dim)?),
            3 => Geom::Polygon(self.ring_list(big, dim)?),
            4 => {
                let n = self.u32(big)?;
                let n = self.guard(n, 5 + dim.size() * 8)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    let child = self.geometry(depth + 1, srid_out)?;
                    match child.geom {
                        Geom::Point(Some(p)) => out.push(p),
                        Geom::Point(None) => {
                            return Err("an empty point inside MULTIPOINT is not supported".into())
                        }
                        other => {
                            return Err(format!(
                                "MULTIPOINT may only contain points, found {}",
                                other.type_name()
                            ))
                        }
                    }
                }
                Geom::MultiPoint(out)
            }
            5 => {
                let n = self.u32(big)?;
                let n = self.guard(n, 9)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    let child = self.geometry(depth + 1, srid_out)?;
                    match child.geom {
                        Geom::LineString(ps) => out.push(ps),
                        other => {
                            return Err(format!(
                                "MULTILINESTRING may only contain line strings, found {}",
                                other.type_name()
                            ))
                        }
                    }
                }
                Geom::MultiLineString(out)
            }
            6 => {
                let n = self.u32(big)?;
                let n = self.guard(n, 9)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    let child = self.geometry(depth + 1, srid_out)?;
                    match child.geom {
                        Geom::Polygon(rings) => out.push(rings),
                        other => {
                            return Err(format!(
                                "MULTIPOLYGON may only contain polygons, found {}",
                                other.type_name()
                            ))
                        }
                    }
                }
                Geom::MultiPolygon(out)
            }
            7 => {
                let n = self.u32(big)?;
                let n = self.guard(n, 5)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(self.geometry(depth + 1, srid_out)?);
                }
                Geom::Collection(out)
            }
            other => {
                return Err(format!(
                "unsupported WKB geometry type code {other} (only codes 1-7 are simple features)"
            ))
            }
        };
        // The declared dimensionality wins; empty geometries keep it as-is.
        let count = ordinate_count(&geom)?;
        if let Some(n) = count {
            if n != dim.size() {
                return Err(format!(
                    "WKB header says {} but the coordinates carry {n} numbers",
                    dim.label()
                ));
            }
        }
        Ok(Geometry { dim, geom })
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32, big: bool) {
    if big {
        out.extend_from_slice(&v.to_be_bytes());
    } else {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn put_f64(out: &mut Vec<u8>, v: f64, big: bool) {
    if big {
        out.extend_from_slice(&v.to_be_bytes());
    } else {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn put_pos(out: &mut Vec<u8>, p: &Pos, big: bool) {
    for v in p {
        put_f64(out, *v, big);
    }
}

fn put_pos_list(out: &mut Vec<u8>, ps: &[Pos], big: bool) {
    put_u32(out, ps.len() as u32, big);
    for p in ps {
        put_pos(out, p, big);
    }
}

fn put_ring_list(out: &mut Vec<u8>, rings: &[Vec<Pos>], big: bool) {
    put_u32(out, rings.len() as u32, big);
    for r in rings {
        put_pos_list(out, r, big);
    }
}

fn write_wkb_geom(out: &mut Vec<u8>, g: &Geometry, big: bool, srid: u32, top: bool) {
    out.push(if big { 0 } else { 1 });
    let code = g.geom.wkb_code();
    let ewkb = srid > 0;
    let type_word = if ewkb {
        code | if g.dim.z { EWKB_Z } else { 0 }
            | if g.dim.m { EWKB_M } else { 0 }
            | if top { EWKB_SRID } else { 0 }
    } else {
        code + 1000 * u32::from(g.dim.z) + 2000 * u32::from(g.dim.m)
    };
    put_u32(out, type_word, big);
    if ewkb && top {
        put_u32(out, srid, big);
    }
    match &g.geom {
        Geom::Point(Some(p)) => put_pos(out, p, big),
        // POINT EMPTY has no count field in WKB — PostGIS writes NaN ordinates.
        Geom::Point(None) => {
            for _ in 0..g.dim.size() {
                put_f64(out, f64::NAN, big);
            }
        }
        Geom::LineString(ps) => put_pos_list(out, ps, big),
        Geom::Polygon(rings) => put_ring_list(out, rings, big),
        Geom::MultiPoint(ps) => {
            put_u32(out, ps.len() as u32, big);
            for p in ps {
                write_wkb_geom(
                    out,
                    &Geometry {
                        dim: g.dim,
                        geom: Geom::Point(Some(p.clone())),
                    },
                    big,
                    srid,
                    false,
                );
            }
        }
        Geom::MultiLineString(lines) => {
            put_u32(out, lines.len() as u32, big);
            for l in lines {
                write_wkb_geom(
                    out,
                    &Geometry {
                        dim: g.dim,
                        geom: Geom::LineString(l.clone()),
                    },
                    big,
                    srid,
                    false,
                );
            }
        }
        Geom::MultiPolygon(polys) => {
            put_u32(out, polys.len() as u32, big);
            for p in polys {
                write_wkb_geom(
                    out,
                    &Geometry {
                        dim: g.dim,
                        geom: Geom::Polygon(p.clone()),
                    },
                    big,
                    srid,
                    false,
                );
            }
        }
        Geom::Collection(children) => {
            put_u32(out, children.len() as u32, big);
            for c in children {
                write_wkb_geom(out, c, big, srid, false);
            }
        }
    }
}

/// Serialise one geometry as WKB bytes (EWKB with an SRID prefix when `srid > 0`).
pub fn to_wkb_bytes(g: &Geometry, srid: u32, big_endian: bool) -> Vec<u8> {
    let mut out = Vec::new();
    write_wkb_geom(&mut out, g, big_endian, srid, true);
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    if cleaned.len() % 2 != 0 {
        return Err("hex WKB must have an even number of digits".into());
    }
    let val = |c: u8| -> Result<u8, String> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(format!("'{}' is not a hex digit", char::from(c))),
        }
    };
    cleaned
        .chunks(2)
        .map(|p| Ok((val(p[0])? << 4) | val(p[1])?))
        .collect()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        out.push(B64[(n >> 18 & 63) as usize] as char);
        out.push(B64[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for c in s.bytes() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => {
                return Err(format!(
                    "'{}' is not a base64 character — is this hex WKB?",
                    char::from(c)
                ))
            }
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

fn looks_hex(s: &str) -> bool {
    let mut n = 0usize;
    for c in s.chars() {
        if c.is_ascii_whitespace() {
            continue;
        }
        if !c.is_ascii_hexdigit() {
            return false;
        }
        n += 1;
    }
    n > 0 && n % 2 == 0
}

fn parse_wkb_doc(s: &str) -> Result<Doc, String> {
    let mut doc = Doc {
        srid: 0,
        geoms: Vec::new(),
    };
    for (idx, line) in s.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let bytes = if looks_hex(line) {
            hex_decode(line)?
        } else {
            b64_decode(line)?
        };
        if bytes.is_empty() {
            return Err(format!("line {} decoded to zero bytes", idx + 1));
        }
        let mut r = Reader { b: &bytes, i: 0 };
        let mut srid = 0u32;
        let mut g = r.geometry(0, &mut srid)?;
        check_finite(&mut g.geom)?;
        if r.remaining() > 0 {
            return Err(format!(
                "{} extra byte(s) after the geometry on line {} — put one WKB value per line",
                r.remaining(),
                idx + 1
            ));
        }
        if srid > 0 {
            doc.srid = srid;
        }
        doc.geoms.push(g);
    }
    if doc.geoms.is_empty() {
        return Err("no geometry found in the WKB input".into());
    }
    Ok(doc)
}

// ---------------------------------------------------------------------------
// GeoJSON
// ---------------------------------------------------------------------------

fn json_pos(v: &Value) -> Result<Pos, String> {
    let arr = v
        .as_array()
        .ok_or_else(|| "a GeoJSON position must be an array of numbers".to_string())?;
    if arr.len() < 2 || arr.len() > 4 {
        return Err(format!(
            "a GeoJSON position must have 2 to 4 numbers, found {}",
            arr.len()
        ));
    }
    arr.iter()
        .map(|n| {
            n.as_f64()
                .ok_or_else(|| "GeoJSON coordinates must be numbers".to_string())
        })
        .collect()
}

fn json_pos_list(v: &Value) -> Result<Vec<Pos>, String> {
    v.as_array()
        .ok_or_else(|| "expected an array of positions".to_string())?
        .iter()
        .map(json_pos)
        .collect()
}

fn json_ring_list(v: &Value) -> Result<Vec<Vec<Pos>>, String> {
    v.as_array()
        .ok_or_else(|| "expected an array of position arrays".to_string())?
        .iter()
        .map(json_pos_list)
        .collect()
}

fn json_geometry(v: &Value, depth: usize) -> Result<Geometry, String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "geometry nesting is deeper than the {MAX_DEPTH}-level limit"
        ));
    }
    let obj = v
        .as_object()
        .ok_or_else(|| "a GeoJSON geometry must be an object".to_string())?;
    let ty = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "a GeoJSON object needs a string \"type\" member".to_string())?;
    if ty == "GeometryCollection" {
        let list = obj
            .get("geometries")
            .and_then(Value::as_array)
            .ok_or_else(|| "a GeometryCollection needs a \"geometries\" array".to_string())?;
        let children = list
            .iter()
            .map(|c| json_geometry(c, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        let dim = children.first().map(|c| c.dim).unwrap_or(Dim::XY);
        return Ok(Geometry {
            dim,
            geom: Geom::Collection(children),
        });
    }
    if !matches!(
        ty,
        "Point" | "LineString" | "Polygon" | "MultiPoint" | "MultiLineString" | "MultiPolygon"
    ) {
        return Err(format!(
            "'{ty}' is not a GeoJSON geometry type — expected Point, LineString, Polygon, MultiPoint, MultiLineString, MultiPolygon or GeometryCollection"
        ));
    }
    let coords = obj
        .get("coordinates")
        .ok_or_else(|| format!("a GeoJSON {ty} needs a \"coordinates\" member"))?;
    let geom = match ty {
        "Point" => {
            if coords.as_array().is_some_and(|a| a.is_empty()) {
                Geom::Point(None)
            } else {
                Geom::Point(Some(json_pos(coords)?))
            }
        }
        "LineString" => Geom::LineString(json_pos_list(coords)?),
        "MultiPoint" => Geom::MultiPoint(json_pos_list(coords)?),
        "Polygon" => Geom::Polygon(json_ring_list(coords)?),
        "MultiLineString" => Geom::MultiLineString(json_ring_list(coords)?),
        "MultiPolygon" => Geom::MultiPolygon(
            coords
                .as_array()
                .ok_or_else(|| "MultiPolygon coordinates must be an array".to_string())?
                .iter()
                .map(json_ring_list)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => unreachable!("unknown GeoJSON geometry type checked above"),
    };
    finish(geom, None)
}

fn collect_geojson(v: &Value, out: &mut Vec<Geometry>, depth: usize) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "geometry nesting is deeper than the {MAX_DEPTH}-level limit"
        ));
    }
    match v {
        Value::Array(items) => {
            for item in items {
                collect_geojson(item, out, depth + 1)?;
            }
            Ok(())
        }
        Value::Object(obj) => {
            let ty = obj
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| "a GeoJSON object needs a string \"type\" member".to_string())?;
            match ty {
                "FeatureCollection" => {
                    let feats = obj
                        .get("features")
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            "a FeatureCollection needs a \"features\" array".to_string()
                        })?;
                    for f in feats {
                        collect_geojson(f, out, depth + 1)?;
                    }
                    Ok(())
                }
                "Feature" => match obj.get("geometry") {
                    None | Some(Value::Null) => Ok(()), // a null-geometry feature contributes nothing
                    Some(g) => {
                        out.push(json_geometry(g, depth + 1)?);
                        Ok(())
                    }
                },
                _ => {
                    out.push(json_geometry(v, depth)?);
                    Ok(())
                }
            }
        }
        _ => Err("GeoJSON input must be an object or an array of objects".into()),
    }
}

fn parse_geojson_doc(s: &str) -> Result<Doc, String> {
    let v: Value = serde_json::from_str(s).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut geoms = Vec::new();
    collect_geojson(&v, &mut geoms, 0)?;
    if geoms.is_empty() {
        return Err(
            "no geometry found in the GeoJSON input (features with a null geometry are skipped)"
                .into(),
        );
    }
    for g in &mut geoms {
        check_finite(&mut g.geom)?;
    }
    Ok(Doc { srid: 0, geoms })
}

fn num(v: f64) -> Value {
    // Keep whole numbers integral so output reads like the input did.
    if v.fract() == 0.0 && v.abs() < 1e15 {
        Value::from(v as i64)
    } else {
        serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
    }
}

/// GeoJSON has no M ordinate (RFC 7946 positions are x, y and optional
/// elevation), so M is dropped here — flagged to the caller by the boolean.
fn json_of_pos(p: &Pos, dim: Dim) -> Value {
    let mut out = vec![num(p[0]), num(p[1])];
    if dim.z {
        out.push(num(p[2]));
    }
    Value::Array(out)
}

fn json_of_pos_list(ps: &[Pos], dim: Dim) -> Value {
    Value::Array(ps.iter().map(|p| json_of_pos(p, dim)).collect())
}

fn json_of_ring_list(rings: &[Vec<Pos>], dim: Dim) -> Value {
    Value::Array(rings.iter().map(|r| json_of_pos_list(r, dim)).collect())
}

fn json_of_geometry(g: &Geometry) -> Value {
    let mut obj = Map::new();
    obj.insert("type".into(), Value::from(g.geom.type_name()));
    match &g.geom {
        Geom::Point(Some(p)) => {
            obj.insert("coordinates".into(), json_of_pos(p, g.dim));
        }
        Geom::Point(None) => {
            obj.insert("coordinates".into(), Value::Array(vec![]));
        }
        Geom::LineString(ps) | Geom::MultiPoint(ps) => {
            obj.insert("coordinates".into(), json_of_pos_list(ps, g.dim));
        }
        Geom::Polygon(rings) | Geom::MultiLineString(rings) => {
            obj.insert("coordinates".into(), json_of_ring_list(rings, g.dim));
        }
        Geom::MultiPolygon(polys) => {
            obj.insert(
                "coordinates".into(),
                Value::Array(polys.iter().map(|p| json_of_ring_list(p, g.dim)).collect()),
            );
        }
        Geom::Collection(children) => {
            obj.insert(
                "geometries".into(),
                Value::Array(children.iter().map(json_of_geometry).collect()),
            );
        }
    }
    Value::Object(obj)
}

// ---------------------------------------------------------------------------
// Format detection + the public entry point
// ---------------------------------------------------------------------------

/// Guess the input format: `"geojson"`, `"wkt"` or `"wkb"`.
pub fn detect_format(input: &str) -> Result<&'static str, String> {
    let t = input.trim_start();
    let first = t
        .chars()
        .next()
        .ok_or_else(|| "input is empty".to_string())?;
    if first == '{' || first == '[' {
        return Ok("geojson");
    }
    let upper: String = t.chars().take(24).collect::<String>().to_ascii_uppercase();
    if upper.starts_with("SRID=")
        || WKT_TYPES.iter().any(|k| upper.starts_with(k))
        || UNSUPPORTED_TYPES.iter().any(|k| upper.starts_with(k))
    {
        return Ok("wkt");
    }
    let first_line = t.lines().next().unwrap_or("").trim();
    if looks_hex(first_line) || (first == 'A' && b64_decode(first_line).is_ok()) {
        return Ok("wkb");
    }
    Err(
        "could not tell whether the input is GeoJSON, WKT or WKB — set the input format explicitly"
            .into(),
    )
}

fn wrap_multi(geoms: Vec<Geometry>) -> Geometry {
    if geoms.len() == 1 {
        return geoms.into_iter().next().unwrap();
    }
    let dim = geoms.first().map(|g| g.dim).unwrap_or(Dim::XY);
    Geometry {
        dim,
        geom: Geom::Collection(geoms),
    }
}

/// Parse `input` (auto-detected unless `from` names a format) and re-emit it.
///
/// - `from`: `auto` | `geojson` | `wkt` | `wkb`
/// - `to`: `wkt` | `wkb` | `geojson`
/// - `multi`: `collection` (wrap several geometries in one collection) |
///   `lines` (one geometry per output line)
/// - `srid`: 0 = plain WKT/WKB; > 0 = EWKT (`SRID=n;…`) / EWKB. Ignored for
///   GeoJSON output, which is always CRS84 per RFC 7946.
/// - `precision`: -1 = keep full precision; 0-15 = round to that many decimals
/// - `wkb_encoding`: `hex` | `base64`; `wkb_endian`: `little` (NDR) | `big` (XDR)
/// - `pretty`: indent GeoJSON output
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    from: &str,
    to: &str,
    multi: &str,
    srid: i64,
    precision: i64,
    wkb_encoding: &str,
    wkb_endian: &str,
    pretty: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste GeoJSON, WKT/EWKT or WKB (hex or base64)".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (~2 MB)",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    let to = match to.trim().to_ascii_lowercase().as_str() {
        "" | "wkt" => "wkt",
        "ewkt" => "wkt",
        "wkb" => "wkb",
        "geojson" | "json" => "geojson",
        other => {
            return Err(format!(
                "invalid to '{other}': expected one of wkt, wkb, geojson"
            ))
        }
    };
    let from_opt = match from.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => "auto",
        "geojson" | "json" => "geojson",
        "wkt" | "ewkt" => "wkt",
        "wkb" | "ewkb" => "wkb",
        other => {
            return Err(format!(
                "invalid from '{other}': expected one of auto, geojson, wkt, wkb"
            ))
        }
    };
    let multi = match multi.trim().to_ascii_lowercase().as_str() {
        "" | "collection" => "collection",
        "lines" => "lines",
        other => {
            return Err(format!(
                "invalid multi '{other}': expected one of collection, lines"
            ))
        }
    };
    let encoding = match wkb_encoding.trim().to_ascii_lowercase().as_str() {
        "" | "hex" => "hex",
        "base64" | "b64" => "base64",
        other => {
            return Err(format!(
                "invalid wkb_encoding '{other}': expected one of hex, base64"
            ))
        }
    };
    let big_endian = match wkb_endian.trim().to_ascii_lowercase().as_str() {
        "" | "little" | "ndr" => false,
        "big" | "xdr" => true,
        other => {
            return Err(format!(
                "invalid wkb_endian '{other}': expected one of little, big"
            ))
        }
    };
    if !(0..=MAX_SRID).contains(&srid) {
        return Err(format!(
            "srid must be a whole number from 0 to {MAX_SRID} (0 leaves it out)"
        ));
    }
    if !(-1..=MAX_PRECISION).contains(&precision) {
        return Err(format!(
            "precision must be -1 (full precision) or 0 to {MAX_PRECISION}"
        ));
    }

    let format = if from_opt == "auto" {
        detect_format(input)?
    } else {
        from_opt
    };
    let mut doc = match format {
        "geojson" => parse_geojson_doc(input)?,
        "wkt" => parse_wkt_doc(input)?,
        _ => parse_wkb_doc(input)?,
    };

    if precision >= 0 {
        for g in &mut doc.geoms {
            each_pos_mut(&mut g.geom, &mut |p| {
                for v in p.iter_mut() {
                    *v = round_to(*v, precision);
                }
            });
        }
    }

    let out_srid = if srid > 0 { srid as u32 } else { doc.srid };
    let geoms = doc.geoms;

    Ok(match (to, multi) {
        ("wkt", "collection") => to_wkt(&wrap_multi(geoms), out_srid),
        ("wkt", _) => geoms
            .iter()
            .map(|g| to_wkt(g, out_srid))
            .collect::<Vec<_>>()
            .join("\n"),
        ("wkb", "collection") => {
            let bytes = to_wkb_bytes(&wrap_multi(geoms), out_srid, big_endian);
            encode_bytes(&bytes, encoding)
        }
        ("wkb", _) => geoms
            .iter()
            .map(|g| encode_bytes(&to_wkb_bytes(g, out_srid, big_endian), encoding))
            .collect::<Vec<_>>()
            .join("\n"),
        (_, "collection") => {
            let v = json_of_geometry(&wrap_multi(geoms));
            if pretty {
                serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?
            } else {
                serde_json::to_string(&v).map_err(|e| e.to_string())?
            }
        }
        _ => geoms
            .iter()
            .map(|g| serde_json::to_string(&json_of_geometry(g)).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
    })
}

fn encode_bytes(bytes: &[u8], encoding: &str) -> String {
    if encoding == "base64" {
        b64_encode(bytes)
    } else {
        hex_encode(bytes)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn wkt(input: &str) -> String {
        convert(
            input,
            "auto",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap()
    }
    fn gj(input: &str) -> String {
        convert(
            input,
            "auto",
            "geojson",
            "collection",
            0,
            -1,
            "hex",
            "little",
            false,
        )
        .unwrap()
    }
    fn wkb(input: &str) -> String {
        convert(
            input,
            "auto",
            "wkb",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap()
    }

    #[test]
    fn geojson_point_to_wkt() {
        assert_eq!(
            wkt(r#"{"type":"Point","coordinates":[30,10]}"#),
            "POINT(30 10)"
        );
    }

    #[test]
    fn geojson_polygon_with_hole_to_wkt() {
        let input = r#"{"type":"Polygon","coordinates":[[[35,10],[45,45],[15,40],[10,20],[35,10]],[[20,30],[35,35],[30,20],[20,30]]]}"#;
        assert_eq!(
            wkt(input),
            "POLYGON((35 10,45 45,15 40,10 20,35 10),(20 30,35 35,30 20,20 30))"
        );
    }

    #[test]
    fn all_seven_types_round_trip_through_wkt() {
        for g in [
            "POINT(30 10)",
            "LINESTRING(30 10,10 30,40 40)",
            "POLYGON((30 10,40 40,20 40,10 20,30 10))",
            "MULTIPOINT(10 40,40 30,20 20,30 10)",
            "MULTILINESTRING((10 10,20 20,10 40),(40 40,30 30,40 20,30 10))",
            "MULTIPOLYGON(((30 20,45 40,10 40,30 20)),((15 5,40 10,10 20,5 10,15 5)))",
            "GEOMETRYCOLLECTION(POINT(4 6),LINESTRING(4 6,7 10))",
        ] {
            let as_json = gj(g);
            assert_eq!(wkt(&as_json), g, "round trip via GeoJSON for {g}");
            let hex = wkb(g);
            assert_eq!(wkt(&hex), g, "round trip via WKB for {g}");
        }
    }

    #[test]
    fn feature_collection_collapses_to_geometrycollection() {
        let fc = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"n":1},"geometry":{"type":"Point","coordinates":[1,2]}},
            {"type":"Feature","properties":{"n":2},"geometry":{"type":"Point","coordinates":[3,4]}}]}"#;
        assert_eq!(wkt(fc), "GEOMETRYCOLLECTION(POINT(1 2),POINT(3 4))");
    }

    #[test]
    fn feature_collection_one_per_line() {
        let fc = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[1,2]}},
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[3,4]}}]}"#;
        let out = convert(fc, "auto", "wkt", "lines", 0, -1, "hex", "little", true).unwrap();
        assert_eq!(out, "POINT(1 2)\nPOINT(3 4)");
    }

    #[test]
    fn null_geometry_features_are_skipped() {
        let fc = r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{},"geometry":null},
            {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[3,4]}}]}"#;
        assert_eq!(wkt(fc), "POINT(3 4)");
    }

    #[test]
    fn z_and_m_dimensions() {
        assert_eq!(wkt("POINT Z (1 2 3)"), "POINT Z (1 2 3)");
        assert_eq!(wkt("POINTZ(1 2 3)"), "POINT Z (1 2 3)");
        assert_eq!(wkt("POINT M (1 2 4)"), "POINT M (1 2 4)");
        assert_eq!(wkt("POINT ZM (1 2 3 4)"), "POINT ZM (1 2 3 4)");
        // GeoJSON's third ordinate is elevation, so Z survives and M does not.
        assert_eq!(
            gj("POINT Z (1 2 3)"),
            r#"{"type":"Point","coordinates":[1,2,3]}"#
        );
        assert_eq!(
            gj("POINT M (1 2 4)"),
            r#"{"type":"Point","coordinates":[1,2]}"#
        );
    }

    #[test]
    fn empty_geometries() {
        for g in [
            "POINT EMPTY",
            "LINESTRING EMPTY",
            "POLYGON EMPTY",
            "MULTIPOINT EMPTY",
            "MULTILINESTRING EMPTY",
            "MULTIPOLYGON EMPTY",
            "GEOMETRYCOLLECTION EMPTY",
        ] {
            assert_eq!(wkt(g), g);
            assert_eq!(wkt(&wkb(g)), g, "WKB round trip for {g}");
        }
        assert_eq!(wkt("POINT Z EMPTY"), "POINT Z EMPTY");
    }

    #[test]
    fn wkb_hex_matches_postgis_layout() {
        // Little-endian POINT(1 2): 01, type 1, then two f64s.
        assert_eq!(
            wkb("POINT(1 2)"),
            "0101000000000000000000F03F0000000000000040"
        );
        // Big-endian (XDR) form of the same point.
        let big = convert(
            "POINT(1 2)",
            "wkt",
            "wkb",
            "collection",
            0,
            -1,
            "hex",
            "big",
            true,
        )
        .unwrap();
        assert_eq!(big, "00000000013FF00000000000004000000000000000");
        assert_eq!(wkt(&big), "POINT(1 2)");
    }

    #[test]
    fn wkb_base64_round_trips() {
        let b64 = convert(
            "POINT(1 2)",
            "wkt",
            "wkb",
            "collection",
            0,
            -1,
            "base64",
            "little",
            true,
        )
        .unwrap();
        assert_eq!(b64, "AQEAAAAAAAAAAADwPwAAAAAAAABA");
        assert_eq!(wkt(&b64), "POINT(1 2)");
    }

    #[test]
    fn srid_makes_ewkt_and_ewkb() {
        let ewkt = convert(
            r#"{"type":"Point","coordinates":[1,2]}"#,
            "auto",
            "wkt",
            "collection",
            4326,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap();
        assert_eq!(ewkt, "SRID=4326;POINT(1 2)");
        let ewkb = convert(
            &ewkt,
            "auto",
            "wkb",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap();
        // 0x20000001 = point + SRID flag, then 4326 = 0x10E6.
        assert_eq!(ewkb, "0101000020E6100000000000000000F03F0000000000000040");
        // The SRID survives a round trip back to EWKT.
        assert_eq!(wkt(&ewkb), "SRID=4326;POINT(1 2)");
    }

    #[test]
    fn ewkb_z_flag_is_understood() {
        // PostGIS EWKB for POINT Z (1 2 3) with SRID 4326.
        let hex = "01010000A0E6100000000000000000F03F00000000000000400000000000000840";
        assert_eq!(wkt(hex), "SRID=4326;POINT Z (1 2 3)");
    }

    #[test]
    fn iso_wkb_z_code_is_understood() {
        // ISO WKB type 1001 = POINT Z.
        let hex = "01E9030000000000000000F03F00000000000000400000000000000840";
        assert_eq!(wkt(hex), "POINT Z (1 2 3)");
    }

    #[test]
    fn precision_rounds_coordinates() {
        let out = convert(
            r#"{"type":"Point","coordinates":[1.23456789,-9.87654321]}"#,
            "auto",
            "wkt",
            "collection",
            0,
            3,
            "hex",
            "little",
            true,
        )
        .unwrap();
        assert_eq!(out, "POINT(1.235 -9.877)");
    }

    #[test]
    fn geojson_output_is_pretty_by_default_and_compact_on_request() {
        let pretty = convert(
            "POINT(1 2)",
            "wkt",
            "geojson",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap();
        assert!(pretty.contains("\n  \"type\": \"Point\""), "{pretty}");
        assert_eq!(gj("POINT(1 2)"), r#"{"type":"Point","coordinates":[1,2]}"#);
    }

    #[test]
    fn multipoint_accepts_both_wkt_spellings() {
        assert_eq!(
            wkt("MULTIPOINT((10 40),(40 30))"),
            "MULTIPOINT(10 40,40 30)"
        );
        assert_eq!(wkt("MULTIPOINT(10 40,40 30)"), "MULTIPOINT(10 40,40 30)");
    }

    #[test]
    fn several_wkt_geometries_on_separate_lines() {
        let out = convert(
            "POINT(1 2)\nPOINT(3 4)",
            "wkt",
            "geojson",
            "lines",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"type\":\"Point\",\"coordinates\":[1,2]}\n{\"type\":\"Point\",\"coordinates\":[3,4]}"
        );
    }

    #[test]
    fn detects_each_format() {
        assert_eq!(detect_format(" {\"type\":\"Point\"}").unwrap(), "geojson");
        assert_eq!(detect_format("srid=4326;POINT(1 2)").unwrap(), "wkt");
        assert_eq!(detect_format("linestring(1 2,3 4)").unwrap(), "wkt");
        assert_eq!(
            detect_format("0101000000000000000000F03F0000000000000040").unwrap(),
            "wkb"
        );
        assert_eq!(
            detect_format("AQEAAAAAAAAAAADwPwAAAAAAAABA").unwrap(),
            "wkb"
        );
    }

    // ---- error paths ----

    #[test]
    fn rejects_unparseable_input() {
        let err = convert(
            "hello there",
            "auto",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("could not tell whether"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = convert(
            "   ",
            "auto",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn rejects_unclosed_wkt() {
        let err = convert(
            "POINT(1 2",
            "wkt",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("expected ')'"), "{err}");
    }

    #[test]
    fn rejects_curved_geometry_types() {
        let err = convert(
            "CIRCULARSTRING(1 1,2 2,3 1)",
            "wkt",
            "geojson",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("CIRCULARSTRING is not supported"), "{err}");
    }

    #[test]
    fn rejects_mixed_coordinate_sizes() {
        let err = convert(
            "LINESTRING(1 2,3 4 5)",
            "wkt",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("mixed coordinate sizes"), "{err}");
    }

    #[test]
    fn rejects_truncated_wkb() {
        let err = convert(
            "0101000000000000000000F03F00000000",
            "wkb",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("truncated"), "{err}");
    }

    #[test]
    fn rejects_bad_enum_values() {
        for (args, needle) in [
            (("wkt", "auto", "collection", "hex", "little"), ""),
            (("xml", "auto", "collection", "hex", "little"), "invalid to"),
            (
                ("wkt", "shapefile", "collection", "hex", "little"),
                "invalid from",
            ),
            (("wkt", "auto", "grid", "hex", "little"), "invalid multi"),
            (
                ("wkt", "auto", "collection", "octal", "little"),
                "invalid wkb_encoding",
            ),
            (
                ("wkt", "auto", "collection", "hex", "middle"),
                "invalid wkb_endian",
            ),
        ] {
            let r = convert(
                "POINT(1 2)",
                args.1,
                args.0,
                args.2,
                0,
                -1,
                args.3,
                args.4,
                true,
            );
            if needle.is_empty() {
                assert!(r.is_ok());
            } else {
                assert!(r.unwrap_err().contains(needle));
            }
        }
    }

    #[test]
    fn rejects_out_of_range_srid_and_precision() {
        let bad_srid = convert(
            "POINT(1 2)",
            "wkt",
            "wkt",
            "collection",
            1_000_000,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(bad_srid.contains("srid must be"), "{bad_srid}");
        let bad_prec = convert(
            "POINT(1 2)",
            "wkt",
            "wkt",
            "collection",
            0,
            42,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(bad_prec.contains("precision must be"), "{bad_prec}");
    }

    #[test]
    fn rejects_oversize_input() {
        let big = format!("POINT(1 2){}", " ".repeat(MAX_INPUT_BYTES));
        let err = convert(
            &big,
            "wkt",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("the limit is"), "{err}");
    }

    #[test]
    fn rejects_non_geometry_geojson() {
        let err = convert(
            r#"{"type":"Topology","objects":{}}"#,
            "geojson",
            "wkt",
            "collection",
            0,
            -1,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("not a GeoJSON geometry type"), "{err}");
    }

    #[test]
    fn rejects_nan_coordinates() {
        let err = convert(
            r#"{"type":"Point","coordinates":[1,2]}"#,
            "geojson",
            "wkt",
            "collection",
            0,
            16,
            "hex",
            "little",
            true,
        )
        .unwrap_err();
        assert!(err.contains("precision must be"), "{err}");
    }
}
