## About this tool

YouTube video IDs are the stable 11-character identifiers behind watch pages, short links, embeds and Shorts. This tool extracts that ID from common YouTube URL shapes and from front-end mirrors such as Invidious and Piped, then reports any start timestamp, playlist ID, playlist index, canonical watch URL, thumbnail URL and optional privacy-enhanced embed URL.

Paste one link or ID per line:

```text
https://youtu.be/dQw4w9WgXcQ?t=90
https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PL1234567890123&index=2&t=1m30s
https://www.youtube.com/shorts/dQw4w9WgXcQ
https://yewtu.be/watch?v=dQw4w9WgXcQ#t=1:30
```

Default output is a labelled text report. Use CSV for spreadsheets or JSON for scripts. The `timestamp` control chooses seconds, clock form, or both. The `thumbnail` control emits one of the standard `i.ytimg.com` URL variants, and the `embed` checkbox adds a `youtube-nocookie.com/embed/...` URL for video rows.

The parser also accepts bare values such as an 11-character video ID, an `@handle`, a `UC...` channel ID or a playlist ID. Those are reported with their own `kind` so a mixed list can be cleaned without losing non-video entries.

## Limits and edge cases

- Input is capped at 200 non-empty lines per run.
- Parsing is local string processing. The tool does not fetch YouTube, so it cannot verify that a video exists, is public, or still has a thumbnail.
- `hqdefault` is the safest thumbnail default. `sddefault` and `maxresdefault` may 404 for older or low-resolution uploads.
- Handles, `/user/` pages and `/c/` custom channel names are reported as names; resolving them to a `UC...` channel ID requires a live lookup and is out of scope.
- Wrapper URLs such as `attribution_link`, `redirect` and `oembed` are unwrapped up to three levels to avoid accidental loops.
- Strict mode is useful in pipelines: one invalid line turns the whole run into an error instead of an `invalid` row.

## FAQ

<details>
<summary>What is the difference between a video ID and a playlist ID?</summary>

A video ID is the 11-character value used by `watch?v=...`, `youtu.be/...`, Shorts and embeds. A playlist ID is longer and usually appears in the `list=` parameter. This tool reports both when a watch link belongs to a playlist.

</details>

<details>
<summary>Can this get the title, duration or channel name?</summary>

No. Titles, durations and channel metadata require a network request to YouTube or the YouTube Data API. This tool is intentionally local and deterministic, so it only extracts information already present in the pasted URL.

</details>

<details>
<summary>Why does maxresdefault sometimes not load?</summary>

YouTube does not generate `maxresdefault.jpg` for every upload. The parser can build that URL, but it does not fetch it to check availability. Use `hqdefault` when you need a thumbnail URL that is broadly available.

</details>

<details>
<summary>Do Invidious and Piped links work?</summary>

Yes. The parser applies YouTube-style path and query rules to any host, so links from front-ends such as `yewtu.be`, `piped.video` or `inv.nadeko.net` resolve when they carry the same `watch?v=`, short path, embed or timestamp structure.

</details>
