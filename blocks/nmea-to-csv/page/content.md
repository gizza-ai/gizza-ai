## Convert NMEA GPS logs to CSV

Paste raw NMEA 0183 GPS/GNSS sentences and convert them into a spreadsheet-ready CSV track. The
converter accepts common `$GPGGA`, `$GNRMC`, `$GPGLL`, `$GPVTG`, `$GPZDA`, and `$GPGSA` sentences
from any talker prefix, then merges sentences that share the same UTC time into one row.

The output includes time or ISO timestamp, latitude, longitude, altitude, fix quality, satellite
count, HDOP, speed, and course. Choose decimal degrees or DMS coordinates, metres or feet, knots,
km/h or mph, comma/semicolon/tab/pipe delimiters, and whether to keep a header row. Enable checksum
validation when you want lines with a bad `*XX` checksum dropped.

## Worked example

Input:

```text
$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47
$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A
```

Default output:

```csv
timestamp,time,latitude,longitude,altitude_m,fix,satellites,hdop,speed_knots,course
1994-03-23T12:35:19Z,12:35:19,48.1173,11.516667,545.4,gps,8,0.9,22.4,84.4
```

## Limits and edge cases

- This is a CSV converter, not a map viewer or GPX/KML generator.
- GSV satellite-in-view details are not emitted; only GGA satellite count and HDOP are included.
- Proprietary vendor sentences are ignored unless they use one of the supported NMEA sentence types.
- When checksum validation is enabled, sentences with an invalid checksum are skipped; lines without a
  checksum are still parsed.
- Rows require a latitude and longitude from GGA, RMC, or GLL. Speed-only VTG lines do not produce rows
  by themselves.

## FAQ

<details>
<summary>Which NMEA sentences are supported?</summary>

The converter reads GGA, RMC, GLL, VTG, ZDA, and GSA sentences. It accepts common talker prefixes such
as `GP`, `GN`, `GL`, `GA`, and `GB` because the sentence type is the last three characters before the
first comma.

</details>

<details>
<summary>How are multiple sentences merged into one CSV row?</summary>

Sentences with the same UTC time-of-day are treated as one cycle. For example, GGA supplies altitude,
fix quality, satellite count, and HDOP, while RMC supplies date, speed, and course for the same point.

</details>

<details>
<summary>What happens when a date is missing?</summary>

If RMC or ZDA provides a date, the CSV includes an ISO `timestamp` column. If the log only has time-only
sentences such as GGA, the timestamp column is omitted and the `time` column remains.

</details>

<details>
<summary>Should I enable checksum validation?</summary>

Enable it for recorded logs where the trailing `*XX` checksum is present and you want corrupt lines
dropped. Leave it off when pasting logs from devices or examples that omit checksums.

</details>
