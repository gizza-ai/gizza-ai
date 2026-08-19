## About this tool

Peak Detector finds local maxima and local minima in a one-dimensional signal. Paste a row or column of numbers and it reports each peak's 0-based index, value, prominence, width, bases, and neighbouring-drop measurements. It is useful for quick checks of time-series data, sensor traces, chromatography-like curves, benchmark samples, traffic series, or any numeric sequence where you need to identify events rather than sort or smooth the data by hand.

The defaults intentionally behave like a simple local-extrema finder: a maximum is higher than both neighbours, a minimum is lower than both neighbours, the first and last samples are not considered peaks, and equal flat tops collapse to the middle sample. Add filters when real data is noisy: `threshold` checks the immediate neighbour drop, `min_prominence` checks whether the peak stands out from the wider curve, `min_distance` keeps one peak per event cluster, `min_width` rejects needle spikes, and `smooth` applies a centred moving average before detection.

### Worked example

Input:

```text
0, 1, 0, 3, 0, 2, 0
```

With `mode=maxima`, the report finds three peaks at indices 1, 3, and 5. The highest and most prominent is index 3 with value `3`.

```text
kind,index,value,prominence,width,left_base,right_base
maximum,1,1,1,...
maximum,3,3,3,...
maximum,5,2,2,...
```

Use `format=csv` when you want to paste the peak table into a spreadsheet, and `format=json` when another script or agent should consume the full measurements.

### Limits and edge cases

- Accepts up to 20,000 samples per run.
- Input numbers may be integers, decimals, negatives, or scientific notation such as `1e-3`.
- Non-numeric tokens such as a pasted header are skipped and counted.
- `smooth` must be odd (or 0/1 for off) so the moving-average window stays centred.
- `rel_height` must be between 0 and 1; the default `0.5` measures width at half prominence.
- `max_peaks=0` means report every matching peak; higher values cap the sorted result list.

## FAQ

<details>
<summary>What is the difference between threshold and prominence?</summary>

`threshold` is local: it requires the peak to rise above the two samples touching it by at least the requested amount. `min_prominence` is broader: it looks outward until the signal climbs back above the peak or reaches an edge, then measures how far the peak stands above its surrounding bases. Prominence is usually better for noisy real signals.

</details>

<details>
<summary>Can this find valleys as well as peaks?</summary>

Yes. Set `mode=minima` to find local minima only, or `mode=both` to return maxima and minima in one report. For minima, prominence and width are measured on the inverted signal, so the same filters work for valleys.

</details>

<details>
<summary>How should I choose min_distance?</summary>

Use `min_distance` when one physical event creates several nearby candidate peaks. The detector keeps the most extreme candidate first, then drops other peaks of the same kind that are closer than the requested number of samples.

</details>

<details>
<summary>Does smoothing change the reported values?</summary>

Yes. Smoothing is applied before detection, and reported peak values come from the smoothed signal. Keep `smooth=0` when exact original sample heights matter; use an odd window such as 3, 5, or 7 when noise would otherwise create many tiny peaks.

</details>
