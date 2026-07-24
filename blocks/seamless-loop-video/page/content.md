## Turn a hard restart into a smooth video loop

A normal repeat jumps straight from the final frame to the first. This tool moves
the cut to the middle of the clip—where adjacent frames already flow naturally—
and blends the original ending into the original beginning. The resulting MP4 is
about the same length as the source and can be repeated by a player without a
visible hard cut.

Enter the source's **exact duration** in seconds. For a 2-second clip, choose
`duration = 2` and `crossfade = 0.25`. The output begins around the one-second
mark, dissolves the old ending into the old beginning near its middle, and ends
back around the one-second mark.

Short fades keep motion crisp; longer fades hide a larger mismatch but can show
double images. A useful starting point is 5–15% of the clip duration. Steady
shots of water, clouds, particles, abstract motion, or a fixed camera usually
loop most convincingly.

## Controls and limits

- Input videos are limited to 25 MiB and 0.5–600 seconds.
- Crossfades can be 0.05–10 seconds and must be shorter than half the clip.
- Output is H.264 MP4 and is re-encoded; **High**, **Balanced**, and **Small**
  trade file size against compression quality.
- **Remove audio** works with every clip. **Crossfade audio** requires an actual
  audio stream and blends it across the same seam.
- The page processes locally. Large, high-resolution clips need more memory and
  can be slow on phones.

<details>
<summary>Why do I need to enter the clip duration?</summary>

The filter must know where the midpoint and overlap fall before processing
starts. Enter the media duration shown by your player or file inspector. A
wrong value can cut the clip early or leave the transition in the wrong place.

</details>

<details>
<summary>Will every clip become perfectly invisible?</summary>

No crossfade can fully reconcile unrelated frames. Results are strongest when
the camera is fixed and the motion or lighting near the beginning and ending is
similar. If you see ghosting, shorten the fade or trim the source first.

</details>

<details>
<summary>What happens to the audio?</summary>

Audio is removed by default so silent videos always work. Choose **Crossfade**
only when the source has an audio track; it rotates and blends the audio at the
same point as the picture. Spoken words or strong beats can still reveal the
edit, so a muted or ambient soundtrack is often best.

</details>

<details>
<summary>Does this make a longer repeated file?</summary>

No. It creates one loopable cycle at roughly the source duration. Use the
result in a player with looping enabled, as a background video, or pass it to a
separate repeat/extend tool when you need a longer physical file.

</details>
