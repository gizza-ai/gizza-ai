## About this tool

Some clips almost loop — a candle flicker, a drifting cloud, a looping animation —
but replaying them shows an ugly jump where the last frame snaps back to the first.
This tool fixes that by **crossfading the clip's tail back into its head**: it
overlaps the final moments of the clip onto the opening moments and dissolves
between them, so the end flows into the start. You get back **one MP4 that is a
little shorter** and loops perfectly — set it to repeat and the join is invisible.

Everything runs locally in your browser with ffmpeg compiled to WebAssembly. The
file never leaves your device.

## Worked example

Take a **4-second** clip of a flag waving that doesn't quite line up when it
repeats. Set **Crossfade** to `0.5` seconds and leave **Quality** at `75`:

- The tool blends the last `0.5 s` of the clip over its first `0.5 s`.
- It returns a **3.5-second** MP4 (input length minus the crossfade).
- That output's first frame matches the source frame half a second before the
  end — so when it loops, the motion is continuous and you can't see the seam.

Want a softer transition? Try the **Long dissolve (1 s)** preset. Want a barely-there
join? The **Subtle blend (0.3 s)** preset keeps almost all of the clip.

## How it works

The tool splits the clip in two, fades the **tail** copy out over the crossfade
window, and **overlays** it onto the **head** copy — a straight alpha crossfade
(the same "crossfade" every seamless-loop tool advertises). It locates the clip's
end without ever measuring the clip's duration, so it behaves identically here and
on the command line. The result is re-encoded to universally-playable H.264 /
`yuv420p` MP4 with `+faststart`.

## Tips

- **Use a short clip.** A few seconds is ideal. The clip is buffered in memory to
  reverse it, so very long or very high-resolution inputs can be slow or run out
  of browser memory.
- **Keep the crossfade well below the clip length.** A 1-second crossfade on a
  2-second clip leaves very little untouched footage.
- **Already trimmed to the loop region?** If not, cut the exact section you want
  to loop first with the video-trim tool, then run it through here.
- **Need it to play for longer?** This makes one seamless clip; to repeat that
  clip N times or fill a target length, run the result through the loop-video tool.

## FAQ

<details>
<summary>Why is the output shorter than my input?</summary>

Because the loop is made by **overlapping** the tail onto the head instead of
adding new footage. If you ask for a `0.5 s` crossfade, the final half-second is
blended into the opening half-second, so the returned clip is `0.5 s` shorter
than the source. That overlap is exactly what removes the visible jump at the
loop point.

</details>

<details>
<summary>Does the output keep its audio?</summary>

No — the output is **silent**. Making a video loop cleanly and crossfading its
audio are separate problems, and blending audio blind (without first probing
whether the clip even has a sound track) can break the render, so this tool
drops audio entirely. That's ideal for muted background loops. If you need sound,
add or crossfade a matching audio track separately.

</details>

<details>
<summary>What crossfade length should I pick?</summary>

Start at **0.5 seconds** (the default) and adjust: shorter (down to `0.1 s`)
keeps more of the original and suits fast motion; longer (up to `5 s`) gives a
smoother, more dreamlike dissolve but consumes more of the clip. The three preset
chips — subtle, standard, long — cover the common choices in one click.

</details>

<details>
<summary>Is there a size or length limit?</summary>

The input and the output are each capped at **10 MB**, and the tool is built for
**short clips** — the whole clip is buffered so it can be reversed, so long or
high-resolution videos can exhaust browser memory. If your clip is large, trim it
to just the section you want to loop first, then run it through here.

</details>
