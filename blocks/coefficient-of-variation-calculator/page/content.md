## About this tool

The coefficient of variation (CV) is the standard deviation divided by the mean. Because it is unitless, it helps compare relative dispersion across datasets that live on different scales: a two-kilogram spread means something different for kittens than it does for oxen. This calculator reports CV as both a ratio and a percentage, shows the supporting n, mean, standard deviation, sum of squares, and min/max values, then ranks datasets by relative spread.

Paste raw values as one dataset per line. A leading `Label:` names the row, so `Machine A: 4.2 5.1 4.8` appears in the report as Machine A. If you already have a mean and standard deviation, leave Data empty and fill the known summary fields instead.

### Worked example

Input:

```text
Kittens: 4.2 5.1 4.8 5.6 5.1
Oxen: 792 800 803 795 797
```

With the sample basis, the oxen row has the lower CV and is ranked most consistent. The kitten row has a larger relative spread even though its absolute standard deviation is smaller, which is exactly the kind of comparison CV is meant to make.

### Limits and caveats

CV is undefined when the mean is zero and unstable when the mean is very close to zero. It is meaningful on ratio scales with a true zero; temperatures in Celsius, scores that can cross zero, or mixed positive and negative data need extra care. The interpretation bands in the output are rules of thumb, not significance tests. The browser page caps a run at 200,000 numeric values across at most 1,000 datasets.

## FAQ

<details>
<summary>Should I use sample or population standard deviation?</summary>

Use **sample (n-1)** when your values are observations from a larger process, which is the common spreadsheet and textbook case. Use **population (N)** only when the values are the entire group you care about. A single value has no sample standard deviation, so switch to population if you really need a one-reading CV.

</details>

<details>
<summary>Why can the CV be undefined or very large?</summary>

CV divides by the mean. If the mean is zero, there is no defined result. If the mean is only a tiny distance from zero, even a modest standard deviation can create a very large or sign-flipping CV. The tool reports those cases in the Notes section instead of hiding the caveat.

</details>

<details>
<summary>How do I compare several datasets?</summary>

Put one dataset on each line, optionally with a `Label:` prefix. The report sorts datasets by absolute CV from lowest relative spread to highest, then names the most consistent and most variable rows. Use `grouping = single` only when multiple rows should be pooled into one dataset.

</details>

<details>
<summary>What does the outlier checkbox do?</summary>

It applies Tukey's 1.5×IQR fence within each dataset before computing the statistics. Any removed values are listed in the report so the run remains auditable. For datasets with fewer than four values, the filter is skipped because quartiles are not stable enough to support the fence.

</details>
