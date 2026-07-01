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

## FAQ

<details>
<summary>My shapes come out mirrored or in the wrong place — why?</summary>

The most common cause is swapped coordinates. GeoJSON positions are
`[longitude, latitude]` (x before y), the opposite of the "lat, lon" order
most people say aloud. The tool follows the GeoJSON spec strictly, so if your
data was exported lat-first, every point lands transposed.

</details>

<details>
<summary>How do I control the size of the SVG?</summary>

Set the **width** (accepted range 16–4096 px, default 800). The height isn't
a separate setting — it's derived from your data's bounding box so the aspect
ratio is preserved, with a small margin added around the drawing.

</details>

<details>
<summary>What happens to features near the poles in Mercator mode?</summary>

Web-Mercator can't represent the poles, so latitudes are clamped to
±85.05° before projecting — anything beyond that is drawn at the clamp
edge rather than producing infinities. If you're plotting polar data, switch
the projection to **none** to plot raw longitude/latitude instead.

</details>

<details>
<summary>Which GeoJSON types are supported?</summary>

`FeatureCollection`, single `Feature`, or a bare geometry, containing `Point`
/ `MultiPoint` (drawn as circles), `LineString` / `MultiLineString`
(strokes), `Polygon` / `MultiPolygon` (fills, with inner rings cut out as
holes), and nested `GeometryCollection`. An unrecognized geometry type stops
the render with an explicit error naming the type.

</details>
