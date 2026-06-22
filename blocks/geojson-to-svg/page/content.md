## About this tool

Paste **GeoJSON** and get back a clean, scalable **SVG** map — rendered entirely
in your browser, with no tile servers, no API key and nothing ever uploaded.

It accepts a `FeatureCollection`, a single `Feature`, or a bare geometry, and
draws every supported type: `Point` and `MultiPoint` as circles, `LineString`
and `MultiLineString` as strokes, `Polygon` and `MultiPolygon` as filled shapes
(with inner rings cut out as holes), plus `GeometryCollection`. Coordinates are
read as `[longitude, latitude]`.

Everything is scaled to fit the chosen **width** while preserving the aspect
ratio, and you can tune the **fill**, **stroke**, **stroke width**, **point
radius** and **background** colour. Choose the **Web-Mercator** projection for a
familiar map look, or **none** to plot raw longitude/latitude.

## Example

```json
{
  "type": "FeatureCollection",
  "features": [
    { "type": "Feature",
      "geometry": { "type": "Polygon",
        "coordinates": [[[0,0],[10,0],[10,10],[0,10],[0,0]]] },
      "properties": {} },
    { "type": "Feature",
      "geometry": { "type": "Point", "coordinates": [5,5] },
      "properties": {} }
  ]
}
```

The result is a standalone `.svg` you can save, embed in a page, or drop into a
document — it stays crisp at any size because it's vector, not a bitmap.

## Privacy

The map is projected and rendered locally in your browser. Your GeoJSON is never
sent to a server.
