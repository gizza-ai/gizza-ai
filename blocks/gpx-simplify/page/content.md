## About this tool

Large GPX tracks slow down map apps, syncing, and route sharing. This tool reduces the number of `<trkpt>` points with the Douglas-Peucker algorithm: endpoints are always kept, then the algorithm keeps only the intermediate points needed to stay within your chosen tolerance in meters.

The result is a clean GPX 1.1 document. Elevation and time values on kept points are preserved, and the summary output lets you preview how many points would be removed before exporting.

### Worked example

Input:

```
<gpx><trk><trkseg>
<trkpt lat="0" lon="0"><ele>1</ele><time>2026-08-06T00:00:00Z</time></trkpt>
<trkpt lat="0" lon="0.0001"></trkpt>
<trkpt lat="0.001" lon="0.001"></trkpt>
<trkpt lat="0" lon="0.002"></trkpt>
</trkseg></trk></gpx>
```

With a 50 m tolerance the jitter point near the start is dropped while the bend and both endpoints remain:

```
<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="gpx-simplify" xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <trkseg>
      <trkpt lat="0" lon="0">
        <ele>1</ele>
        <time>2026-08-06T00:00:00Z</time>
      </trkpt>
      <trkpt lat="0.001" lon="0.001"></trkpt>
      <trkpt lat="0" lon="0.002"></trkpt>
    </trkseg>
  </trk>
</gpx>
```

Switching the output to **Summary only** previews the same run without exporting XML:

```
GPX simplify: 4 → 3 points (25.0% removed)
Tolerance: 50 m
Distance before: 318.0 m
Distance after: 314.5 m
```

Raising the tolerance to 200 m removes the bend as well (4 → 2 points), because the bend sits only about 111 m off the straight line between the endpoints.

### Limits and edge cases

- The parser focuses on GPX track points (`<trkpt lat="…" lon="…">`). Routes (`<rtept>`) and waypoints (`<wpt>`) are not simplified.
- All `<trkpt>` points in the document are treated as one continuous path and written back as a single `<trk>`/`<trkseg>`. Multi-track or multi-segment files are flattened, so split them first if segment boundaries matter.
- The input must contain at least two track points; a file with one point (or none) returns an error instead of an empty track.
- Elevation (`<ele>`) and timestamp (`<time>`) are preserved for points that remain; other nested extension metadata is not copied.
- Distances use a local spherical/equirectangular approximation, which is appropriate for normal GPS tracks but not survey-grade geodesy.
- A tolerance of 0 keeps every bend; higher tolerances shrink the file more aggressively.

<details>
<summary>What tolerance should I use?</summary>

For running and hiking tracks, 5–20 meters usually removes GPS jitter while preserving shape. For driving or country-scale tracks, 50–200 meters may be fine. Use the summary output to preview the reduction before exporting GPX.

</details>

<details>
<summary>Does it keep the first and last point?</summary>

Yes. Douglas-Peucker always keeps segment endpoints, and this tool also lets you keep every Nth original point as an extra safety sample.

</details>

<details>
<summary>Will this preserve heart-rate or extension data?</summary>

No. The simplified GPX keeps coordinates plus the common `ele` and `time` children for retained points. Vendor extensions are intentionally dropped in the clean output.

</details>

<details>
<summary>Why did my distance change slightly?</summary>

Removing points straightens tiny wiggles, so the simplified path length can be a little shorter. Lower the tolerance if you need more detail.

</details>

<details>
<summary>What does "keep every Nth source point" do?</summary>

It is an extra safety sample on top of Douglas-Peucker: with a value of 10, every 10th original point is retained even if the tolerance would have dropped it. That bounds how far apart kept points can drift on long straight stretches — useful when a downstream app expects a roughly regular sampling rate. Leave it at 0 to let the tolerance decide alone.

</details>

<details>
<summary>How do the coordinate decimals affect file size?</summary>

Each decimal place is one more character per coordinate. Six decimals is about 11 cm of precision at the equator; five is about 1.1 m, which is already finer than consumer GPS accuracy. Dropping from six to five decimals shrinks a large track noticeably with no visible change on a map. Trailing zeros are trimmed automatically.

</details>

<details>
<summary>My file has several tracks or segments — what happens?</summary>

Every `<trkpt>` in the document is read in order as one path and written back as a single track with one segment. If the segment breaks are meaningful (for example a paused recording), split the file before simplifying and run each part separately.

</details>
