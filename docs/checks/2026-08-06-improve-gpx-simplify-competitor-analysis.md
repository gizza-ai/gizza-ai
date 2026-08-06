# gpx-simplify — competitor analysis (2026-08-06)

## Function under study

Reduce the point count of a GPX track with Douglas-Peucker simplification at a user-chosen tolerance, while preserving the recognizable route shape and exporting GPX.

## Duplicate / viability check

Existing GPX blocks analyze, merge, split, scrub, or convert GPX to CSV/GeoJSON/KML. None simplifies a track by geometric tolerance. The input and output are text XML, so this fits the pure Rust/WASM model.

## Competitors reviewed

- Online GPX simplifiers commonly expose a tolerance or quality slider, show original vs reduced point counts, and return a downloadable simplified GPX.
- GPSBabel-style command-line workflows use Douglas-Peucker or similar filters, preserve endpoints, and often allow a distance threshold.
- Map editing tools sometimes include simplification alongside elevation/profile views, but those extra map previews are UI features rather than core conversion.

## Table-stakes → decisions

| Capability | Fit | Decision |
| --- | --- | --- |
| Douglas-Peucker algorithm | in-model | Built over GPX track coordinates. |
| Tolerance in meters | in-model | Built as `tolerance_meters` with slider/page control. |
| Preserve endpoints | in-model | Built. |
| Summary with point reduction | in-model | Built as `output=summary`. |
| Export simplified GPX | in-model | Built as `output=gpx`. |
| Keep elevation and timestamps | in-model | Built for retained points. |
| Coordinate precision | in-model | Built as `decimals` 0–8. |
| Extra safety sampling | in-model | Built as `keep_every`. |
| Interactive before/after map preview | out-of-model | Not built; standalone page generator has no map canvas/GIS preview. |
| Preserve arbitrary vendor extensions | out-of-model for this lightweight tool | Not built; documented as a clean GPX output limitation. |
| Reproject/geodesic survey precision | out-of-model | Not built; uses a practical local spherical approximation. |

## UX notes

The page uses a multiline GPX field, a tolerance slider, preset examples, summary output for previewing reduction, and explicit FAQs about tolerance, endpoints, extension metadata, and distance changes.
