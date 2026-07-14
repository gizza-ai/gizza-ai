## About this tool

GPX to GeoJSON converts GPS tracks, routes, and waypoints between the formats
GPS devices and mapping tools speak. Paste GPX or KML XML and get a clean
GeoJSON `FeatureCollection` back, or flip the direction and turn a GeoJSON
document back into a GPX file — the input format is detected automatically,
so there's nothing to select except which way you're converting. Everything
runs locally in your browser; nothing is uploaded.

## How the conversion works

**GPX → GeoJSON.** A track's segments (`<trk>`/`<trkseg>`/`<trkpt>`) become a
`LineString` (or a `MultiLineString` when the track has more than one
segment); a route (`<rte>`/`<rtept>`) becomes a `LineString`; a waypoint
(`<wpt>`) becomes a `Point`. Standard tags — `name`, `cmt`, `desc`, `link`,
`keywords`, `sym`, `type` — are copied onto the feature's `properties`.
Elevation (`<ele>`) becomes the position's third coordinate, and per-point
timestamps become a `coordTimes` property (one string per point, `null` where
a point has no timestamp) — a naming convention shared with other well-known
GPX/KML→GeoJSON tools, so the output plugs into the same downstream code.

**KML → GeoJSON.** Each `Placemark`'s `Point`, `LineString`, or `Polygon`
becomes the matching GeoJSON geometry; a `Polygon`'s interior rings (holes)
are preserved as additional coordinate rings; a `MultiGeometry` becomes a
`GeometryCollection`. `name`/`description` become properties, and
`ExtendedData`/`SimpleData` entries become arbitrary properties under their
own names. `TimeSpan` becomes `begin`/`end` properties and `TimeStamp`
becomes a `time` property. With "Include KML Style/styleUrl colors" on
(the default), each Placemark's own `<Style>`, or one it references via
`styleUrl` — including through a `StyleMap`'s "normal" pair — is resolved
into simplestyle-spec-style properties: `stroke`/`stroke-width`/
`stroke-opacity` from `LineStyle`, `fill`/`fill-opacity` from `PolyStyle`
(dropped when `<fill>0</fill>` turns fill off), and `marker-color` from
`IconStyle`. KML's `aabbggrr` hex color is converted to a standard `#rrggbb`
plus a separate opacity fraction.

**GeoJSON → GPX.** Switch "Convert to" to GPX and paste a `FeatureCollection`,
a bare `Feature`, or a bare geometry. A `Point` (or each point of a
`MultiPoint`) becomes a `<wpt>`; a `LineString` or `MultiLineString` becomes a
`<trk>` with one `<trkseg>` per line; a `Polygon`'s or `MultiPolygon`'s
exterior ring becomes a `<trk>` too (GPX has no polygon primitive, so interior
rings/holes are dropped — a stated limitation, not a silent one). A
`coordTimes` property, if present, is restored as each point's `<time>`, so a
GPX → GeoJSON → GPX round trip keeps its timestamps.

## Worked example

Input (GPX track with elevation and per-point timestamps):

```xml
<gpx version="1.1"><trk><name>Morning Run</name><trkseg>
<trkpt lat="52.100" lon="5.100"><ele>10</ele><time>2026-07-01T08:00:00Z</time></trkpt>
<trkpt lat="52.101" lon="5.102"><ele>12</ele><time>2026-07-01T08:05:00Z</time></trkpt>
</trkseg></trk></gpx>
```

Output:

```json
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "geometry": {
        "type": "LineString",
        "coordinates": [
          [5.1, 52.1, 10.0],
          [5.102, 52.101, 12.0]
        ]
      },
      "properties": {
        "name": "Morning Run",
        "coordTimes": ["2026-07-01T08:00:00Z", "2026-07-01T08:05:00Z"]
      }
    }
  ]
}
```

## Limits and edge cases

- A GPX/route/track with zero points, or a KML `Placemark` with no
  `Point`/`LineString`/`Polygon`/`MultiGeometry` geometry, is skipped rather
  than emitted as an empty or null-geometry feature. If every track/route/
  waypoint is empty, the tool reports an error instead of an empty result.
- GPX `<extensions>` blocks (Garmin heart-rate/cadence/power sensor data) are
  not converted — only the standard tags listed above become properties.
- Document-level GPX `<metadata>` (author, copyright) and KML document
  metadata are not carried over; only per-track/per-point/per-Placemark data
  becomes feature properties.
- Converting GeoJSON to GPX only produces waypoints and tracks: a `Polygon`'s
  holes are dropped (GPX has no polygon concept), and there's no GeoJSON → KML
  direction — GPX is the far more common re-import target for GPS tools.
- KML style resolution is one level deep: a `styleUrl` pointing at a
  `StyleMap` follows that map's "normal" pair to a `Style`; deeper chains
  (a `StyleMap` pointing at another `StyleMap`) aren't followed.
- Input format is auto-detected from the content (a `<gpx`/`<kml` root vs. a
  JSON document) — there's no manual "which format is this" selector to get
  out of sync with what you pasted.

## FAQ

<details>
<summary>Why did some of my track's points get dropped?</summary>

They probably weren't dropped — check whether your track has more than one
`<trkseg>`. Multiple segments become a `MultiLineString` (an array of lines),
not one flat `LineString`, so a naive reader that only looks at
`coordinates[0]` will see just the first segment. All points are preserved;
they're just grouped by segment.

</details>

<details>
<summary>What happens to GPX `&lt;extensions&gt;` like heart rate or cadence?</summary>

They're intentionally not converted. `<extensions>` holds vendor-specific
sensor data (heart rate, cadence, power, temperature) that has no standard
GeoJSON property mapping, so this tool focuses on the geometry and the
standard GPX/KML tags (name, description, elevation, timestamps, styling).

</details>

<details>
<summary>Why is my GeoJSON `stroke`/`fill` color missing after converting KML?</summary>

Check "Include KML Style/styleUrl colors" is turned on (it's on by default) —
turning it off skips style resolution entirely. If it's on and the color is
still missing, the Placemark may not have its own `<Style>` and its
`styleUrl` may not resolve to any `<Style id="...">` or `<StyleMap>` in the
document (a broken or external reference can't be resolved).

</details>

<details>
<summary>Can I convert a KML Polygon to GPX?</summary>

Yes, with a caveat: GPX has no polygon primitive, so a Polygon's exterior
ring becomes a closed `<trk>` (a track that returns to its starting point),
and any interior rings (holes) are dropped. If you need the polygon shape
preserved exactly, keep it as GeoJSON or KML rather than converting to GPX.

</details>

<details>
<summary>Does this tool upload my GPS data anywhere?</summary>

No. The conversion runs entirely in your browser via WebAssembly — your GPX,
KML, or GeoJSON never leaves your device. There's no upload, no account, and
no tracking, which matters for GPS data that can reveal home addresses or
daily routines.

</details>
