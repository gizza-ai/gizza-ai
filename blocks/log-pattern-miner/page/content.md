## About this tool

**Log Pattern Miner** turns a noisy batch of raw log lines into the small set of
message shapes that actually repeat. It is a deterministic, Drain-style template
miner: split each line into tokens, mask values that usually change, then merge
similar token sequences into ranked templates with occurrence counts.

Use it when you have thousands of lines and want to answer:

- Which messages dominate this file?
- Are one-off errors hiding behind repeated noise?
- What changed between deployments, hosts or time windows?
- Which fields are variable inside a recurring message?

### Worked example

Input:

```text
Jan 12 03:04:05 web1 sshd[2311]: Failed password for root from 10.0.0.1 port 51234 ssh2
Jan 12 03:04:07 web1 sshd[2312]: Failed password for admin from 10.0.0.2 port 51999 ssh2
Jan 12 03:04:09 web1 sshd[2313]: Failed password for root from 10.0.0.3 port 52001 ssh2
Jan 12 03:05:00 web1 sshd[2400]: Accepted publickey for deploy from 10.0.0.9 port 60001 ssh2
```

Table output:

```text
count	percent	first	last	template
3	75	1	3	Jan <NUM> <TIME> web1 sshd[<NUM>]: Failed password for <*> from <IP> port <NUM> ssh2
1	25	4	4	Jan <NUM> <TIME> web1 sshd[<NUM>]: Accepted publickey for deploy from <IP> port <NUM> ssh2
```

The first row covers three similar failed-login lines. The username position
varies (`root`, `admin`), so it becomes `<*>`; dates, times, process IDs, IPs and
ports become typed placeholders.

### What gets masked

With the default **Typed** placeholders, the miner recognises:

- numbers and units: `250ms`, `1,024`, `80%`
- IPv4/IPv6 addresses, MAC addresses and UUIDs
- hex IDs and `0x...` values
- dates, timestamps and clock times
- file paths, URLs, e-mail addresses and quoted strings

Choose **Wildcard** to render every masked value as `<*>`, or **None** to keep
literal values and let only the clustering merge create wildcards.

### Tuning knobs

- **Similarity threshold** controls how aggressively lines merge. `0.4` is the
  reference Drain default; increase it to keep near-neighbours apart.
- **Parse-tree depth** controls how many leading tokens shape the search path.
  Deeper trees are stricter; shallower trees merge more.
- **Max branches** limits high-cardinality tree nodes before extra branches fall
  into a shared wildcard bucket.
- **Minimum lines per template** hides one-off messages.
- **Extra delimiters** split tokens on characters such as `=` so `status=500`
  becomes `status <NUM>`.
- **Leading tokens to drop** removes fixed prefixes such as dates before mining.

### Limits and edge cases

- Maximum input: **2,000,000 characters** and **200,000 lines** per run.
- This is one-shot batch mining. It does not persist cluster IDs or update a
  tree incrementally across runs.
- Masking uses a built-in placeholder set, not custom regex rules.
- The miner tokenizes on whitespace plus optional single-character delimiters; it
  is not a full parser for every log format.
- Very low similarity can over-merge unrelated messages; very high similarity can
  leave too many near-duplicate templates.

Everything runs locally in WebAssembly. No log lines are uploaded.

Also available from the gizza CLI and in chat.

## FAQ

<details>
<summary>How is this different from simple duplicate-line counting?</summary>

Duplicate counting only groups byte-identical lines. This tool masks fields that
normally change — request IDs, IP addresses, durations, timestamps, paths — and
then clusters similar token sequences, so `worker 17 finished in 250ms` and
`worker 42 finished in 311ms` become one template with a count of two.

</details>

<details>
<summary>What does “Drain-style” mean here?</summary>

Drain is a common deterministic log-template mining approach. Lines are routed
through a fixed-depth parse tree by token count and leading tokens; each leaf
contains candidate templates. A line joins the most similar template in its leaf
when the similarity threshold is met, and mismatched positions become wildcards.
This implementation follows that shape but stays dependency-free and browser-safe.

</details>

<details>
<summary>When should I change the similarity threshold?</summary>

Use the default `0.4` first. Raise it toward `0.7` or `0.9` when unrelated
messages are merging into broad templates. Lower it toward `0.2` or `0.3` when
one message family is splitting into many near-duplicates because several words
change together.

</details>

<details>
<summary>Why do my timestamps still influence the result?</summary>

Typed masking turns timestamps into `<DATE>` and `<TIME>`, but those tokens still
exist and may be part of the parse-tree prefix. If every line begins with a fixed
prefix such as `date time level`, set **Leading tokens to drop** to remove it, or
use a shallower **Parse-tree depth**.

</details>

<details>
<summary>Can I use custom regular expressions for masking?</summary>

No. Custom regex masking is deliberately out of scope for this browser-safe tool.
Use **Extra delimiters**, **Leading tokens to drop**, and the built-in typed
placeholder set for common log values. If you need a highly specialised parser,
pre-normalise the logs before pasting them here.

</details>
