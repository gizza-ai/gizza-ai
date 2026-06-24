//! geometry-calculator core — pure geometry, shared by the chat skill block and
//! the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Given a shape name and its dimensions, computes the relevant measures:
//! - 2D shapes return `area` and `perimeter`.
//! - 3D shapes return `surface_area` and `volume`.
//!
//! Every dimension is interpreted in the same (unitless) length unit the caller
//! supplies; results are areas in unit², volumes in unit³. All math is `f64`.

use serde::Serialize;

/// One named numeric measure of the shape (e.g. "area", "volume").
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measure {
    /// Measure name: "area", "perimeter", "surface_area", or "volume".
    pub name: String,
    /// The computed value, rounded to 6 decimal places.
    pub value: f64,
    /// The unit suffix relative to the input length unit: "" (length),
    /// "²" (area), or "³" (volume).
    pub unit: String,
}

/// Structured geometry result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Geometry {
    /// The canonical shape name (e.g. "circle", "rectangular_prism").
    pub shape: String,
    /// Whether the shape is "2D" or "3D".
    pub dimensionality: String,
    /// The input dimensions echoed back, in the order they were consumed.
    pub dimensions: Vec<DimensionEcho>,
    /// The computed measures (area/perimeter for 2D, surface_area/volume for 3D).
    pub measures: Vec<Measure>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// An input dimension echoed in the result for transparency.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DimensionEcho {
    /// Dimension label (e.g. "radius", "width", "height").
    pub name: String,
    /// The supplied value.
    pub value: f64,
}

const PI: f64 = std::f64::consts::PI;

/// Round to 6 decimals to keep output tidy and deterministic across platforms.
fn r6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

fn echo(name: &str, value: f64) -> DimensionEcho {
    DimensionEcho {
        name: name.to_string(),
        value,
    }
}

fn measure(name: &str, value: f64, unit: &str) -> Measure {
    Measure {
        name: name.to_string(),
        value: r6(value),
        unit: unit.to_string(),
    }
}

/// Validate that a dimension is finite and strictly positive.
fn require_positive(label: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{label} must be a finite number"));
    }
    if v <= 0.0 {
        return Err(format!("{label} must be greater than zero (got {v})"));
    }
    Ok(())
}

