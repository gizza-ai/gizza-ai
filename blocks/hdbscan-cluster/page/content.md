## About this tool

HDBSCAN clusters rows by density instead of asking for the number of clusters up front. Paste a CSV, TSV, semicolon, pipe, or whitespace-delimited table and the tool builds an exact mutual-reachability minimum spanning tree, condenses the cluster hierarchy, and selects stable groups. Rows that do not belong to a stable dense region are labelled as noise (`-1`).

Use this when DBSCAN's single `eps` radius is awkward: HDBSCAN can keep compact and loose clusters in the same run, then report a membership probability, a GLOSH-style outlier score, and a persistence score for every selected cluster. The implementation is deterministic and local — there is no random seed and no network call.

## Worked example

Paste this table with the defaults:

```csv
x,y
1,1
1,2
2,1
2,2
1.5,1.5
1.2,1.8
10,10
10,11
11,10
11,11
10.5,10.5
10.2,10.8
60,-40
```

The report finds two dense clusters and marks the far-away row as noise. The row table includes the cluster id, membership probability, and outlier score, while the cluster summary lists each cluster's size, persistence, medoid row, and feature means.

For CSV export, set **Output format** to CSV. The original columns are preserved and three columns are appended: `cluster`, `probability`, and `outlier_score`.

## Limits and edge cases

- HDBSCAN is quadratic in the number of clustered rows because this block uses an exact minimum spanning tree. The hard cap is 5,000 data rows and 200 columns.
- `min_samples = 0` mirrors `min_cluster_size`, matching the common reference default. Increase it when you want more conservative clusters and more noise.
- `normalize` is on by default so mixed-unit tables behave sensibly. Turn it off for coordinates or other values where raw distance units already mean the right thing.
- Blank and non-numeric feature cells can be dropped, filled with median/mean/zero, or treated as an error. Dropped rows remain visible in the listing as skipped rows.
- Noise uses `-1`, matching the common Python convention. R users may be used to `0` for noise.
- This page reports labels and scores only. It does not draw a condensed-tree plot, k-distance plot, or PCA scatter.

## FAQ

<details>
<summary>How is this different from DBSCAN?</summary>

DBSCAN needs one global neighbourhood radius (`eps`). HDBSCAN builds a hierarchy over density levels and selects stable branches from that hierarchy, so it can keep clusters at different densities in one run and label uncertain border points as noise.

</details>

<details>
<summary>What should I tune first?</summary>

Start with **Minimum cluster size**. Raising it returns fewer, larger, more persistent clusters; lowering it allows smaller groups. Leave **Minimum samples** at `0` until you have a reason to make the density estimate more conservative.

</details>

<details>
<summary>Why are many rows labelled noise?</summary>

That usually means the density requirement is too strict for the table, the features are on incompatible scales, or the data genuinely lacks dense regions. Try lowering `min_cluster_size`, keeping `normalize` enabled, or using `allow_single_cluster` if one broad population is a valid answer.

</details>

<details>
<summary>Can I use text or category columns?</summary>

Only numeric features are clustered. Leave **Feature columns** blank to use every fully numeric column, or name numeric columns explicitly. Non-feature columns can still be present in the source table and are preserved in CSV output.

</details>
