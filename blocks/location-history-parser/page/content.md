## About this tool

This parser turns a raw **location-history JSON export** into a readable per-day
summary: how many **places** you visited each day and how far you **traveled**.
Paste one JSON file, pick your output format and unit, and read the result — no
account, no upload, no map to pan around. Everything runs locally in your browser
as WebAssembly.

It auto-detects the common export shapes:

- **Google Takeout classic Records** — a `{ "locations": [ … ] }` file with
  `latitudeE7` / `longitudeE7` integers and `timestamp` / `timestampMs`.
- **Semantic Location History** — the monthly `{ "timelineObjects": [ … ] }`
  files with `placeVisit` and `activitySegment` objects (named places and the
  distance each activity already records).
- **On-device Timeline** — the newer `{ "semanticSegments": [ … ] }` export with
  `visit` / `activity` / `timelinePath` and `latLng` strings.
- **Dawarich / generic point arrays** — a plain `[ { "latitude", "longitude",
  "timestamp" }, … ]`, and GeoJSON `Point` FeatureCollections with a time
  property.

**How places and distance are computed.** When an export names your visited
places (Semantic and on-device Timeline), those named visits are counted
directly, and each day's distance comes from the `distance` / `distanceMeters`
each activity segment carries — the same figure the export reports. For raw GPS
streams (classic Records, Dawarich, GeoJSON) there are no named places, so the
tool uses **stop detection**: a run of consecutive points that stays within the
**stop radius** (meters) and spans at least the **minimum stay** (minutes) counts
as one visited place. Distance for raw streams is the great-circle (haversine)
sum of consecutive points on a 6,371 km-radius sphere.

### Worked example

Paste this Semantic Location History snippet, leave the output on **Text** and
unit on **km**:

```json
{"timelineObjects":[
  {"placeVisit":{"location":{"latitudeE7":400000000,"longitudeE7":-740000000,"name":"Home"},"duration":{"startTimestamp":"2024-01-01T08:00:00Z"}}},
  {"activitySegment":{"distance":5000,"duration":{"startTimestamp":"2024-01-01T09:00:00Z"}}},
  {"placeVisit":{"location":{"latitudeE7":401000000,"longitudeE7":-740000000,"name":"Office"},"duration":{"startTimestamp":"2024-01-01T10:00:00Z"}}}
]}
```

You get:

```
2024-01-01 — 2 places, 5.00 km

Total over 1 day: 2 places, 5.00 km
```

Switch **Output** to CSV for `date,places,distance_km` rows you can open in a
spreadsheet, or JSON for structured days that include each day's place names.

## FAQ

<details>
<summary>Which exports does it accept?</summary>

Google Takeout **classic Records** (`locations[]` with `latitudeE7` /
`longitudeE7`), **Semantic Location History** (`timelineObjects[]`), the newer
on-device **Timeline** (`semanticSegments[]`), a **GeoJSON** `Point`
FeatureCollection with a time property, and a plain **array of points**
(`{latitude, longitude, timestamp}`, e.g. a Dawarich export). The format is
detected automatically from the JSON structure.

</details>

<details>
<summary>Is my location data uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs entirely in your browser. The
JSON you paste never leaves your device — there is no server, upload, or network
request involved.

</details>

<details>
<summary>The days are off by one — how do I fix that?</summary>

Timestamps in the exports are UTC. Set the **UTC offset (hours)** to your local
zone so each record buckets into the right calendar day — for example `-5` for US
Eastern, `1` for Central Europe, or `5.5` for India. A visit recorded at 02:00
UTC moves to the previous day once you apply a negative offset.

</details>

<details>
<summary>What do stop radius and minimum stay do?</summary>

They only apply to **raw GPS streams** (classic Records, Dawarich, GeoJSON) that
have no named places. A cluster of consecutive points staying within **stop
radius** meters of an anchor, spanning at least **minimum stay** minutes, is
counted as one visited place. Larger radius or longer stay means fewer, more
significant places. Exports that already name your places (Semantic, on-device
Timeline) ignore both settings.

</details>

<details>
<summary>How is distance calculated, and does it match Google's number?</summary>

For **Semantic** and **on-device Timeline** exports, distance is the
`distance` / `distanceMeters` each activity segment already carries, so it
matches the export's own figure. For **raw point streams** there is no recorded
distance, so the tool sums the great-circle (haversine) distance between
consecutive points — a straight-line-between-fixes estimate that runs a little
short of real road distance.

</details>

<details>
<summary>Can it read a Takeout ZIP, KML, or draw a map?</summary>

No. This is a single-JSON, text-in / text-out tool. Unzip your Takeout archive
first and paste one JSON file; convert KML to JSON separately; and use a mapping
app for an interactive map or heatmap. Unnamed stops are reported per day rather
than reverse-geocoded to street addresses (that would need an online geocoding
service).

</details>
