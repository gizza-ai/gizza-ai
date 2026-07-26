## About this tool

This point-in-polygon checker tests a batch of latitude/longitude points against
one geofence polygon and reports whether each point is inside, outside, or exactly
on the boundary. It is built for quick geofencing QA: paste a boundary, paste
points from a spreadsheet or a GeoJSON export, choose how boundary points should
be classified, and copy the text, CSV, or JSON result.

The polygon input accepts:

- GeoJSON `Polygon` or `MultiPolygon`, including holes and `Feature` wrappers.
- A simple ring as one coordinate pair per line.
- A JSON array of coordinate pairs for quick scripted input.

Point input accepts CSV-style `lat,lon[,label]` lines, JSON arrays/objects, and
GeoJSON `Point`, `MultiPoint`, `Feature`, or `FeatureCollection` values. For
non-GeoJSON formats you can choose `lat,lon` or `lon,lat`; GeoJSON always uses
standard `[longitude, latitude]` ordering.

### Worked example

Use this square polygon:

```text
0,0
0,10
10,10
10,0
```

and these points:

```text
5,5,Center
10,5,Edge
20,20,Far away
```

With **Boundary = Inside** and **Output = Text**, the result is:

```text
3 points: 2 inside, 1 outside
#1  5, 5 (Center)  inside
#2  10, 5 (Edge)  inside
#3  20, 20 (Far away)  outside
```

Switch to CSV for spreadsheet-friendly `point,latitude,longitude,label,status`
rows, or JSON for structured test results.

## FAQ

<details>
<summary>Does it support holes and multipolygons?</summary>

Yes. GeoJSON polygons can include interior rings; a point inside a hole is
reported outside. `MultiPolygon` inputs are treated as a union of polygons, so a
point is inside when it falls inside any polygon part and not inside that part's
holes.

</details>

<details>
<summary>Which coordinate order should I pick?</summary>

Use **Latitude, Longitude** for ordinary pasted rows such as `40.0,-105.0`.
Choose **Longitude, Latitude** when your non-GeoJSON source is written in GIS
`x,y` order. GeoJSON ignores this setting because GeoJSON coordinates are always
`[longitude, latitude]`.

</details>

<details>
<summary>How are points exactly on the polygon edge handled?</summary>

The **On-boundary points count as** control decides that. Pick **Inside** to match
common geofence behavior, **Outside** for strict containment, or **Boundary** when
you want edge hits reported as their own status in the output.

</details>

<details>
<summary>Can I draw a polygon on a map or import KML/GPX?</summary>

No. This is a deterministic text/GeoJSON checker, not an interactive map editor.
Convert KML or GPX to GeoJSON first, or paste simple coordinate lines. It also
uses decimal degrees only and does not reproject coordinates from other CRSs.

</details>