/// Compute geometry for a shape from its dimensions.
///
/// `shape` is matched case-insensitively and tolerates spaces, hyphens and
/// underscores (e.g. "Rectangular Prism" == "rectangular_prism"). The dimension
/// parameters relevant to the chosen shape are read; the rest are ignored. A
/// missing-or-non-positive required dimension is an error.
///
/// Recognised shapes and the dimensions they use:
/// - `square` — `side`
/// - `rectangle` — `width`, `height`
/// - `triangle` — `base`, `height` (area); `side_a`/`side_b`/`side_c` (perimeter, optional)
/// - `circle` — `radius`
/// - `ellipse` — `radius_a` (semi-major), `radius_b` (semi-minor)
/// - `trapezoid` — `base`, `top` (parallel sides), `height`; `side_a`/`side_b` (legs, optional)
/// - `parallelogram` — `base`, `side_a` (slant side), `height`
/// - `regular_polygon` — `sides` (count), `side` (edge length)
/// - `cube` — `side`
/// - `rectangular_prism` (box) — `width`, `height`, `length`
/// - `sphere` — `radius`
/// - `cylinder` — `radius`, `height`
/// - `cone` — `radius`, `height`
/// - `pyramid` — square base: `base`, `height`
pub fn compute(shape: &str, d: &Dimensions) -> Result<Geometry, String> {
    let canon = normalize_shape(shape);
    match canon.as_str() {
        "square" => {
            let s = d.get("side")?;
            require_positive("side", s)?;
            Ok(geom_2d("square", vec![echo("side", s)], s * s, 4.0 * s))
        }
        "rectangle" => {
            let w = d.get("width")?;
            let h = d.get("height")?;
            require_positive("width", w)?;
            require_positive("height", h)?;
            Ok(geom_2d(
                "rectangle",
                vec![echo("width", w), echo("height", h)],
                w * h,
                2.0 * (w + h),
            ))
        }
        "triangle" => {
            let base = d.get("base")?;
            let h = d.get("height")?;
            require_positive("base", base)?;
            require_positive("height", h)?;
            let area = 0.5 * base * h;
            let mut dims = vec![echo("base", base), echo("height", h)];
            let mut measures = vec![measure("area", area, "²")];
            if let (Some(a), Some(b), Some(c)) =
                (d.opt("side_a"), d.opt("side_b"), d.opt("side_c"))
            {
                require_positive("side_a", a)?;
                require_positive("side_b", b)?;
                require_positive("side_c", c)?;
                if !triangle_inequality(a, b, c) {
                    return Err(format!(
                        "side_a={a}, side_b={b}, side_c={c} violate the triangle inequality"
                    ));
                }
                dims.push(echo("side_a", a));
                dims.push(echo("side_b", b));
                dims.push(echo("side_c", c));
                measures.push(measure("perimeter", a + b + c, ""));
            }
            let summary = summary_line("triangle", "2D", &measures);
            Ok(Geometry {
                shape: "triangle".into(),
                dimensionality: "2D".into(),
                dimensions: dims,
                measures,
                summary,
            })
        }
        "circle" => {
            let r = d.get("radius")?;
            require_positive("radius", r)?;
            Ok(geom_2d(
                "circle",
                vec![echo("radius", r)],
                PI * r * r,
                2.0 * PI * r,
            ))
        }
        "ellipse" => {
            let a = d.get("radius_a")?;
            let b = d.get("radius_b")?;
            require_positive("radius_a", a)?;
            require_positive("radius_b", b)?;
            let area = PI * a * b;
            // Ramanujan's approximation for the ellipse perimeter.
            let hh = ((a - b) * (a - b)) / ((a + b) * (a + b));
            let perim = PI * (a + b) * (1.0 + (3.0 * hh) / (10.0 + (4.0 - 3.0 * hh).sqrt()));
            Ok(geom_2d(
                "ellipse",
                vec![echo("radius_a", a), echo("radius_b", b)],
                area,
                perim,
            ))
        }
        "trapezoid" => {
            let base = d.get("base")?;
            let top = d.get("top")?;
            let h = d.get("height")?;
            require_positive("base", base)?;
            require_positive("top", top)?;
            require_positive("height", h)?;
            let area = 0.5 * (base + top) * h;
            let mut dims = vec![echo("base", base), echo("top", top), echo("height", h)];
            let mut measures = vec![measure("area", area, "²")];
            if let (Some(la), Some(lb)) = (d.opt("side_a"), d.opt("side_b")) {
                require_positive("side_a", la)?;
                require_positive("side_b", lb)?;
                dims.push(echo("side_a", la));
                dims.push(echo("side_b", lb));
                measures.push(measure("perimeter", base + top + la + lb, ""));
            }
            let summary = summary_line("trapezoid", "2D", &measures);
            Ok(Geometry {
                shape: "trapezoid".into(),
                dimensionality: "2D".into(),
                dimensions: dims,
                measures,
                summary,
            })
        }
        "parallelogram" => {
            let base = d.get("base")?;
            let side = d.get("side_a")?;
            let h = d.get("height")?;
            require_positive("base", base)?;
            require_positive("side_a", side)?;
            require_positive("height", h)?;
            Ok(geom_2d(
                "parallelogram",
                vec![echo("base", base), echo("side_a", side), echo("height", h)],
                base * h,
                2.0 * (base + side),
            ))
        }
        "regular_polygon" => {
            let n = d.get("sides")?;
            let s = d.get("side")?;
            if !n.is_finite() || n < 3.0 || n.fract() != 0.0 {
                return Err(format!("sides must be a whole number >= 3 (got {n})"));
            }
            require_positive("side", s)?;
            let area = (n * s * s) / (4.0 * (PI / n).tan());
            Ok(geom_2d(
                "regular_polygon",
                vec![echo("sides", n), echo("side", s)],
                area,
                n * s,
            ))
        }
        "cube" => {
            let s = d.get("side")?;
            require_positive("side", s)?;
            Ok(geom_3d("cube", vec![echo("side", s)], 6.0 * s * s, s * s * s))
        }
        "rectangular_prism" => {
            let w = d.get("width")?;
            let h = d.get("height")?;
            let l = d.get("length")?;
            require_positive("width", w)?;
            require_positive("height", h)?;
            require_positive("length", l)?;
            let sa = 2.0 * (w * h + w * l + h * l);
            Ok(geom_3d(
                "rectangular_prism",
                vec![echo("width", w), echo("height", h), echo("length", l)],
                sa,
                w * h * l,
            ))
        }
        "sphere" => {
            let r = d.get("radius")?;
            require_positive("radius", r)?;
            Ok(geom_3d(
                "sphere",
                vec![echo("radius", r)],
                4.0 * PI * r * r,
                (4.0 / 3.0) * PI * r * r * r,
            ))
        }
        "cylinder" => {
            let r = d.get("radius")?;
            let h = d.get("height")?;
            require_positive("radius", r)?;
            require_positive("height", h)?;
            Ok(geom_3d(
                "cylinder",
                vec![echo("radius", r), echo("height", h)],
                2.0 * PI * r * (r + h),
                PI * r * r * h,
            ))
        }
        "cone" => {
            let r = d.get("radius")?;
            let h = d.get("height")?;
            require_positive("radius", r)?;
            require_positive("height", h)?;
            let slant = (r * r + h * h).sqrt();
            Ok(geom_3d(
                "cone",
                vec![echo("radius", r), echo("height", h)],
                PI * r * (r + slant),
                (1.0 / 3.0) * PI * r * r * h,
            ))
        }
        "pyramid" => {
            // Square-based right pyramid.
            let base = d.get("base")?;
            let h = d.get("height")?;
            require_positive("base", base)?;
            require_positive("height", h)?;
            let slant = ((base / 2.0).powi(2) + h * h).sqrt();
            let sa = base * base + 2.0 * base * slant;
            Ok(geom_3d(
                "pyramid",
                vec![echo("base", base), echo("height", h)],
                sa,
                (1.0 / 3.0) * base * base * h,
            ))
        }
        other => Err(format!(
            "unknown shape '{other}'. Supported: square, rectangle, triangle, \
             circle, ellipse, trapezoid, parallelogram, regular_polygon, cube, \
             rectangular_prism, sphere, cylinder, cone, pyramid"
        )),
    }
}

