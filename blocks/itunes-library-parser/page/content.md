## About this tool

This parser reads the XML property list that Music and iTunes write as `Library.xml` and turns it
into something you can actually use: a spreadsheet of your track metadata, a JSON dump for a
script, an M3U playlist another player can open, an index of every playlist in the file, or a
one-screen summary of the whole library.

Everything runs on the text you paste. The library file is never uploaded, no music is read from
disk, and nothing is written anywhere. That also means the tool works on a library exported from a
machine you no longer have — all it needs is the XML.

## Getting your Library.xml

In Music on macOS choose **File → Library → Export Library…** and save the XML file. In iTunes on
Windows, the same command exists, and there is usually an `iTunes Music Library.xml` sitting next
to your library in the iTunes folder (turn on the preference that shares the library XML with other
applications if it is missing). Open the file in a text editor, copy all of it, and paste it above.

The file must be the XML form — it starts with `<?xml` and contains a `<plist>` element. The binary
`.itl` library next to it is a private database format and cannot be read by this tool or any other
third-party one.

## Worked example

A two-track library with one user playlist, exported with the default columns:

```xml
<key>Tracks</key>
<dict>
  <key>101</key>
  <dict>
    <key>Track ID</key><integer>101</integer>
    <key>Name</key><string>Let It Happen</string>
    <key>Artist</key><string>Tame Impala</string>
    <key>Album</key><string>Currents</string>
    <key>Genre</key><string>Electronic</string>
    <key>Year</key><integer>2015</integer>
    <key>Total Time</key><integer>467000</integer>
    <key>Location</key><string>file://localhost/Users/me/Music/iTunes/iTunes%20Media/Music/Tame%20Impala/Currents/01%20Let%20It%20Happen.m4a</string>
  </dict>
  ...
</dict>
```

Output with `output=csv` and the default `fields`:

```text
name,artist,album,genre,year,duration,location
Let It Happen,Tame Impala,Currents,Electronic,2015,7:47,/Users/me/Music/iTunes/iTunes Media/Music/Tame Impala/Currents/01 Let It Happen.m4a
All I Need,Air,Moon Safari,Electronic,1998,4:28,/Users/me/Music/iTunes/iTunes Media/Music/Air/Moon Safari/03 All I Need.m4a
```

Switching the same input to `output=m3u8` with `playlist=Late Night` gives an extended playlist in
the playlist's own running order:

```text
#EXTM3U
#EXTINF:268,Air - All I Need
/Users/me/Music/iTunes/iTunes Media/Music/Air/Moon Safari/03 All I Need.m4a
#EXTINF:467,Tame Impala - Let It Happen
/Users/me/Music/iTunes/iTunes Media/Music/Tame Impala/Currents/01 Let It Happen.m4a
```

## Columns you can export

`fields` takes a comma-separated list in the order you want the columns, and applies to the CSV,
TSV and JSON outputs. The default is `name,artist,album,genre,year,duration,location`.

Available: `track_id`, `name`, `artist`, `album_artist`, `composer`, `album`, `grouping`, `work`,
`genre`, `kind`, `size`, `duration` (`m:ss`), `duration_seconds`, `disc_number`, `disc_count`,
`track_number`, `track_count`, `year`, `bpm`, `date_added`, `date_modified`, `release_date`,
`bit_rate`, `sample_rate`, `comments`, `play_count`, `play_date`, `skip_count`, `rating` (0–5
stars), `rating_raw` (the stored 0–100 value), `album_rating`, `loved`, `compilation`, `podcast`,
`sort_name`, `sort_artist`, `sort_album`, `persistent_id`, `location`.

JSON keeps native types, so a missing field is `null` and numbers stay numbers. CSV quotes any cell
containing a comma, quote or newline; TSV strips tabs and newlines out of cells instead, because
TSV has no quoting convention.

## Fixing file paths for another machine

Track locations are stored as percent-encoded `file://` URLs, which are decoded back to ordinary
paths for every output. Two options rewrite them:

- **Replace music folder with** swaps the library's own music-folder prefix for a new root, so a
  playlist exported on one machine can point at a different drive letter, an external disk, or a
  network share. Tracks stored outside that folder keep their original path.
- **Path style** controls slash direction: keep whatever was recorded, force forward slashes, force
  backslashes, or drop every folder and keep only the file name for a flat folder of copied music.

## Limits and edge cases

- Up to 20 MB of library XML per run, which is roughly a 40,000-track library.
- `limit` caps the number of track rows and is applied *after* sorting; `0` means no limit and the
  ceiling is 100,000 rows.
- Only the XML property list form is readable; binary `.itl` libraries are not.
- Tracks with no `Location` (cloud-only or missing files) come out with an empty path in CSV, TSV
  and JSON, and are skipped entirely in M3U, because a blank line is not a valid playlist entry.
- Selecting a playlist *folder* returns an error rather than an empty export — folders hold other
  playlists, not tracks of their own.
- Sorting is ascending and stable, using the library's sort-name fields when present; tracks
  missing the sort field are placed last rather than clustered at the top.
- Smart playlists are exported from the track list the library recorded, not by re-evaluating their
  rules, so the result matches what the library last saved.

## FAQ

<details>
<summary>What is the difference between the two M3U outputs?</summary>

`m3u` writes one file path per line and nothing else, which is what simple players and car stereos
expect. `m3u8` writes the extended form: an `#EXTM3U` header plus an `#EXTINF` line before each
path carrying the track's duration in seconds and an `Artist - Title` label. Both are UTF-8, so
accented artist and album names survive either way.

</details>

<details>
<summary>Why don't I see the Music, Downloaded or Recently Added playlists?</summary>

The playlist index hides the ones the app creates for itself so you only see your own. Turn on
**Include built-in playlists** to list them too. They are detected from the `Master` and
`Distinguished Kind` markers plus a list of well-known names, because libraries written by older
versions and by third-party exporters often omit both markers. Exporting a built-in playlist by
name always works regardless of that setting.

</details>

<details>
<summary>How do I find the exact playlist name to type?</summary>

Run the tool once with **Export as → Playlist index**. It returns a CSV of every playlist with its
name, kind (playlist, smart, folder or built-in), track count, parent folder and persistent ID.
Playlist names are matched case-insensitively, so `late night` and `Late Night` both work, and an
unrecognised name comes back with a list of the names that do exist in your file.

</details>

<details>
<summary>Why do the star ratings look odd?</summary>

Ratings are stored as a 0–100 number in steps of 20, so three stars is `60`. The `rating` column
converts that to 0–5 stars (half stars included), and `rating_raw` keeps the stored number if you
would rather work with it directly. A track that has never been rated has no value at all and comes
out blank, not as zero.

</details>

<details>
<summary>Can it copy or rename my actual music files?</summary>

No. The tool only reads the pasted text and returns text, so it never touches your filesystem. What
it can do is produce the paths a copy or move would need: use the file-name path style for a flat
folder, or re-root the paths onto the drive you are copying to, then feed the resulting list to
whatever tool actually moves the files.

</details>
