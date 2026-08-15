## About this tool

This validator reads one GeoJSON document and reports **every** problem it can find in a single
pass, instead of stopping at the first one. Each issue carries a stable rule id and a JSON path,
so you can jump straight to the offending position in a large file — `features[42].geometry.coordinates[0][3]`
rather than "invalid geometry".

It checks the document against [RFC 7946](https://datatracker.ietf.org/doc/html/rfc7946):

- **Structure** — the `type` member and its value, `features` on a FeatureCollection, `properties`
  and `geometry` on every Feature, `coordinates` on every geometry, `geometries` on a
  GeometryCollection, and the type of a Feature `id`.
- **Coordinates** — positions must be arrays of 2 or 3 finite numbers, longitude within
  −180…180 and latitude within −90…90. Coordinates in a projected CRS (Web Mercator metres, state
  plane feet) or with the pair swapped fall out of range and are flagged with that hint.
- **Rings** — a polygon ring needs at least four positions, must repeat its first position as its
  last, and must have at least three distinct nodes before that closing repeat.
- **Winding** — the right-hand rule: exterior rings counterclockwise, holes clockwise. Reversed
  winding is the classic bug behind a map that fills the whole world instead of one shape. Keep
  "right-hand-rule violations are errors" on for a strict check, or switch it off to see them as
  warnings — the RFC words this as SHOULD, and plenty of older data winds the other way.
- **Bounding boxes** — a `bbox` is 4 values (`[west, south, east, north]`) or 6 with altitudes, all
  finite, in range, and with south below north.

A second, optional family of **warnings** covers data that is technically valid but trips up
downstream tools: duplicate consecutive nodes, zero-area rings, zero-length lines, `MultiPoint` /
`MultiLineString` / `MultiPolygon` / `GeometryCollection` holding a single element, nested
GeometryCollections, null geometries, bare geometries with no Feature wrapper, the legacy `crs`
member that RFC 7946 removed, segments jumping the antimeridian, and coordinates carrying more
decimal places than the data can possibly resolve.

Everything runs locally in your browser — the document you paste never leaves the page.

### Worked example

Input — a square that was never closed:

```json
{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1]]]}
```

Report:

```text
INVALID — 1 error, 1 warning

Errors
  1. [ring-not-closed] (root).coordinates[0]: the ring is not closed — its last position must repeat its first ([0, 1] vs [0, 0])

Warnings
  1. [bare-geometry] (root): the document is a bare geometry — valid GeoJSON, but many tools expect a Feature or FeatureCollection wrapper

Summary
  document type: Polygon
  features: 0
  geometries: 1 (Polygon 1)
  positions: 4
  rings: 1 (1 exterior, 0 interior)
  bbox members: 0
  bounds: longitude 0 .. 1, latitude 0 .. 1
```

Adding `[0,0]` back as the fourth position turns that into `VALID — 0 errors, 1 warning`.

### Limits

The document is validated in memory, so the practical ceiling is what your browser will hold in a
textarea — tens of megabytes of GeoJSON works, gigabyte extracts do not. Only **one** document per
run: line-delimited GeoJSON must be validated a line at a time. The report lists 50 issues per
severity by default and truncates the rest with an "and N more" line; the counts in the verdict
stay complete either way, and you can raise the limit to 1000.

Two things are deliberately **not** checked, because both need a full geometry engine rather than a
structural read: **self-intersection** (a ring crossing itself, or a hole crossing its exterior)
and **duplicate object members** (the same key twice in one JSON object, which the JSON parser
collapses before the validator sees it).

## FAQ

<details>
<summary>What is the right-hand rule, and why does reversed winding matter?</summary>

RFC 7946 asks for a polygon's exterior ring to be wound counterclockwise and each interior ring
(hole) clockwise, when read as longitude/latitude. Renderers and spatial databases use that
orientation to decide which side of the boundary is "inside". Get it backwards and some consumers
draw the *complement* — the whole globe with a hole where your shape should be — while others
silently correct it. The rule is a SHOULD, not a MUST, which is why this validator lets you
downgrade winding violations to warnings.

</details>

<details>
<summary>My coordinates are valid but reported as out of range. What happened?</summary>

GeoJSON positions are always `[longitude, latitude]` in WGS 84 (EPSG:4326) — longitude first,
degrees, never metres. Two things commonly break that: the pair is **swapped** (many APIs and
spreadsheets emit latitude first), or the data is still in a **projected** coordinate system such
as Web Mercator, where values run into the millions. Reproject to EPSG:4326, or swap the pair, and
the errors clear.

</details>

<details>
<summary>Does an invalid file stop the check?</summary>

No. Invalid JSON is reported as a finding (`invalid-json`) with the parser's line and column, so
you can paste a broken file safely. Beyond that, the walker keeps going after each problem and
returns the full list — that is the point of the tool. It does stop descending into a subtree it
cannot interpret at all, such as a geometry whose `type` is not a GeoJSON type, to avoid burying
the real error under cascading noise.

</details>

<details>
<summary>What is the difference between an error and a warning here?</summary>

Errors are RFC 7946 violations: the document is not conforming GeoJSON, and the verdict is
INVALID. Warnings describe documents that parse and conform but will still cause trouble
somewhere — single-element MultiPolygons, null geometries, a legacy `crs` member, twelve decimal
places on a hand-drawn boundary. Warnings never change the verdict. Turn the whole warning family
off with "Warn about valid-but-problematic data" for a spec-only check.

</details>

<details>
<summary>Can I use this in a script or in CI?</summary>

Yes — switch the report format to JSON. You get
`{valid, error_count, warning_count, errors[], warnings[], summary}`, where each issue is
`{rule, path, message}` with a stable kebab-case rule id such as `ring-not-closed`,
`winding-wrong`, or `longitude-out-of-range`. Branch on `valid`, or filter for the rules you care
about. The same validation is available from the command line, which is usually the easier CI
path.

</details>

<details>
<summary>Will it fix the problems it finds?</summary>

No — this tool only reports. It deliberately leaves your document untouched so the report can be
trusted as a description of what you actually have. Rewinding rings to the right-hand rule,
rounding coordinate precision, recomputing bbox values, and pretty-printing are formatting jobs;
use a GeoJSON formatter for those, then re-run this check.

</details>