/// Same as [`compute`] but returns the result as a pretty-printed JSON string
/// (for the web page). Errors are returned as the error string.
pub fn compute_json(shape: &str, d: &Dimensions) -> Result<String, String> {
    let g = compute(shape, d)?;
    serde_json::to_string_pretty(&g).map_err(|e| format!("serialization failed: {e}"))
}

fn geom_2d(shape: &str, dims: Vec<DimensionEcho>, area: f64, perimeter: f64) -> Geometry {
    let measures = vec![
        measure("area", area, "²"),
        measure("perimeter", perimeter, ""),
    ];
    let summary = summary_line(shape, "2D", &measures);
    Geometry {
        shape: shape.into(),
        dimensionality: "2D".into(),
        dimensions: dims,
        measures,
        summary,
    }
}

fn geom_3d(shape: &str, dims: Vec<DimensionEcho>, surface_area: f64, volume: f64) -> Geometry {
    let measures = vec![
        measure("surface_area", surface_area, "²"),
        measure("volume", volume, "³"),
    ];
    let summary = summary_line(shape, "3D", &measures);
    Geometry {
        shape: shape.into(),
        dimensionality: "3D".into(),
        dimensions: dims,
        measures,
        summary,
    }
}

fn summary_line(shape: &str, dim: &str, measures: &[Measure]) -> String {
    let parts: Vec<String> = measures
        .iter()
        .map(|m| format!("{} = {}{}", m.name, m.value, m.unit))
        .collect();
    format!("{shape} ({dim}): {}", parts.join(", "))
}

fn triangle_inequality(a: f64, b: f64, c: f64) -> bool {
    a + b > c && a + c > b && b + c > a
}

