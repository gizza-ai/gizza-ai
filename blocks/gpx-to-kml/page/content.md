## About this tool

Paste GPX XML and get a KML 2.2 document that opens in Google Earth and other KML-aware viewers. Tracks and routes become LineString placemarks, waypoints become Point placemarks, and the converter keeps useful metadata such as names, descriptions, timestamps, and elevation when those fields are present.

Use the styling controls to set the track/route line color, line width, line opacity, and waypoint icon color before exporting. The altitude mode controls how Google Earth interprets elevation values: clamp to ground for normal map overlays, absolute for 3D paths with sea-level altitude, or relative to ground for heights above terrain.

### Worked example

Input:

```xml
<gpx version="1.1">
  <metadata><name>Trip</name></metadata>
  <trk><name>Morning Run</name><trkseg>
    <trkpt lat="52.100" lon="5.100"><ele>10</ele></trkpt>
    <trkpt lat="52.101" lon="5.102"><ele>12</ele></trkpt>
  </trkseg></trk>
</gpx>
```

With the default red line, width 4, 80% opacity, and clamp-to-ground altitude mode, the output includes a KML `<Document>` named `Trip`, a shared `<LineStyle>` color of `cc4444ef`, and a `<LineString>` with coordinates `5.1,52.1,10 5.102,52.101,12`.

### Limits and edge cases

- The tool converts one GPX document at a time and returns plain KML text, not KMZ zip files.
- It reads standard `<trk>`, `<rte>`, and `<wpt>` elements. Vendor extension data is ignored so the output stays portable.
- KML colors use `aabbggrr` byte order; this tool converts from familiar CSS `#RRGGBB` and `#RGB` inputs for you.
- Empty documents or GPX files with no track, route, or waypoint return a clear error instead of an empty KML file.

## FAQ

<details>
<summary>Does this preserve tracks, routes, and waypoints?</summary>

Yes. GPX tracks (`trk`) and routes (`rte`) become KML LineString placemarks, while GPX waypoints (`wpt`) become Point placemarks. Names and descriptions are copied when the GPX file provides them.

</details>

<details>
<summary>Which altitude mode should I choose?</summary>

Use clamp to ground for normal map overlays. Choose absolute if the GPX elevation is metres above sea level and you want a 3D path. Choose relative to ground when the elevation values represent height above terrain.

</details>

<details>
<summary>Can this tool create KMZ files?</summary>

No. It returns standard KML text so you can inspect or save it directly. KMZ is a zipped KML package; if you need one, save the output as `.kml` first and package it separately.

</details>

<details>
<summary>Why do KML colors look different from CSS hex colors?</summary>

KML stores colors as `aabbggrr`: alpha first, then blue, green, and red. The form accepts CSS-style `#RRGGBB` or `#RGB` colors and converts them to the KML byte order automatically.

</details>
