## What this tool does

A `.fit` file is the compact binary format Garmin, Wahoo, and other ANT devices
write for a recorded activity. This decoder reads that file and turns it into
something you can actually read:

- **Summary** — protocol/profile version, how many `record` messages were
  decoded, the time range, a GPS bounding box, and the `session` totals and
  averages (sport, distance, moving/elapsed time, calories, speed, heart rate,
  power, ascent/descent).
- **CSV** — one row per sampled record with `timestamp,latitude,longitude,altitude_m,distance_m,speed_mps,heart_rate,cadence,power`,
  ready for a spreadsheet.
- **GPX** — a GPX 1.1 `<trk>` with `<ele>` elevation, `<time>` stamps, and
  Garmin `TrackPointExtension` heart-rate/cadence plus `<power>` extensions, for
  any GPX-aware map or training app.

## Input shape and limits

The tool works on text, so paste the file as **base64**, not the raw binary.
Convert your `.fit` first — for example `base64 ride.fit` on macOS/Linux, or
`certutil -encode ride.fit out.txt` on Windows — then paste the result. Both the
standard (`+`/`/`) and URL-safe (`-`/`_`) alphabets are accepted, `=` padding is
optional, and whitespace or line breaks in the pasted text are ignored.

- The decoded bytes must begin with a valid FIT header carrying the `.FIT`
  signature; anything else is rejected as "not a FIT file".
- Maximum decoded size is **8 MiB** — enough for very long activities.
- Only `record` (per-second samples) and `session` (activity totals) messages
  are surfaced. Positions are stored as semicircles and converted to degrees;
  the FIT "invalid" sentinels (all-`0xFF`, or `0x7FFFFFFF` for coordinates) drop
  the field rather than printing a garbage value.
- Little- and big-endian files, compressed-timestamp records, and developer
  fields are all handled; developer field bytes are skipped, and unknown message
  types are ignored instead of erroring.

## Worked example

Decode a short cycling activity (three GPS records plus a session summary) with
`format=summary` and you get:

```text
FIT file summary
================
Protocol version: 1.0
Profile version:  21.40
Data section:     192 bytes
Records decoded:  3
Time range:       2024-03-09T08:00:00Z → 2024-03-09T08:02:00Z (2:00)
GPS points:       3 (lat 40.000000..40.002000, lon -105.001000..-105.000000)

Session
-------
Sport:            cycling
Start:            2024-03-09T08:00:00Z
Total distance:   0.25 km
Total time:       2:00 (elapsed 2:00)
Calories:         42 kcal
Speed:            avg 21.6  max 25.2 (km/h)
Heart rate:       avg 137  max 150 bpm
Power:            avg 200  max 250 W
```

Switch to `format=csv` and the same file becomes a table whose first data row is:

```text
2024-03-09T08:00:00Z,40.0000000,-105.0000000,1600.0,0.00,5.000,120,80,150
```

Switch to `format=gpx` for a `<trk>` of three `<trkpt>` elements, each carrying
`<ele>`, `<time>`, `<gpxtpx:hr>`, `<gpxtpx:cad>`, and `<power>`.

## Limits and edge cases

- A record only carries whatever the device sampled at that instant, so any
  column can be blank — a bike computer without GPS produces CSV rows with empty
  latitude/longitude, and GPX skips points that have no position at all.
- Speed and altitude prefer the "enhanced" 32-bit fields when a file provides
  them, falling back to the classic 16-bit values otherwise.
- The summary reads the first `session` message. Multi-sport files show that
  first session's sport and totals.
- This is a read-only decoder: it never re-encodes or edits the FIT file, and
  laps, HR zones, and device-info messages are outside its scope.

## FAQ

<details>
<summary>Why do I have to paste base64 instead of uploading the .fit file?</summary>

The tool runs entirely in your browser as WebAssembly and takes text input, so
the binary file is encoded as base64 first. That keeps everything local — your
activity is never uploaded to a server. Run `base64 ride.fit` (or your OS's
equivalent) and paste the output.

</details>

<details>
<summary>It says "not a FIT file" — what went wrong?</summary>

The decoded bytes did not start with the FIT header signature. Usually the
base64 is incomplete (a partial copy/paste), came from a different file, or the
file was already decompressed/converted. Re-encode the original `.fit` and paste
the full string; leading/trailing whitespace is fine.

</details>

<details>
<summary>Which CSV columns will I get?</summary>

Always the same nine columns: `timestamp, latitude, longitude, altitude_m,
distance_m, speed_mps, heart_rate, cadence, power`. A cell is left empty when
that record did not include the field, so an indoor trainer ride will have empty
position columns but populated power and heart rate.

</details>

<details>
<summary>Does the GPX keep heart rate, cadence, and power?</summary>

Yes. Each `<trkpt>` includes a Garmin `TrackPointExtension` with `<gpxtpx:hr>`
and `<gpxtpx:cad>` when present, plus a `<power>` element, alongside `<ele>`
elevation and `<time>`. Points without a latitude/longitude are omitted from the
track.

</details>
