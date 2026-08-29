## About this tool

`shopping-list-builder` turns pasted recipe ingredient lists into one grocery list. Add recipe headers such as `# Pancakes x2`, separate recipes with `---`, and paste one ingredient per line. The tool scales quantities, merges repeated ingredients, sums compatible units, and groups the result into broad store categories.

Worked example:

```text
# Pancakes x2
1 cup flour
1 cup milk
2 eggs
---
# Sauce
1/2 cup milk
1 tbsp sugar
```

With the defaults, the output includes `2.5 cup milk`, `2 cup flour`, `4 egg`, and `1 tbsp sugar` under grocery category headings. Switch **Unit system** to metric when you want weights and volumes rendered as grams, kilograms, millilitres, or litres; switch **Output format** to CSV or JSON for spreadsheets and automations.

## Limits and edge cases

- Input is capped at 200,000 characters and 5,000 lines. The merged list is capped at 2,000 distinct items.
- `scale` must be between 0.1 and 20. Recipe headers can add their own multiplier, for example `# Curry x3`.
- Volume units are summed with volume units and weight units with weight units, but the tool never converts volume to weight. `1 cup flour` and `120 g flour` stay separate because that conversion depends on ingredient density.
- Ranges such as `3-4 cloves garlic` use the upper bound so the shopping list buys enough.
- Category grouping is keyword-based with an **Other** fallback; store-specific aisle order is out of scope.

## FAQ

<details>
<summary>Can it fetch recipe URLs for me?</summary>

No. This tool is a local ingredient-list aggregator. Use a recipe extraction tool first if you need to turn a recipe page into text, then paste the ingredient lines here.

</details>

<details>
<summary>Why did two spellings not merge?</summary>

The merge key is intentionally conservative: it lowercases names, trims punctuation and folds simple plurals. It does not guess that `caster sugar`, `white sugar`, and `sugar` are always interchangeable. Normalize names before pasting when you want them combined.

</details>

<details>
<summary>What happens to pantry staples?</summary>

Nothing is removed by default. Add staples such as `salt, pepper, water, olive oil` to **Pantry staples to skip** when you already have them and do not want them on the final list.

</details>

<details>
<summary>Does metric mode convert cups of flour into grams?</summary>

No. Metric mode converts within the same measurement family: ounces to grams, pounds to kilograms, cups to millilitres or litres. It does not use density tables, so volume-to-weight conversions are listed as an edge case instead of guessed.

</details>