/// Normalize a shape name: lowercase, collapse spaces/hyphens to underscores,
/// and map common aliases to the canonical name.
fn normalize_shape(s: &str) -> String {
    let lower: String = s
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' || c == '-' { '_' } else { c })
        .collect();
    match lower.as_str() {
        "box" | "cuboid" | "rectangular_box" => "rectangular_prism".into(),
        "oval" => "ellipse".into(),
        "polygon" | "n_gon" | "ngon" => "regular_polygon".into(),
        other => other.into(),
    }
}

/// The full set of optional dimension inputs. Each field is `None` when unset.
/// `compute` reads only the fields relevant to the chosen shape.
#[derive(Debug, Clone, Default)]
pub struct Dimensions {
    pub side: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub length: Option<f64>,
    pub radius: Option<f64>,
    pub radius_a: Option<f64>,
    pub radius_b: Option<f64>,
    pub base: Option<f64>,
    pub top: Option<f64>,
    pub sides: Option<f64>,
    pub side_a: Option<f64>,
    pub side_b: Option<f64>,
    pub side_c: Option<f64>,
}

impl Dimensions {
    fn opt(&self, name: &str) -> Option<f64> {
        match name {
            "side" => self.side,
            "width" => self.width,
            "height" => self.height,
            "length" => self.length,
            "radius" => self.radius,
            "radius_a" => self.radius_a,
            "radius_b" => self.radius_b,
            "base" => self.base,
            "top" => self.top,
            "sides" => self.sides,
            "side_a" => self.side_a,
            "side_b" => self.side_b,
            "side_c" => self.side_c,
            _ => None,
        }
    }

