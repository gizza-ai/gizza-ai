## About this tool

A mailbox is a social network hiding in plain text. Every message carries a `From:` address and one
or more `To:`/`Cc:` addresses, and that is all you need to reconstruct who talks to whom, how often,
and over what period. This tool reads that metadata out of raw email text and builds the graph.

Paste an mbox export (messages separated by a `From ` postmark line), a single `.eml` file, or even
just the headers. Each message contributes one link from its sender to every distinct recipient,
weighted by how many messages travelled along it. You get message, participant, and link totals, the
date span, the top senders, the top recipients, the top correspondents by combined volume, and the
heaviest links with the first and last date each was used. Set **Your address** and the report adds a
personal section: who you mail most, who mails you most, and a reciprocity ratio.

Switch **Nodes** to domains for an organisation-level rollup, **Link direction** to undirected to
merge `A → B` with `B → A`, or **Output** to CSV, JSON, GraphML, or Graphviz DOT when you want to
load the graph into Gephi, NetworkX, or Graphviz instead of reading it here. Message bodies are never
parsed and nothing leaves the browser.

### Worked example

Paste this three-message mbox:

```text
From alice@example.com Mon Jan  1 00:00:00 2024
From: Alice <alice@example.com>
To: bob@example.com
Cc: carol@example.org
Date: Tue, 2 Jan 2024 10:00:00 +0000
Subject: kickoff

hello

From bob@example.com Mon Jan  1 00:00:00 2024
From: Bob <bob@example.com>
To: alice@example.com
Date: Wed, 3 Jan 2024 09:00:00 +0000
Subject: re: kickoff

ack

From alice@example.com Mon Jan  1 00:00:00 2024
From: Alice <alice@example.com>
To: bob@example.com
Date: Fri, 5 Jan 2024 11:30:00 +0000
Subject: status

update
```

With the defaults (address nodes, To: and Cc:, directed) the report opens with:

```text
Email network
  3 messages, 3 addresses, 3 links
  View: directed graph of addresses, recipients = To+Cc
  Dates: 2024-01-02 .. 2024-01-05
```

Alice leads the top-senders list with 2 messages to 2 contacts, Bob follows with 1, and the heaviest
link is `alice@example.com -> bob@example.com` with 2 messages spanning `2024-01-02 .. 2024-01-05`.
Switching **Link direction** to undirected merges Alice→Bob and Bob→Alice into a single pair link
carrying all 3 messages; switching **Nodes** to domains leaves just `example.com -> example.org`,
because the mail inside `example.com` becomes a self-loop and self-loops are dropped by default.

### Limits and edge cases

- Input is capped at **4 MiB** and **20,000 messages**. This is a paste-sized tool — split a
  multi-gigabyte archive before pasting, or filter it with **From date** / **To date** first.
- **Rows per ranked list** accepts **1–100**; **Minimum messages per link** accepts **1–10,000**.
  Neither affects the CSV, JSON, GraphML, or DOT exports, which always contain the full graph.
- A message needs a `From:` header and at least one recipient address to count. Blocks that fail
  either test are skipped and tallied in the report's **Notes** section.
- mbox splitting keys on a line starting with `From ` (with a space) at column 0. A body line that
  starts exactly like that will split a message in two — rare, but it is how the format works.
- Dates are read from the `Date:` header and compared as calendar days in the message's own
  timezone. While **From date** or **To date** is set, undated messages are skipped, not kept.
- Recipients are de-duplicated per message, so someone listed in both `To:` and `Cc:` counts once.
- `Bcc:` normally survives only in your own sent mail; on received mail `to-cc-bcc` behaves like
  `to-cc`.
- Only addresses are read — no subjects, bodies, attachments, or `In-Reply-To` threading. Centrality
  measures such as betweenness are out of scope; export GraphML and compute them in Gephi or
  NetworkX.

## FAQ

<details>
<summary>Where do I get an mbox file to paste?</summary>

Most mail clients export one. Google Takeout returns Gmail as an `.mbox`, Thunderbird stores folders
as mbox files under its profile directory, and Apple Mail can export a mailbox to `.mbox`. You can
also paste a single `.eml`, or copy the header block ("show original"/"view source") of one message —
the tool only needs `From:`, `To:`, `Cc:`, `Bcc:`, and `Date:`.

</details>

<details>
<summary>What is the reciprocity ratio in the personal section?</summary>

It is the messages you received divided by the messages you sent, within the analysed set. Above
`1.00` means more mail arrives than you send; below `1.00` means you are the one driving the
conversation. It only appears when **Your address** is set and you sent at least one message.

</details>

<details>
<summary>Why is my inbox showing barely any links?</summary>

An inbox is mostly mail addressed to you, so a directed graph of it is a star with you at the centre.
Two things help: analyse **sent** mail as well so the outbound half exists, and set **Exclude
addresses containing** to `noreply,notifications@,mailer-daemon` so automated senders stop crowding
out real people. Raising **Minimum messages per link** to 3 or 5 also strips one-off contacts.

</details>

<details>
<summary>What is the difference between address nodes and domain nodes?</summary>

Address nodes keep every person separate — the usual view for a personal mailbox. Domain nodes
collapse everything after the `@`, so all of `alice@example.com` and `bob@example.com` become
`example.com`. That gives an organisation-level picture of which companies talk to each other. Note
that internal mail then becomes a self-loop, which is dropped unless you tick **Keep self-addressed
links**.

</details>

<details>
<summary>How do I get this into Gephi or NetworkX?</summary>

Set **Output** to GraphML and save the result as `network.graphml`. Gephi opens it directly; in
Python, `networkx.read_graphml("network.graphml")` gives you a graph whose edges carry a `weight`
attribute (the message count) and whose nodes carry `label`, `sent`, and `received`. The
`edgedefault` attribute follows the **Link direction** setting. The DOT output is the same graph for
Graphviz: `dot -Tsvg network.dot -o network.svg`.

</details>

<details>
<summary>Is my mail uploaded anywhere?</summary>

No. The parser and the graph builder are compiled to WebAssembly and run inside this page, so the
pasted text never leaves your machine. The same Rust code runs locally in the CLI. There is no
account, no mailbox connection, and no server-side storage.

</details>
