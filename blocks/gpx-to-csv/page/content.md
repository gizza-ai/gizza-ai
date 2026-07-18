## About this tool

**GPX to CSV** turns a GPX file — the XML format that GPS watches, bike computers,
phone apps and mapping tools export — into a clean CSV table you can open in a
spreadsheet or import anywhere. Paste the file contents and you get one row per
point with these columns:

`point_type, name, latitude, longitude, elevation_m, time`

plus a **speed_kmh** column when you enable it, and **heart_rate_bpm**,
**cadence_rpm**, **power_w** and **temperature_c** columns whenever the file
carries those Garmin-style sensor extensions. Coordinates and elevations are
copied through verbatim, so nothing is rounded or reformatted, and every field is
RFC-4180 escaped so names containing commas or quotes stay intact.

Everything runs locally in your browser — the GPX never leaves your device.

### Worked example

Input (a two-point track):

```xml
<gpx><trk><trkseg>
  <trkpt lat="40.000000" lon="-105.000000"><ele>1600</ele><time>2024-03-09T08:00:00Z</time></trkpt>
  <trkpt lat="40.001000" lon="-105.000000"><ele>1610</ele><time>2024-03-09T08:01:00Z</time></trkpt>
</trkseg></trk></gpx>
```

Output (defaults — comma delimiter, header on, ISO time):

```csv
point_type,name,latitude,longitude,elevation_m,time
track,,40.000000,-105.000000,1600,2024-03-09T08:00:00Z
track,,40.001000,-105.000000,1610,2024-03-09T08:01:00Z
```

Tick **Add derived speed** and a `speed_kmh` column is appended — the two points
are ~111 m apart over 60 s, so the second row reads about `6.68` km/h (the first
row is blank because speed needs a previous point).

### Options

- **Point types** — keep everything (`all`) or restrict rows to track points
  (`<trkpt>`), route points (`<rtept>`), or waypoints (`<wpt>`).
- **Delimiter** — comma, semicolon (handy for comma-decimal locales), tab (TSV),
  or pipe.
- **Include header row** — turn the column-name row off for a headerless CSV.
- **Time column** — `iso` keeps the original UTC timestamp text, `unix` converts
  to epoch seconds, `none` drops the time column.
- **Derived speed** — adds `speed_kmh` computed from consecutive timestamps
  (great-circle distance ÷ elapsed time) along each track segment / route.

### Limits & edge cases

- Points need valid `lat`/`lon` attributes to appear; a point missing either is
  skipped.
- Sensor columns appear only when at least one included point actually carries
  that channel, so the CSV never has a column of blanks.
- Derived speed is a point-to-point estimate from the raw timestamps — it is not
  smoothed and does not use a Doppler/GPS speed field if the device recorded one.
- `unix` time needs parseable timestamps; if a `<time>` value can't be read as a
  date you'll get an error (switch to `iso` to keep the original text).
- GPX timestamps are UTC by spec — there is no local-timezone option because that
  would need a timezone/DST database the tool doesn't ship.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. Keep the blank line inside each. -->

<details>
<summary>What columns does the CSV have?</summary>

Always `point_type`, `name`, `latitude`, `longitude`, `elevation_m`, and `time`
(unless you set the time format to `none`). A `speed_kmh` column is added when you
enable derived speed, and `heart_rate_bpm`, `cadence_rpm`, `power_w` and
`temperature_c` are added automatically whenever the file contains those sensor
extensions. Empty cells mean that point didn't record that value.

</details>

<details>
<summary>Can it read heart rate, cadence, power and temperature?</summary>

Yes. It reads the common Garmin `TrackPointExtension` channels — heart rate
(`hr`), cadence (`cad`/`cadence`), power (`power`/`watts`) and temperature
(`atemp`/`temp`/`wtemp`) — by their local element names, so files from Garmin,
Wahoo, Polar and similar devices work even with different XML namespace prefixes.

</details>

<details>
<summary>Why is the first speed value blank?</summary>

Speed is derived from the distance and time between a point and the **previous**
point in the same track segment or route, so the very first point of each segment
has nothing to compare against and its `speed_kmh` cell is left empty. Waypoints
are independent locations, so they never get a derived speed either.

</details>

<details>
<summary>Is my GPX file uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using WebAssembly — the GPX text
you paste is processed on your own device and is never sent to a server.

</details>
