## About this tool

TCX to GPX turns a Garmin Training Center (`.tcx`) workout into a GPX 1.1
track. TCX is the format Garmin, Wahoo, Polar, Zwift, and TrainingPeaks export;
GPX is the universal GPS format that Strava, Komoot, OruxMaps, Locus Map, QGIS,
and Google Earth all read. Paste the TCX XML and get a clean GPX document back —
trackpoints, elevation, and timestamps are preserved, and heart rate and cadence
carry over as GPX `TrackPointExtension` elements. Everything runs locally in your
browser; nothing is uploaded, and there is no file-size cap.

## How the conversion works

A TCX file nests its data as
`Activities` → `Activity` → `Lap` → `Track` → `Trackpoint` (routes use
`Courses` → `Course` → `Track` instead). This tool maps that structure onto
GPX 1.1:

- Each `<Activity>` (or `<Course>`) becomes one `<trk>`.
- Each TCX `<Track>` becomes one `<trkseg>`, so a multi-lap activity keeps its
  segment structure instead of being flattened into one line.
- Each `<Trackpoint>` that has a `<Position>` becomes a `<trkpt lat lon>`,
  carrying `<ele>` from `<AltitudeMeters>` and `<time>` from `<Time>` when they
  are present.
- The `Sport` attribute becomes the track's `<type>`, a `<Course>`'s `<Name>`
  (or an `<Activity>`'s `<Id>`) becomes the track's `<name>`, and the earliest
  trackpoint time becomes the document's `<metadata><time>`.
- With **Keep heart rate & cadence** on (the default), each point's
  `<HeartRateBpm>` and `<Cadence>` are emitted as Garmin `TrackPointExtension`
  elements (`<gpxtpx:hr>` / `<gpxtpx:cad>`) — the place GPS apps expect them,
  because GPX 1.1 has no core element for heart rate or cadence. Turn it off for
  a plain GPX with only latitude, longitude, elevation, and time.

Trackpoints without a position are skipped, because a GPX `<trkpt>` requires a
latitude and longitude.

## Worked example

Input (a TCX activity with one trackpoint carrying elevation, time, heart rate,
and cadence):

```xml
<TrainingCenterDatabase><Activities><Activity Sport="Running">
<Id>2026-07-01T08:00:00Z</Id>
<Lap><Track>
<Trackpoint><Time>2026-07-01T08:00:00Z</Time><Position><LatitudeDegrees>52.100</LatitudeDegrees><LongitudeDegrees>5.100</LongitudeDegrees></Position><AltitudeMeters>10</AltitudeMeters><HeartRateBpm><Value>120</Value></HeartRateBpm><Cadence>82</Cadence></Trackpoint>
</Track></Lap></Activity></Activities></TrainingCenterDatabase>
```

Output (GPX 1.1):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="gizza-ai/tcx-to-gpx"
     xmlns="http://www.topografix.com/GPX/1/1"
     xmlns:gpxtpx="http://www.garmin.com/xmlschemas/TrackPointExtension/v1"
     xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:schemaLocation="http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd">
  <metadata>
    <time>2026-07-01T08:00:00Z</time>
  </metadata>
  <trk>
    <name>2026-07-01T08:00:00Z</name>
    <type>Running</type>
    <trkseg>
      <trkpt lat="52.1" lon="5.1">
        <ele>10</ele>
        <time>2026-07-01T08:00:00Z</time>
        <extensions>
          <gpxtpx:TrackPointExtension>
            <gpxtpx:hr>120</gpxtpx:hr>
            <gpxtpx:cad>82</gpxtpx:cad>
          </gpxtpx:TrackPointExtension>
        </extensions>
      </trkpt>
    </trkseg>
  </trk>
</gpx>
```

## Limits and edge cases

- **Trackpoints need a position.** A `<Trackpoint>` with no `<Position>` (a
  paused/indoor sample with only heart rate) is skipped. If every point in the
  file lacks a position, the tool reports an error rather than emitting an empty
  `<gpx>`.
- **Laps have no GPX equivalent.** TCX lap boundaries and per-lap summaries
  (total time, distance, calories, average heart rate) are not carried over as
  GPX metadata; each TCX `<Track>` still becomes its own `<trkseg>`, preserving
  where the segments break.
- **Heart rate and cadence only.** These are the standard TCX per-point sensor
  fields and map cleanly to the Garmin `TrackPointExtension`. Power, temperature,
  speed, and running dynamics live in vendor-specific `<Extensions>` namespaces
  with no single standard target, so they are not converted.
- **Direction is one-way.** This tool converts TCX → GPX only; there is no
  GPX → TCX reverse here.
- **Paste text, no upload.** Paste the `.tcx` file's contents. The conversion is
  pure text-to-text and runs entirely in your browser via WebAssembly, so there
  is no server-side file-size limit.

## FAQ

<details>
<summary>Does converting TCX to GPX keep my heart rate and cadence?</summary>

Yes, by default. Heart rate and cadence are written as Garmin
`TrackPointExtension` elements (`<gpxtpx:hr>` and `<gpxtpx:cad>`) inside each
`<trkpt>`, which is where apps like Strava and Garmin Connect look for them —
GPX 1.1 itself has no core element for either. Turn off **Keep heart rate &
cadence** if you want a plain GPX with only position, elevation, and time.

</details>

<details>
<summary>Why did my multi-lap workout become one track with several segments?</summary>

That is intentional and lossless. Each TCX `<Track>` (Garmin usually writes one
per lap) becomes its own `<trkseg>` inside a single `<trk>`, so the segment
breaks are preserved. Every trackpoint is kept; they are just grouped by
segment, which is how GPX represents a multi-part track.

</details>

<details>
<summary>What becomes the track's name and type?</summary>

The `Sport` attribute on `<Activity>` (Running, Biking, …) becomes the track's
`<type>`. For a route, the `<Course>`'s `<Name>` becomes the `<name>`; for an
activity, the `<Id>` (its start timestamp) is used as the `<name>` when no other
name is available. The earliest trackpoint's time becomes the document's
`<metadata><time>`.

</details>

<details>
<summary>Is there a file-size limit, and is my data uploaded?</summary>

No limit and no upload. The conversion runs entirely in your browser via
WebAssembly, so your workout — which can reveal home addresses and daily
routines — never leaves your device. There is no account and no server-side size
cap; very large files are limited only by your browser's memory.

</details>

<details>
<summary>My TCX file has no GPS — can I still convert it?</summary>

Only if at least one trackpoint has a `<Position>` (latitude and longitude).
Indoor workouts (treadmill, trainer) often record heart rate and cadence but no
GPS position; those points are skipped, and if none of them have a position the
tool reports that no track could be built rather than returning an empty file.

</details>