    /// Fetch a required dimension, erroring if it was not supplied.
    fn get(&self, name: &str) -> Result<f64, String> {
        self.opt(name)
            .ok_or_else(|| format!("missing required dimension '{name}' for this shape"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> Dimensions {
        Dimensions::default()
    }

    fn val(g: &Geometry, name: &str) -> f64 {
        g.measures.iter().find(|m| m.name == name).unwrap().value
    }

    #[test]
    fn circle_area_and_perimeter() {
        let mut dim = d();
        dim.radius = Some(2.0);
        let g = compute("circle", &dim).unwrap();
        assert_eq!(g.dimensionality, "2D");
        assert!((val(&g, "area") - 12.566371).abs() < 1e-5, "{}", val(&g, "area"));
        assert!((val(&g, "perimeter") - 12.566371).abs() < 1e-5);
    }

    #[test]
    fn rectangle_area_and_perimeter() {
        let mut dim = d();
        dim.width = Some(3.0);
        dim.height = Some(4.0);
        let g = compute("rectangle", &dim).unwrap();
        assert_eq!(val(&g, "area"), 12.0);
        assert_eq!(val(&g, "perimeter"), 14.0);
    }

    #[test]
    fn square_via_side() {
        let mut dim = d();
        dim.side = Some(5.0);
        let g = compute("square", &dim).unwrap();
        assert_eq!(val(&g, "area"), 25.0);
        assert_eq!(val(&g, "perimeter"), 20.0);
    }

    #[test]
    fn triangle_area_without_sides_has_no_perimeter() {
        let mut dim = d();
        dim.base = Some(6.0);
        dim.height = Some(4.0);
        let g = compute("triangle", &dim).unwrap();
        assert_eq!(val(&g, "area"), 12.0);
        assert!(g.measures.iter().all(|m| m.name != "perimeter"));
    }

    #[test]
    fn triangle_with_sides_has_perimeter() {
        let mut dim = d();
        dim.base = Some(3.0);
        dim.height = Some(4.0);
        dim.side_a = Some(3.0);
        dim.side_b = Some(4.0);
        dim.side_c = Some(5.0);
        let g = compute("triangle", &dim).unwrap();
        assert_eq!(val(&g, "area"), 6.0);
        assert_eq!(val(&g, "perimeter"), 12.0);
    }

    #[test]
    fn triangle_inequality_rejected() {
        let mut dim = d();
        dim.base = Some(1.0);
        dim.height = Some(1.0);
        dim.side_a = Some(1.0);
        dim.side_b = Some(1.0);
        dim.side_c = Some(10.0);
        let err = compute("triangle", &dim).unwrap_err();
        assert!(err.contains("triangle inequality"), "{err}");
    }

    #[test]
    fn sphere_surface_and_volume() {
        let mut dim = d();
        dim.radius = Some(3.0);
        let g = compute("sphere", &dim).unwrap();
        assert_eq!(g.dimensionality, "3D");
        assert!((val(&g, "surface_area") - 113.097336).abs() < 1e-4);
        assert!((val(&g, "volume") - 113.097336).abs() < 1e-4);
    }

    #[test]
    fn cube_surface_and_volume() {
        let mut dim = d();
        dim.side = Some(2.0);
        let g = compute("cube", &dim).unwrap();
        assert_eq!(val(&g, "surface_area"), 24.0);
        assert_eq!(val(&g, "volume"), 8.0);
    }

    #[test]
    fn rectangular_prism_box_alias() {
        let mut dim = d();
        dim.width = Some(2.0);
        dim.height = Some(3.0);
        dim.length = Some(4.0);
        let g = compute("box", &dim).unwrap();
        assert_eq!(g.shape, "rectangular_prism");
        assert_eq!(val(&g, "volume"), 24.0);
        assert_eq!(val(&g, "surface_area"), 52.0);
    }

    #[test]
    fn cylinder_volume() {
        let mut dim = d();
        dim.radius = Some(2.0);
        dim.height = Some(5.0);
        let g = compute("cylinder", &dim).unwrap();
        assert!((val(&g, "volume") - 62.831853).abs() < 1e-4);
    }

    #[test]
    fn cone_volume() {
        let mut dim = d();
        dim.radius = Some(3.0);
        dim.height = Some(4.0);
        let g = compute("cone", &dim).unwrap();
        // (1/3) pi r^2 h = 12 pi
        assert!((val(&g, "volume") - 37.699112).abs() < 1e-4);
    }

    #[test]
    fn regular_polygon_hexagon() {
        let mut dim = d();
        dim.sides = Some(6.0);
        dim.side = Some(2.0);
        let g = compute("regular_polygon", &dim).unwrap();
        assert_eq!(val(&g, "perimeter"), 12.0);
        // area of regular hexagon side 2 = (3*sqrt(3)/2)*4 ≈ 10.392305
        assert!((val(&g, "area") - 10.392305).abs() < 1e-4, "{}", val(&g, "area"));
    }

    #[test]
    fn ellipse_area() {
        let mut dim = d();
        dim.radius_a = Some(4.0);
        dim.radius_b = Some(2.0);
        let g = compute("ellipse", &dim).unwrap();
        assert!((val(&g, "area") - 25.132741).abs() < 1e-4);
    }

    #[test]
    fn pyramid_volume() {
        let mut dim = d();
        dim.base = Some(3.0);
        dim.height = Some(4.0);
        let g = compute("pyramid", &dim).unwrap();
        assert_eq!(val(&g, "volume"), 12.0);
    }

    #[test]
    fn missing_dimension_errors() {
        let dim = d();
        let err = compute("circle", &dim).unwrap_err();
        assert!(err.contains("missing required dimension 'radius'"), "{err}");
    }

    #[test]
    fn negative_dimension_errors() {
        let mut dim = d();
        dim.radius = Some(-1.0);
        let err = compute("circle", &dim).unwrap_err();
        assert!(err.contains("greater than zero"), "{err}");
    }

    #[test]
    fn unknown_shape_errors() {
        let mut dim = d();
        dim.side = Some(1.0);
        let err = compute("dodecahedron", &dim).unwrap_err();
        assert!(err.contains("unknown shape"), "{err}");
    }

    #[test]
    fn shape_name_is_case_and_separator_insensitive() {
        let mut dim = d();
        dim.width = Some(2.0);
        dim.height = Some(3.0);
        dim.length = Some(4.0);
        let g = compute("Rectangular Prism", &dim).unwrap();
        assert_eq!(g.shape, "rectangular_prism");
        let g2 = compute("rectangular-prism", &dim).unwrap();
        assert_eq!(g2.shape, "rectangular_prism");
    }

    #[test]
    fn json_round_trips() {
        let mut dim = d();
        dim.radius = Some(1.0);
        let json = compute_json("circle", &dim).unwrap();
        assert!(json.contains("\"shape\": \"circle\""));
        assert!(json.contains("\"area\""));
    }
}
