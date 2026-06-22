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
