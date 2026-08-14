## About this tool

Frequent address lists drift. A one-off launch thread can crowd out the person you wrote to every
week, and an old coworker can stay in autocomplete long after you stopped emailing them. This tool
rebuilds that list from a mailbox export: paste an mbox file or a batch of raw email messages, then
rank the people in the headers by both frequency and recency.

The parser reads only the mail headers it needs — `From`, `To`, `Cc`, `Bcc` and `Date`. Message
bodies and attachments are ignored. Addresses are case-folded so `Bob@Example.org` and
`bob@example.org` become one row, while the most common display name is kept for paste-ready output.
Use **Exclude addresses and domains** for your own address and internal/list domains, otherwise your
own address often ranks first.

The default score is recency-weighted: every message contributes
`0.5^(age_days / half_life_days)`, measured from the newest dated message in the paste. That means
the same archive produces the same list next month, and recent conversations rise above ancient
bulk threads. Set the half-life to `0` when you want a pure message-count ranking.

### Worked example

Input:

```text
From 1@x Mon Sep 03 10:00:00 +0000 2018
From: Alice Example <alice@example.com>
To: Bob <bob@example.org>, Carol <carol@example.net>
Date: Mon, 3 Sep 2018 10:00:00 +0000

Hi both.

From 2@x Tue Sep 04 09:30:00 +0000 2018
From: Bob <bob@example.org>
To: alice@example.com
Cc: Dave <dave@example.com>
Date: Tue, 4 Sep 2018 09:30:00 +0000

Sounds good.

From 3@x Wed Sep 05 08:00:00 +0000 2018
From: Alice Example <alice@example.com>
To: Bob <bob@example.org>
Date: Wed, 5 Sep 2018 08:00:00 +0000

Slides attached.
```

With `alice@example.com` excluded, the default report is:

```text
Top 3 of 3 contacts · 3 messages · 2018-09-03 → 2018-09-05 · half-life 180 days, clocked from 2018-09-05

#  contact                    msgs  to  from  last seen   score
1  Bob <bob@example.org>         3   2     1  2018-09-05  100.0
2  Dave <dave@example.com>       1   1     0  2018-09-04   33.3
3  Carol <carol@example.net>     1   1     0  2018-09-03   33.2
```

Switch **Output shape** to **Paste-ready Name <address> lines** when you want a compact address-book
seed list, or to CSV/JSON when you want to audit the counts elsewhere.

### Limits and edge cases

- One run parses at most 5000 messages.
- The tool expects text input: an mbox export or raw RFC 5322 messages. It does not connect to Gmail,
  Outlook, IMAP or a local mailbox database.
- Messages without parseable addresses are skipped. If every address is excluded, automated, or below
  the minimum-message threshold, the run reports that filtering removed everything.
- Dates are used for recency only. Undated messages still count, but they receive no recency boost and
  do not set the date range.
- `skip_automated` removes common machine senders such as `noreply`, `mailer-daemon`, `postmaster`
  and `bounce` addresses. Turn it off when you are intentionally ranking newsletters or alerts.

## FAQ

<details>
<summary>What should I paste into the mailbox box?</summary>

Paste an mbox export from a mail client or service such as Gmail Takeout, Thunderbird or Apple Mail.
A single raw `.eml` message also works, and multiple raw messages can be pasted back to back when
they have standard headers. The tool splits mbox messages on the classic `From ` postmark line at
column 0.

</details>

<details>
<summary>How do I keep my own address out of the ranking?</summary>

Put your address in **Exclude addresses and domains**, for example `me@example.com`. You can also
exclude a whole domain with `@example.com` or `example.com`. Exclusions are useful for dropping your
own account, internal aliases, mailing-list domains or shared helpdesk addresses before scoring.

</details>

<details>
<summary>What does the half-life slider change?</summary>

It controls how quickly old conversations fade. With the default 180-day half-life, a message six
months older than the newest message in the archive counts about half as much as a new one. Lower
values favour very recent contacts; higher values behave more like a lifetime frequency count. Set
it to `0` to disable recency weighting entirely.

</details>

<details>
<summary>Should I rank recipients, senders or both?</summary>

Use **recipients** for an autocomplete list of people you write to. Use **senders** to see who writes
to you most often, or to find noisy newsletters by turning off automated-sender filtering. Use
**both** when you want a general relationship-strength list; the report still shows separate `to`
and `from` counts.

</details>

<details>
<summary>Does this upload or read my real mailbox account?</summary>

No. The page runs the parser in WebAssembly on the text you paste, and the CLI reads only the input
you give it. It does not log in to a mail provider, read contacts from an account, or upload the
mailbox anywhere.

</details>
