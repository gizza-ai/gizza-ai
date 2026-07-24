# mbox-dedup — competitor analysis (2026-07-23)

Scope: a browser-local, no-upload tool that removes duplicate messages from a pasted mbox
by their **Message-ID** header and returns the de-duplicated mbox plus kept/removed counts.
All notes below are paraphrased from public sources — no competitor copy, branding, or
trademarks are reproduced.

## Competitors scanned

1. **Jachimo/mbox_dedupe** (open-source CLI script). Deduplicates *purely* on the
   `Message-ID` header — "if the ID is the same, the rest of the message probably is too."
   No content analysis, timestamps, or hashing. Requires valid Message-IDs (messages
   lacking one — e.g. drafts — are not handled well). Modifies the mbox in place (backup
   warning). Retention (first vs last) and case handling are unspecified.

2. **mail-deduplicate** (`mail-deduplicate` on PyPI, open-source CLI). Reads/writes mbox,
   maildir, babyl, mh, mmdf. Detects duplicates on *cherry-picked, normalized* mail headers,
   with selection strategies to keep/discard by size, content, timestamp (oldest/newest),
   file path, or random, and an action to copy/move/delete the chosen set. Emphasizes
   false-positive protection via size/content safety checks.

3. **Commercial MBOX "duplicate remover" GUIs** (RecoveryTools / SysCurve / SysTools family,
   Windows desktop). Match on a configurable combination of Internet Message-ID, full RFC
   headers, sender/recipient, subject, date, normalized body text, MIME structure, and
   attachment digests. Preview duplicates before removal; write a *new* mbox (originals left
   intact); generate a log report; batch multiple files; two modes (within vs across files).

Also noted: procmail/mutt can drop duplicates by Message-ID; IMAPdedup does the same over IMAP.

## Table-stakes → decision

| Capability (from competitors)                        | In model? | Decision |
|------------------------------------------------------|-----------|----------|
| Match/dedupe by Message-ID header                    | in-model  | **Core.** Split mbox on `From ` postmarks, key each message on its Message-ID. |
| Keep first vs last occurrence                        | in-model  | **`keep` = first \| last** (default first). Order preserved. |
| Normalize Message-ID before compare (strip `<>`/ws)  | in-model  | **Always applied** — trim whitespace and one surrounding `<…>` pair before comparing. |
| Case sensitivity of Message-ID                       | in-model  | **`ignore_case` boolean** (default false — RFC 5322 Message-IDs are case-sensitive). |
| Handling messages that lack a Message-ID             | in-model  | **`no_message_id` = keep \| drop** (default keep — never merge distinct ID-less messages). |
| Preview / count of duplicates found                  | in-model  | **Counts** (`total`/`kept`/`removed` + `messages_without_id`) returned to chat/CLI; the page shows the deduped mbox. |
| Preserve originals / output a fresh mbox             | in-model  | **Inherent** — input is never mutated; output is a new deduped mbox string. |
| Match on subject + date + sender combination         | out-of-model (slug is "by Message-ID") | Listed, not built — the tool's defined key is Message-ID. |
| Body-hash / MIME / attachment-digest matching        | out-of-model | Listed, not built — heavier fuzzy matching beyond this slug's scope. |
| Batch of multiple mbox files, folder structure       | out-of-model | Single pasted/typed mbox; no server-side batch or filesystem. |
| Other mailbox formats (maildir, mh, mmdf, babyl)     | out-of-model | mbox in / mbox out only. |
| In-place file modification                            | out-of-model | Browser-local; we return text, never touch a file. |

## UX patterns adopted

- **Preset example chips** for the common flows (keep first, keep last, drop ID-less).
- Friendly `<select>` labels for `keep` and `no_message_id`.
- Multiline paste area for the mbox; deduped mbox is downloadable (page `format = "text"`).
- Worked example + FAQ covering Message-ID normalization, ID-less messages, and ordering.
- Errors state what was expected (e.g. no parseable messages found).

Nothing above copies competitor text or assets; original copy/design only.
