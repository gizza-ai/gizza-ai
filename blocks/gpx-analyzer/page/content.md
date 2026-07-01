## About this tool

**GPX Analyzer** reads a GPX file — the GPS track exported by a sports watch,
phone app, bike computer, or route planner — and summarizes it:

- **Distance** — total track length in kilometres and miles, summed
  great-circle (haversine) between consecutive track points.
- **Elevation** — total ascent and descent, plus the minimum and maximum
  altitude, when the points carry `<ele>` data.
- **Duration** — wall-clock time from the first to the last timestamped point.
- **Speed & pace** — average speed in km/h and mph, and average pace in
  min/km and min/mi.

It reads `<trkpt>` (track), `<rtept>` (route), and `<wpt>` (waypoint) points, so
it works for recorded activities and planned routes alike.

Everything is computed **locally in your browser** via WebAssembly — your GPX
file is never uploaded.

### Where to get a GPX file

- **Strava:** open an activity → ⋯ menu → *Export GPX*.
- **Garmin Connect:** open an activity → gear icon → *Export to GPX*.
- **Komoot / Ride with GPS / AllTrails:** use the route's *Export / Download GPX*
  option.
- Most apps that record or plan an outdoor activity can export `.gpx`.

### Notes

- Distance uses the horizontal great-circle distance between points; it does not
  add the extra length from elevation change, so it matches how most apps report
  track distance.
- Elevation gain is the sum of all the uphill segments. GPS altitude is noisy, so
  the figure depends on how the track was recorded (barometric vs. GPS).
- Duration, speed and pace appear only when the track has `<time>` stamps.

### Common uses

- Check the distance and climbing of a route before heading out.
- Pull pace and speed from a recorded run or ride.
- Sanity-check a track exported from one app before importing it into another.

## FAQ

<details>
<summary>Why are duration, speed and pace missing from my result?</summary>

Those stats need `<time>` stamps on the track points. Recorded activities have
them; *planned* routes exported from Komoot, Ride with GPS, etc. usually
don't — for those you'll get distance and elevation only.

</details>

<details>
<summary>Does it read heart rate, cadence or power?</summary>

Yes — sensor channels stored as Garmin TrackPointExtension (or the common
variants `hr`/`heartrate`, `cad`/`cadence`, `power`/`watts`,
`atemp`/`temperature`) are picked up, and each channel is reported as an
average plus a maximum.

</details>

<details>
<summary>Why is my elevation gain different from Strava or Garmin Connect?</summary>

The tool sums every positive elevation change between consecutive points, with
no smoothing. Platforms apply their own noise filtering and sometimes replace
GPS altitude with map elevation data, so their gain figures routinely differ —
especially for tracks recorded without a barometric altimeter.

</details>

<details>
<summary>How are the per-kilometre and per-mile splits calculated?</summary>

Split boundaries rarely land exactly on a track point, so the crossing segment
is linearly interpolated (constant pace assumed within the segment). Each
split therefore gets a precise duration, pace, and elevation gain rather than
being snapped to the nearest point.

</details>
