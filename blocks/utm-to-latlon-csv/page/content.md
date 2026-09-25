## About this tool

UTM coordinates are convenient for local surveying and field data because they are measured in metres, but spreadsheets, web maps and GIS imports often want latitude/longitude. This tool converts a pasted CSV with easting, northing and zone columns into WGS84-style latitude/longitude columns. It can also go the other way: paste latitude and longitude rows and generate UTM easting, northing, zone and hemisphere columns.

The converter auto-detects common column names such as `easting`, `northing`, `zone`, `latitude`, `longitude`, `x` and `y`. You can also name columns explicitly or use 1-based column numbers. Zone values may be written as plain numbers (`18`), latitude-band zones (`18T`), hemisphere suffixes (`56S`) or EPSG codes (`EPSG:32618`, `EPSG:32756`).

Example UTM input:

```csv
id,easting,northing,zone
p1,583960,4507523,18T
```

Default output:

```csv
id,latitude,longitude
p1,40.714349,-74.005970
```

Choose `latlon_to_utm` to reverse the conversion:

```csv
id,latitude,longitude
nyc,40.7128,-74.0060
```

with `decimals=3` returns:

```csv
id,easting,northing,zone,hemisphere
nyc,583959.372,4507350.998,18T,N
```

## Limits and edge cases

- Input is capped at 5 MB and 200,000 data rows.
- UTM covers latitudes from 80°S to 84°N. Polar UPS grids are outside this tool's model.
- Ellipsoid choices change the Transverse Mercator math, but they do not apply a datum shift. Use a GIS datum transformation when your source data is not already on the chosen datum.
- `validate_ranges` catches out-of-range eastings/northings, likely swapped columns and latitude-band mismatches. Turn it off only when you intentionally want to project unusual values.
- GeoJSON and KML output use decimal point coordinates even when the text latitude/longitude format is DMS or DDM.

## FAQ

<details>
<summary>Can I use EPSG:326xx or EPSG:327xx zone codes?</summary>

Yes. `EPSG:32618` means WGS84 / UTM zone 18 north, and `EPSG:32756` means zone 56 south. You can put those codes in a zone column or in the fallback zone field.

</details>

<details>
<summary>What happens if my CSV has no zone column?</summary>

For UTM to latitude/longitude, provide a fallback zone such as `18T`, `18N` or `EPSG:32618`. For latitude/longitude to UTM, leaving the zone blank lets the tool choose the standard UTM zone from longitude, including the Norway and Svalbard exceptions.

</details>

<details>
<summary>Does choosing Clarke 1866 or International 1924 transform datums?</summary>

No. The ellipsoid changes the projection surface used in the formulas, but it does not perform a geodetic datum shift. If the coordinates need NAD27-to-WGS84 or another datum transformation, do that in GIS software before or after this conversion.

</details>

<details>
<summary>Why did the tool say my band letter disagrees with the coordinates?</summary>

Latitude-band letters imply a north/south hemisphere and an approximate latitude span. When `validate_ranges` is on, the tool checks that the projected latitude lands near the band. If your data uses a different convention or the band was entered incorrectly, fix the zone or set hemisphere to north or south to override it.

</details>
