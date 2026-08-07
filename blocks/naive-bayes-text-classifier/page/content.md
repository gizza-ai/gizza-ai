## About this tool

Naive Bayes Text Classifier trains a small supervised text model from the examples you paste, then immediately classifies new text in the browser or CLI. Each training line is `label<separator>text`, such as `spam,win a free prize now`. The separator can be auto-detected from tab, comma, pipe, or colon, or forced when your examples need it.

The default multinomial model is the classic bag-of-words baseline for spam, topic, ticket, and short-message classification. Bernoulli mode uses token presence/absence, and complement mode is useful when one label has many more examples than another. Controls cover smoothing alpha, n-grams, case folding, English stop-word removal, rare-token pruning, class priors, class-list length, explanations, and report vs JSON output.

### Worked example

Training data:

```text
spam,win a free prize now
spam,free money click here
spam,claim your free gift
ham,meeting at ten tomorrow
ham,lunch with the team
ham,project update attached
```

Text to classify:

```text
claim your free money now
```

With default settings, the model predicts `spam`, lists the class probabilities, and shows the tokens that pushed the decision toward spam over the runner-up. Switch `input_mode=lines` to classify each non-blank input line as a separate document.

### Limits and edge cases

- Training data is capped at 1 MiB, 20,000 examples, 200 labels, and 200,000 vocabulary tokens after `min_count` filtering.
- Text to classify is capped at 256 KiB; batch mode accepts up to 1,000 non-blank lines.
- `ngram_max` is limited to 3 so vocabulary growth stays predictable in WASM.
- A single label is rejected; the model needs at least two distinct labels.
- Unseen input tokens are ignored. If none of the input tokens appeared in training, the prediction comes from class priors alone and the report says so.
- This is a stateless local baseline, not a persisted ML project. It does not save models, run validation splits, or download pre-trained language models.

## FAQ

<details>
<summary>How should I format the training data?</summary>

Use one labeled example per line: `label,text`. The first separator on the line splits the label from the example text, so the text may contain more commas after that. Tabs, pipes, and colons are also supported; choose the separator explicitly if auto-detection guesses wrong.

</details>

<details>
<summary>Which model variant should I choose?</summary>

Start with `multinomial` for ordinary word-count text classification. Use `bernoulli` for very short texts where word presence matters more than repeated words. Try `complement` when the labels are imbalanced and the largest class otherwise dominates too easily.

</details>

<details>
<summary>What does alpha smoothing do?</summary>

`alpha` adds a small count to every token/class pair so a word that was unseen for one label does not make that label impossible. The default `1` is Laplace smoothing. Lower values make the classifier more confident in the pasted examples; higher values flatten the probabilities.

</details>

<details>
<summary>Can I reuse or export the trained model?</summary>

No. The tool trains from scratch on every run and returns only the prediction/report. That keeps the tool stateless and local. If you need persisted models, cross-validation, or deployment, use a dedicated machine-learning workflow after validating the idea here.

</details>
