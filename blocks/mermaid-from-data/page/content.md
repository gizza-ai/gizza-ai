## About this tool

Mermaid from data turns the rows you already have into paste-ready Mermaid diagram source. Use arrow lines like `Start -> Build : work`, delimited edge rows like `from,to,label,style`, or chain rows like `Draft,Review,Ship`. Optional node rows add friendly labels, flowchart shapes, subgraph groups, class members or ER attributes.

The output is plain Mermaid text, so you can paste it into Markdown, docs sites, issue trackers or any renderer that supports Mermaid. The generator does not render images; it focuses on producing readable, deterministic source that can be reviewed and version-controlled.

Example: a short left-to-right flowchart from arrow lines:

```bash
gizza tool mermaid-from-data edges="Start -> Build : implement
Build -> Ship : deploy" direction=LR title="Release pipeline"
```

For tabular input, use `row_mode=pair` when each row is `from,to,label,style`, or `row_mode=chain` when each row is a path and adjacent columns should be linked.

## Limits and edge cases

- Up to 500 nodes, 1000 edges and 2 MB each for edges and node declarations.
- Flowcharts support node shapes, subgraph groups and arrow/open/dotted/thick/bidirectional edge styles.
- Class diagrams map relationship styles such as inheritance, composition, aggregation and dependency to Mermaid operators.
- ER diagrams map common cardinality names such as one-to-many and one-to-one-or-more.
- Node identifiers are sanitized for Mermaid while labels preserve the original text.

## FAQ

<details>
<summary>Can I paste CSV directly?</summary>

Yes. Set delimiter to comma or leave it on auto for simple CSV-style rows. A header row such as `from,to,label,style` is skipped automatically, and quoted fields keep commas inside the label.

</details>

<details>
<summary>What is the difference between pair mode and chain mode?</summary>

Pair mode treats each row as one edge: source, target, optional label and optional style. Chain mode treats each row as a path, so `Draft,Review,Publish` becomes `Draft --> Review` and `Review --> Publish`.

</details>

<details>
<summary>Does this validate Mermaid rendering?</summary>

It validates the structured data and emits Mermaid source, but it does not run a browser renderer or catch every Mermaid syntax rule. Paste the output into your Mermaid renderer for final visual layout checks.

</details>

<details>
<summary>How do I add class members or ER attributes?</summary>

Use the node declarations field. For a class diagram, write rows such as `Animal | +String name; +eat()`. For an ER diagram, write rows such as `CUSTOMER | string name PK; string email`.

</details>
