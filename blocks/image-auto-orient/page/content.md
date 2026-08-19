## Why Photos Come Out Sideways

A phone camera almost always writes its pixels in the same sensor order, no matter how you
held it. How the phone was held is recorded separately, as a small EXIF field called
`Orientation` with a value from 1 to 8. Photo apps that read that field rotate the picture
for you; a lot of other software — upload forms, older editors, some print and CMS
pipelines — ignores it and shows the raw pixels, which is why the same photo looks upright
on your phone and sideways everywhere else.

This tool applies the correction to the pixels themselves and writes the result with **no
orientation tag at all**. Upright pixels plus no tag means nothing downstream can rotate it
again — the double-rotation you get from "fixing" a photo in a tag-aware editor and then
opening it somewhere tag-aware can't happen.

## Worked Example

A portrait photo taken on a phone, `IMG_4821.jpg`, 4032 × 3024 pixels, with
`Orientation = 6` ("rotate 90° clockwise to display"):

- **Input:** `IMG_4821.jpg` — 4032 × 3024 stored, shows upright in Photos, sideways in a
  browser upload preview.
- **Settings:** Correction `Auto — use the photo's EXIF flag`, output format
  `Same as the upload`, quality `90`.
- **Output:** `IMG_4821-oriented.jpg` — **3024 × 4032**, upright in every viewer, and
  `exiftool -Orientation` reports nothing, because the tag is gone.

The width and height swap for the quarter-turn values (5, 6, 7, 8) and stay the same for
1, 2, 3 and 4.

## The Eight EXIF Orientation Values

Each value names the correction needed to display the stored pixels the right way up. You
normally never touch this — `Auto` reads it from the file. Pick one explicitly when the tag
is missing (screenshots, scans, already-stripped files), or when it is simply wrong:

| Value | What it means | What gets applied |
|-------|---------------|-------------------|
| 1 | Already upright | nothing (the flag is just dropped) |
| 2 | Mirrored left-right | horizontal flip |
| 3 | Upside down | 180° rotation |
| 4 | Mirrored top-bottom | vertical flip |
| 5 | Mirrored, quarter turn | transpose |
| 6 | Rotated 90° clockwise | 90° clockwise |
| 7 | Mirrored, quarter turn the other way | transverse |
| 8 | Rotated 90° counter-clockwise | 90° counter-clockwise |

The four mirrored values (2, 4, 5, 7) are rare but real — they come from front-facing
cameras and some scanners — and are handled here, not silently treated as plain rotations.

## Limits and Edge Cases

- **Input:** up to 16 MiB per photo, one file at a time. There is no batch mode.
- **Formats:** JPEG, PNG and WebP are the formats the URL/CLI path accepts; the page
  additionally hands whatever your browser's file picker allows to ffmpeg, and any format
  ffmpeg can decode will work. EXIF orientation in practice only exists on JPEG (and some
  WebP/TIFF) files — a PNG has no orientation flag, so `Auto` leaves it untouched.
- **This is a re-encode, not a metadata edit.** JPEG in, JPEG out means the picture is
  decoded and re-compressed once at the chosen quality. Choose `PNG (lossless)` if you
  need to avoid any generation loss; there is no coefficient-level lossless rotation here.
- **All other metadata is dropped too**, not just the orientation tag — no EXIF, GPS,
  camera model, or capture date survives. That is a privacy win for uploads and a loss if
  you were relying on it, so keep the original if the metadata matters.
- **A photo with no orientation tag and `Auto` selected comes back unchanged** (apart from
  the re-encode). If it still looks sideways, the tag was never there — pick the matching
  value 1-8 by hand.
- **HEIC/HEIF** (the default iPhone format) is not decodable here; convert to JPEG first.

## FAQ

<details>
<summary>How is this different from just rotating the image?</summary>

A plain rotate turns the pixels by an angle you choose and leaves the EXIF orientation flag
alone — so a tag-aware viewer applies the old flag on top of your rotation and the photo
ends up wrong again. This tool reads the flag, applies exactly the transform it asks for
(including mirroring, which a rotate-by-90° control can't express), and removes the flag so
nothing gets applied twice. If you just want to turn a picture by a set angle, a rotate tool
is the right one; if a photo is sideways only in *some* apps, this is the one.

</details>

<details>
<summary>My photo has no EXIF orientation — what happens?</summary>

`Auto` finds nothing to apply and the image comes back with its pixels unchanged (it is
still decoded and re-encoded, so a JPEG is re-compressed at your chosen quality). Screenshots,
exports from most editors, and files that have already been through a metadata stripper all
land here. Fix them by selecting the value that matches what you see: 6 for a photo lying on
its right side, 8 for its left side, 3 for upside down, 2 for a mirrored selfie.

</details>

<details>
<summary>Is this lossless?</summary>

No. The photo is decoded and re-encoded once, so a JPEG loses a little quality at the
default quality of 90 — visually hard to spot, but it is a real generation of loss. True
lossless JPEG rotation works on the compressed DCT blocks instead of the pixels and is a
different operation entirely; it is not offered here. To avoid loss completely, set the
output format to PNG (lossless) — the file will be considerably larger.

</details>

<details>
<summary>Does the corrected file keep my GPS location and camera info?</summary>

No. The output carries no EXIF block at all, so the orientation flag, GPS coordinates,
camera make and model, lens, and capture timestamp are all gone. This is usually what you
want before posting a photo publicly, but if you need that metadata, keep the original file
alongside the corrected one.

</details>

<details>
<summary>Can I fix a whole folder of holiday photos at once?</summary>

Not on this page — it handles one file per run. For bulk work, use the command line example
above in a shell loop over your files; each run is a single, independent invocation.

</details>

<details>
<summary>Is my photo uploaded to a server?</summary>

No. ffmpeg is compiled to WebAssembly and runs inside your browser tab, so the image is
decoded, rotated and re-encoded on your own machine. Closing the page discards everything.

</details>
