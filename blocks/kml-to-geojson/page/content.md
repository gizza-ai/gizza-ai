## About this tool

KML is what Google Earth, My Maps and most survey apps hand you. GeoJSON is what
Leaflet, Mapbox, OpenLayers, QGIS and every modern web map want to read. This
converter moves map data between the two in either direction, in the browser —
your file is never uploaded, and there is no account or size-gated tier.

Going **KML → GeoJSON**, each `Placemark` becomes a GeoJSON `Feature`:

- `Point`, `LineString` and `Polygon` become the matching geometry, with a
  polygon's inner rings kept as holes; `MultiGeometry` becomes a
  `GeometryCollection`.
- `name` and `description` become properties, and everything in
  `ExtendedData`/`SimpleData` becomes a property of the same name.
- `TimeStamp` becomes a `time` property; `TimeSpan` becomes `begin` and `end`.
- Inline and shared styles (`Style`, `styleUrl`, `StyleMap`) are resolved and
  folded into simplestyle-spec properties — `stroke`, `stroke-width`,
  `stroke-opacity`, `fill`, `fill-opacity`, `marker-color` — so a styled route
  still looks like a route when a web map draws it.
- The `Folder` hierarchy becomes a slash-separated `folder` property, so a layer
  tree survives the trip instead of collapsing into one flat list.

Going **GeoJSON → KML**, all of that runs backwards: properties become `<name>`,
`<description>` and `<ExtendedData>`, simplestyle properties become an inline
`<Style>`, the `folder` property rebuilds the `<Folder>` tree, and you choose the
`<Document>` name plus the `<altitudeMode>` Google Earth should use.

### Worked example

Input (KML):

```xml
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
<Folder><name>Trails</name>
<Placemark><name>Trailhead</name><description>Parking lot</description>
<Point><coordinates>-122.0841234,37.4212345,15</coordinates></Point>
</Placemark>
</Folder>
</Document></kml>
```

Output (GeoJSON, at the default 6 decimal places):

```json
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [-122.084123, 37.421235, 15.0]
      },
      "properties": {
        "name": "Trailhead",
        "description": "Parking lot",
        "folder": "Trails"
      }
    }
  ]
}
```

Switch **Convert to** to *KML* and paste that GeoJSON back to get a KML document
with a `Trails` folder and the same placemark inside it.

### Limits and what is not carried over

- **Input is capped at 2 MB**, and a KML entry inside a KMZ at 8 MB. Larger
  exports should be split by layer first.
- **KMZ arrives base64-encoded.** A KMZ is a zip archive and this page takes
  text, so paste the base64 form — it is detected automatically by its `UEsD`
  prefix. On macOS or Linux: `base64 -w0 map.kmz` (drop `-w0` on macOS).
- **Both formats are WGS84**, so nothing is reprojected. Coordinates in another
  system must be reprojected before conversion.
- **Not carried over:** `NetworkLink` and other external references (fetching
  them would need network access), screen overlays, tours, and image or model
  files bundled inside a KMZ — a custom icon's URL survives as a property, the
  icon file itself does not.
- **KMZ output is not produced.** Take the KML result and zip it with any
  archiver to get a KMZ.

## FAQ

<details>
<summary>How do I convert a KMZ file here?</summary>

Base64-encode it and paste that text. A KMZ is a zip archive containing a KML
document (usually `doc.kml`), and this page's input is a text box, so the binary
has to arrive encoded. Run `base64 -w0 map.kmz` on Linux (or `base64 map.kmz` on
macOS), paste the result, and the archive is unzipped in your browser — the
`doc.kml` entry is used, or the first `.kml` entry if there isn't one.

</details>

<details>
<summary>Are colors, line widths and icons preserved?</summary>

Colors and line widths are, as simplestyle-spec properties. KML's own styling
lives in `Style` elements that GeoJSON has no slot for, so with **Carry styles
across** on, each placemark's resolved style (inline, `styleUrl`, or a
`StyleMap`) is written onto the feature as `stroke`, `stroke-width`,
`stroke-opacity`, `fill`, `fill-opacity` and `marker-color` — the convention web
mapping libraries already recognise. Going the other way, those same properties
are turned back into a `LineStyle`, `PolyStyle` and `IconStyle`. Custom icon
images are not embedded; only the color is.

</details>

<details>
<summary>What happens to KML folders?</summary>

With **Carry the folder hierarchy across** on, each feature gets a `folder`
property holding its full path, like `Trails/Day 1`. GeoJSON has no native
grouping, so a property is the only place that structure can live. Convert back
to KML with the same option on and the nested `<Folder>` elements are rebuilt
from those paths. Turn the option off to get a flat list.

</details>

<details>
<summary>Why would I lower the coordinate precision?</summary>

To make the file smaller. Six decimal places — the default — locates a point to
about 0.1 m, which is far finer than a phone GPS fix. Five places is roughly 1 m
and trims a long track noticeably; two places is about 1 km, useful for
country-scale outlines. The setting applies in both directions, so you can also
use it to shrink an over-precise GeoJSON on its way into KML.

</details>

<details>
<summary>Is my map data uploaded anywhere?</summary>

No. The converter is a WebAssembly module that runs inside this page, so the
document you paste stays in the browser tab — nothing is sent to a server, and
there is no account, quota or file-size tier. The same conversion is available
offline from the command line with `gizza tool kml-to-geojson`.

</details>

<details>
<summary>Can I go from GeoJSON to KML for Google Earth?</summary>

Yes — set **Convert to** to *KML*. Set the document name to whatever you want
listed in Google Earth's Places panel, and pick an altitude mode: *clamp to
ground* drapes geometry on the terrain (the safest default), *relative to
ground* reads a position's third value as height above the terrain, and
*absolute* reads it as height above sea level. Save the result as a `.kml` file
and open it directly, or zip it into a `.kmz` first.

</details>
