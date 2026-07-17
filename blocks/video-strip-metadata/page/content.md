## About this tool

Use this tool to remove privacy-sensitive video metadata before sharing a clip.
It remuxes the file with ffmpeg stream copy, so the video and audio streams are
not re-encoded and visual quality is preserved. The default removes global tags,
per-stream tags, chapter markers, location/device/timestamp-style metadata, and
the muxer's encoder tag.

Example: upload `holiday.mov`, leave `container=keep` and `chapters=remove`, and
download `holiday-metadata-stripped.mov`. Choose `container=mp4` when you want a
standard `.mp4` output and the input streams are already MP4-compatible.

Limits: this removes container metadata; it does not blur visible text, remove
burned-in date stamps, anonymize audio, or inspect every proprietary sidecar
format. Forcing MP4 is a remux, not a transcode, so incompatible codecs may fail;
use `keep` for the safest lossless path.

## FAQ

<details>
<summary>Does stripping metadata change video quality?</summary>

No. The ffmpeg command uses stream copy (`-c copy`), so the encoded video and
audio packets are copied into a clean container instead of being re-encoded.

</details>

<details>
<summary>What metadata is removed?</summary>

The tool removes global container tags, per-stream tags, and chapter metadata by
default. That covers common GPS/location, camera/device, creation timestamp,
title, comment, author, software, and encoder-style tags stored in the container.

</details>

<details>
<summary>When should I choose MP4 output?</summary>

Choose `mp4` when you specifically need a `.mp4` file and your streams are
already compatible with MP4. If the input uses codecs that MP4 cannot contain,
keep the original container instead.

</details>
