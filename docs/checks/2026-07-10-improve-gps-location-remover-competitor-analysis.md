# gps-location-remover — competitor analysis (2026-07-10)

Paraphrased scan of the leading "remove photo location / GPS" tools. No competitor
copy, branding, or trademarks are reproduced — this is a decisions record only.

## What the tool does (scope)

Strip **only** the GPS/geolocation tags from a photo's EXIF metadata, leaving camera
make/model, lens, and exposure data (ISO, aperture, shutter, timestamps) intact. This
is deliberately narrower than the existing `strip-exif` block, which drops the entire
EXIF/GPS/XMP/comment payload. The differentiator is selective removal.

## Competitors surveyed (paraphrased)

1. **ExifTool** (open-source CLI, the reference implementation). Only tool found with
   true granular control — `-GPS*` removes every GPS-namespace tag while preserving all
   other EXIF. This is the exact behavior we target.
2. **A browser "wipe" tool** (client-side JPEG/PNG cleaner). Notable because it exposes
   *toggles* to PRESERVE the ICC colour profile and the orientation tag — i.e. its users
   care about not losing render-critical, non-location metadata when cleaning.
3. **A "remove GPS from photo" web service.** One-click; keeps pixels untouched. Framed
   around privacy-before-sharing.
4. **A general metadata remover.** Confirms the market norm: most one-click tools remove
   *all* metadata at once (GPS + camera model + serial + timestamps + ISO/aperture/
   shutter + lens + author + thumbnail), which is exactly what users of a GPS-only tool
   want to AVOID.

## Table-stakes → decision

| Capability | In model? | Decision |
| --- | --- | --- |
| Remove all GPS-namespace tags (lat/long/altitude/timestamp/datestamp/etc.) | yes | **In descriptor.** We empty the GPS sub-IFD (tag 0x8825), which holds every GPS tag as a unit. |
| Preserve camera make/model, lens, exposure (ISO/aperture/shutter), timestamps | yes | **Core guarantee.** We edit only the GPS IFD; every other IFD/sub-IFD is left byte-for-byte. |
| Preserve ICC colour profile + orientation | yes | **Automatic.** Those live in ICC (APP2) / IFD0 Orientation, which we never touch. No toggle needed — unlike strip-all tools we never risk them. |
| No pixel re-encode (no quality loss) | yes | **Core guarantee.** img-parts preserves the compressed scan data; only the EXIF payload is rewritten. |
| Preserve MakerNote (offset-sensitive) | yes | **Core guarantee.** We relocate nothing (in-place zero of the GPS block), so MakerNote absolute offsets stay valid — a known corruption hazard for naive EXIF re-serializers. |
| JPEG + PNG (eXIf chunk) support | yes | **Both supported** via img-parts `ImageEXIF`. |
| Truly erase location residue (not just hide the tag) | yes | **Core guarantee.** We zero the raw GPS coordinate bytes in the data area, not merely unlink the IFD, so no forensic residue remains. |
| Also strip GPS embedded in XMP (`exif:GPSLatitude`, etc.) | out of model (for now) | **Listed, not built.** Removing XMP GPS risks discarding non-location XMP (captions, ratings); EXIF GPS is the geotag phones/cameras actually write. Documented as a limitation. |
| Video geotag removal | out of model | **Listed, not built.** ffmpeg cannot run in the chat Service Worker; this is a still-image tool. |
| Batch / multi-file | out of model | **Listed, not built.** Single image per call (matches the block surface). |

## UX / control patterns

Image-bytes output → chat + CLI surfaces only, **no standalone page** (image-bytes have
no page render mode — same shape as `strip-exif` / `image-collage`). No scalar params: the
tool's entire value proposition is "GPS only, everything else kept", so there is nothing
to toggle. The one input is the image (url ⊕ ref).

## Honesty note

The core does NOT re-serialize the whole TIFF (which would risk MakerNote/thumbnail
corruption). It performs a bounded in-place edit of the GPS sub-IFD only. Verified in
unit tests by re-parsing the output with `kamadak-exif` and asserting the camera Make
survives while every GPS field is gone.
