## About this tool

**GPX Privacy Scrubber** cleans a GPX track before you post it publicly, attach it to a race report, or send it to someone else. GPX files often expose where an activity started and ended, the exact time you recorded it, and sensor data such as heart rate or cadence. This tool removes or fuzzes those parts while preserving the rest of the XML.

Everything runs locally in your browser — the GPX text you paste is never uploaded.

### Worked example

**Input GPX**

```xml
<gpx version="1.1"><trk><trkseg><trkpt lat="52.100000" lon="4.100000"><time>2024-01-01T08:00:00Z</time></trkpt><trkpt lat="52.200000" lon="4.200000"><time>2024-01-01T08:05:00Z</time><extensions><hr>150</hr></extensions></trkpt><trkpt lat="52.300000" lon="4.300000"><time>2024-01-01T08:10:00Z</time></trkpt></trkseg></trk></gpx>
```

With the default settings, the first and last points are removed, all timestamps are rewritten, and the sensor extension block is deleted:

```xml
<gpx version="1.1"><trk><trkseg><trkpt lat="52.200000" lon="4.200000"><time>1970-01-01T00:00:00Z</time></trkpt></trkseg></trk></gpx>
```

### What each option does

- **Start/end handling** — choose `remove` to delete the first and last `<trkpt>`, `<rtept>`, or `<wpt>` elements, or `fuzz` to keep those points but move their coordinates by a deterministic offset.
- **Fuzz radius** — the offset distance, in metres, used by `fuzz` mode. The first and last points move in opposite directions so the exact start/end is hidden.
- **Rewrite all timestamps** — replaces every `<time>…</time>` value with `1970-01-01T00:00:00Z` so date, time of day, and pace are not leaked.
- **Remove sensor extensions** — deletes `<extensions>…</extensions>` blocks that often contain heart rate, cadence, power, temperature, or device metadata.

### Limits

- This is a lightweight GPX/XML string scrubber, not a full XML normalizer. Well-formed GPX exported by fitness apps is expected.
- The tool treats the first and last GPX point elements in document order as the sensitive start/end points. It does not detect your home address or perform map matching.
- `fuzz` mode is deterministic privacy fuzzing, not cryptographic anonymization. For maximum privacy, use `remove` mode.
- Timestamp scrubbing intentionally replaces times with a fixed epoch rather than preserving intervals.

## FAQ

<details>
<summary>Which GPX point types are scrubbed?</summary>

The tool looks for `<trkpt>`, `<rtept>`, and `<wpt>` elements in document order. The first and last such points are treated as the privacy-sensitive start and end locations.

</details>

<details>
<summary>Should I choose remove or fuzz?</summary>

Use **remove** when you want the strongest simple protection: the exact start and end points disappear. Use **fuzz** when you need to keep a visible start/end shape but can tolerate approximate, shifted coordinates.

</details>

<details>
<summary>Does it remove heart rate, cadence, or power data?</summary>

Yes, when **Remove sensor extensions** is enabled. It deletes every `<extensions>…</extensions>` block, where many devices store heart rate, cadence, power, temperature, hdop/vdop, and vendor metadata.

</details>

<details>
<summary>Is my GPX file uploaded?</summary>

No. The scrubber runs entirely in your browser with WebAssembly. The text you paste stays on your device.

</details>
