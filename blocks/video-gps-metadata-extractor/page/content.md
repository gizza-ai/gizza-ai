## About this tool

**Video GPS Metadata Extractor** reads the GPS location that phones and cameras
embed in an MP4 or MOV video's metadata and turns it into plain coordinates you
can use: latitude, longitude, altitude (when present), and a ready-made map link.
It parses the container's box tree directly in pure Rust — there is no ffmpeg, no
external service, and nothing is uploaded. Everything runs locally in your browser
as WebAssembly.

Two metadata layouts carry a video's location, and this tool reads both:

- **`©xyz` user-data atom** (in `moov/udta`) — the classic QuickTime tag that
  iPhones and many Android phones write. Its value is an ISO 6709 string such as
  `+37.7749-122.4194/`.
- **`com.apple.quicktime.location.ISO6709`** — Apple's newer layout, where a
  `keys` table names each metadata item and the value sits in the `meta`/`ilst`
  table.

Because only the metadata region matters, you don't have to paste the whole file.
A truncated head that reaches the `moov` box is enough — useful for large videos.

### How to get the bytes

The page takes the file's bytes as **base64** or **hex**, so you never upload the
video itself. A quick way to produce them from a terminal:

```
# base64 of the first 256 KB (usually reaches moov on phone videos)
head -c 262144 clip.mov | base64

# or hex of the same head
head -c 262144 clip.mov | xxd -p
```

Paste the result, choose the matching **Input encoding**, and read the report.

### Worked example

Paste the base64 of a San Francisco iPhone clip and you get:

```
GPS location found: 1

#1  ©xyz (udta)
  ISO 6709    +37.7749-122.4194
  Latitude    37.7749
  Longitude   -122.4194
  Map         https://www.openstreetmap.org/?mlat=37.7749&mlon=-122.4194#map=15/37.7749/-122.4194
```

Switch **Output** to **JSON** for a structured object with `count` and a
`locations` array (each entry has `source`, `iso6709`, `latitude`, `longitude`,
optional `altitude`, and `map_url`) — handy for scripting or feeding another tool.

### Options

- **Input encoding** — whether the pasted bytes are **base64** (standard or
  URL-safe, padding optional) or **hex** (spaces, `:`, and `-` separators are
  ignored).
- **Output** — a readable **report** (default) or structured **JSON**.

### Limits

- This reads the **static** location tag written once per file. It does not decode
  per-frame movement tracks such as GoPro's GPMF telemetry — that is a separate,
  proprietary binary stream.
- A valid video with no embedded location is reported as *none found*, not an
  error. Many videos are recorded with location tagging turned off.
- The bytes must reach the `moov` box. On some cameras `moov` sits at the end of
  the file, so a short head may not contain it — paste more of the file, or the
  whole thing, if nothing is found.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser. You
paste the file's bytes as base64 or hex, and the parsing happens on your device —
nothing is sent to a server, so it is safe for private or unpublished footage.

</details>

<details>
<summary>Which videos actually contain a GPS tag?</summary>

Videos recorded on a phone or action camera with location services enabled. iPhone
`.mov` clips and many Android `.mp4` clips write a QuickTime `©xyz` location atom;
newer Apple files use the `com.apple.quicktime.location.ISO6709` key. If location
tagging was off when recording, or the file was re-encoded by an editor or social
platform, the tag is usually stripped and the tool reports *none found*.

</details>

<details>
<summary>Do I have to paste the whole file?</summary>

Usually not. Only the `moov`/metadata region is parsed, so a truncated head of the
file that reaches the `moov` box is enough — e.g. `head -c 262144 clip.mov | base64`.
Some cameras place `moov` at the end of the file; if a short head finds nothing,
paste more of the file (or all of it).

</details>

<details>
<summary>What is the ISO 6709 string in the output?</summary>

ISO 6709 is the standard format QuickTime stores the coordinate in, e.g.
`+37.7749-122.4194/` — signed decimal latitude then longitude, with an optional
altitude in metres and a trailing `/`. The tool shows this raw string alongside
the parsed latitude, longitude, and altitude so you can verify the source value.

</details>

<details>
<summary>Does it read GoPro or dashcam movement tracks?</summary>

No. This extracts the single static location tag written in the file's metadata.
Continuous per-frame GPS — such as GoPro's GPMF telemetry track — is a separate
proprietary binary stream and is not decoded here.

</details>
