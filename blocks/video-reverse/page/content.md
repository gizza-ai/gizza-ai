## About this tool

Use this when you need a clip to play backwards: reaction videos, before/after reveals, rewind effects, or a quick boomerang loop. The transform runs through ffmpeg in your browser. Your file stays local; there is no upload step.

Worked example: choose a short MP4, set `mode = reverse`, `audio = reverse`, and `quality = balanced`. A two-second clip that originally plays A → B is exported as B → A, and the soundtrack is reversed to match the picture.

For social loops, choose `mode = forward-reverse`: the output plays the original clip and then the reversed copy. `reverse-forward` starts with the backwards half and ends on the original forward motion. Use `audio = mute` for silent loops or `audio = keep` if a forward-playing narration or music bed should remain recognizable.

Limits and edge cases: input is capped at 25 MiB because reversing has to buffer decoded frames. The output is MP4/H.264 for broad browser playback. Reversing always re-encodes video, so pick **High quality** for archival clips, **Balanced** for normal sharing, or **Small file** when size matters more than detail. Very long clips are better handled in a desktop editor.

## FAQ

<details>
<summary>Does this reverse the audio too?</summary>

Yes by default. Set `audio = reverse` to match the backwards picture, `audio = keep` to leave the original soundtrack running forward, or `audio = mute` to remove it entirely.

</details>

<details>
<summary>What is boomerang mode?</summary>

`forward-reverse` plays the original clip first and appends a reversed copy. `reverse-forward` does the mirror image: it starts backwards and then plays forward. Both make a two-part loop from one input.

</details>

<details>
<summary>Why is the output always MP4?</summary>

Reverse filters force a full re-encode, so the tool writes H.264/AAC MP4: the most compatible browser video format. Use a separate video-transcode tool if you need another container afterward.

</details>

<details>
<summary>Will my file be uploaded?</summary>

No. The generated page runs ffmpeg in the browser and writes a local download. The CLI surface also runs locally against the file or reference you provide.

</details>

<details>
<summary>Why is there a 25 MiB limit?</summary>

Unlike trimming or remuxing, reverse filters must hold frames while reordering them. The cap keeps browser memory use predictable and makes failures obvious before a large encode starts.

</details>
