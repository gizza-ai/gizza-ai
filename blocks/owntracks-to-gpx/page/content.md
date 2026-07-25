## Convert OwnTracks location history to standard GPX

OwnTracks is great for logging private location history, but many map and fitness tools expect **GPX 1.1**. This converter turns an OwnTracks export into one GPX track that you can import into map viewers, GPS editors, route archives, or GIS tools.

It accepts the common OwnTracks shapes:

- JSON arrays of OwnTracks `location` messages, such as exports from `ocat --format json`.
- Recorder API objects like `{ "count": 2, "data": [ ... ] }`.
- A single OwnTracks location object.
- Recorder `.rec` files: one line per message, with an ISO timestamp, a tab, a type column, another tab, then the JSON payload.

Only location fixes are converted. Transition, waypoint, last-will, and other non-track messages are skipped. Every output point becomes a GPX `<trkpt lat="..." lon="...">` with `<time>` from `tst` (Unix seconds) and `<ele>` from `alt` when present. With **Include OwnTracks extensions** enabled, accuracy, velocity, course, battery, and tracker id are preserved under a small `ot:` extension namespace.

## Worked example

Input JSON:

```json
[
  {"_type":"location","tid":"5f","lat":52.1,"lon":5.1,"alt":10,"acc":12,"tst":1704110400},
  {"_type":"location","tid":"5f","lat":52.101,"lon":5.102,"alt":12,"acc":8,"tst":1704110700}
]
```

Output excerpt:

```xml
<trk>
  <name>Morning walk</name>
  <trkseg>
    <trkpt lat="52.1" lon="5.1">
      <ele>10</ele>
      <time>2024-01-01T12:00:00Z</time>
      <extensions>
        <ot:accuracy>12</ot:accuracy>
        <ot:tid>5f</ot:tid>
      </extensions>
    </trkpt>
  </trkseg>
</trk>
```

## Options

- **Track name** writes a `<name>` inside the GPX track.
- **Include OwnTracks extensions** keeps accuracy (`acc`), velocity (`vel`), course (`cog`), battery (`batt`), and tracker id (`tid`). Turn it off for plain GPX.
- **Split segments after gaps longer than** starts a new `<trkseg>` when the time gap between fixes exceeds the chosen number of minutes. Use this to split daily logs into trips without making separate files.
- **Drop fixes with accuracy worse than** removes points whose `acc` value is larger than the limit, while keeping points that have no `acc` field.

## Limits and edge cases

- Coordinates must be `lat` and `lon`; values can be JSON numbers or numeric strings.
- If a `.rec` record has no `tst`, the line's leading ISO timestamp is used for `<time>`.
- The converter does not simplify, smooth, map-match, or infer missing points. It preserves the fixes that OwnTracks recorded.
- All work happens locally in WebAssembly; your location history is not uploaded.

## FAQ

<details>
<summary>What OwnTracks export formats does this accept?</summary>

It accepts JSON arrays of location messages, Recorder API wrappers with a `data` array, single location objects, newline-delimited JSON objects, and Recorder `.rec` lines where the JSON payload appears after the timestamp/type columns.

</details>

<details>
<summary>Why are some records missing from the GPX?</summary>

Only `_type="location"` messages become track points. OwnTracks also stores transition, waypoint, and status messages, which are not GPS fixes. Records without latitude/longitude are skipped, and the accuracy filter can remove imprecise fixes.

</details>

<details>
<summary>Should I include OwnTracks extensions?</summary>

Leave them on when you want to preserve accuracy, speed, course, battery, and tracker id. Turn them off if the target importer only accepts plain GPX elements and rejects custom extension namespaces.

</details>

<details>
<summary>How does segment splitting work?</summary>

Set a gap threshold in minutes. If consecutive fixes are farther apart than that threshold, the converter starts a new `<trkseg>`. A value of `0` keeps all points in one continuous segment.

</details>

<details>
<summary>Is my location data uploaded?</summary>

No. Parsing and GPX generation run locally in the browser through WebAssembly. There is no network call and no server-side processing.

</details>
