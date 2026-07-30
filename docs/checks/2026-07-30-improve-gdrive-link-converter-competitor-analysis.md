# gdrive-link-converter — competitor analysis (2026-07-30)

Scan of the top Google Drive "direct link / download link" tools. All notes are
**paraphrased** observations of publicly visible behaviour — no competitor copy,
branding, or trademarked text is reproduced. Used only to find capability/UX/SEO
gaps that fit gizza's browser-local, wasm, no-account model.

## Competitors surveyed

1. **gdocs2direct** — the classic single-purpose "share link → download link"
   page. Accepts only `file/d/ID/view`; emits `uc?export=download&id=ID`. No
   embed/thumbnail/batch/ID. FAQ centres on making Drive files download instead
   of preview, and that native Google Docs aren't uploaded files.
2. **directlinkgenerator.com** — batch of up to 10 links, clipboard paste/copy;
   claims broad input tolerance (`file/d/`, `open?id=`, `uc?id=`,
   `uc?export=download&id=`, drive.google.com + docs.google.com). Download-only.
3. **SudoMock — Google Drive direct link** — most modern; uses the newer
   `drive.usercontent.google.com/download?id=ID&export=download` host and a
   `…/uc?id=ID&export=view` embed/preview URL. Admits 100 MB+ files hit a
   virus-scan page that URL edits can't fully bypass. No batch/thumbnail/folder.
4. **TemplateRadar — Drive image URL generator** — image/embed specialist:
   direct image URL + resizable thumbnail (`=w{w}-h{h}`), alt-text field, and a
   ready `<img>` HTML snippet. Uses the `lh3.googleusercontent.com/d/ID` host.
   Images only.
5. **Protoolio — direct link generator** — thin single-file download-only page in
   a multi-tool suite; accepts `file/d/ID/view`, no options.

Runner-ups (thin/blog): sheetany.com, labnol.org — both about embedding Drive
images; labnol still recommends the legacy `uc?id=ID` `<img>` form.

## Canonical URL templates (confirmed current)

| Purpose | Template |
|---|---|
| Direct download (small) | `https://drive.google.com/uc?export=download&id=ID` |
| Direct download (large, skip scan) | `https://drive.usercontent.google.com/download?id=ID&export=download&confirm=t` |
| Inline image embed | `https://drive.google.com/uc?export=view&id=ID` (legacy but widely used) / `https://lh3.googleusercontent.com/d/ID` |
| Thumbnail (sized) | `https://drive.google.com/thumbnail?id=ID&sz=w1000` |
| Preview iframe | `https://drive.google.com/file/d/ID/preview` |
| Share / view (input shape) | `https://drive.google.com/file/d/ID/view?usp=sharing` |

## Gap analysis vs our tool

Every competitor is either download-only or image-only. Our single `output` enum
already leapfrogs all five by emitting **seven** forms from one paste.

| Gap | Status in our build |
|---|---|
| Multiple output forms from one link | **Built** — `output` = direct / direct_confirm / view / share / preview / thumbnail / id |
| Broadest input parsing (file/d, open?id, uc, usercontent, docs, folders, bare ID) | **Built** — `extract_id` covers all, with unit tests per shape |
| Large-file / virus-scan handling | **Built** — `direct_confirm` uses `drive.usercontent…&confirm=t`; FAQ explains the 100 MB scan page in plain language |
| Reverse conversion (download/id → share link) | **Built** — `output=share` |
| Thumbnail with size control | **Built** — `size` token (w/h/s syntax) with `w1000` default + preset example chips (w500) |
| Extract file ID for scripts | **Built** — `output=id` |
| Batch of links | **Built** — `per_line=true`, no 10-link cap |
| FAQ/SEO on the recurring pain points | **Built** — page FAQ covers scan warning, image embed, permissions, native-Docs export, batch |

### Considered, not built (out-of-model or rejected)

- **Copy-as-HTML / Markdown / BBCode / iframe snippets with alt text** (TemplateRadar):
  a nice convenience but it's presentation sugar over the URL we already emit; the
  page's Copy button already yields the raw URL. Rejected to keep one clean output
  string per conversion rather than a multi-field panel; users paste the URL into
  their own `<img>`/`![]()`. Could be a future page-only enhancement.
- **`lh3.googleusercontent.com/d/ID` image host** as an alternative embed form:
  considered; kept the official `drive.google.com` `uc?export=view` and
  `thumbnail` hosts, which are stable and self-documenting. The lh3 host is
  undocumented and rate-limited.
- **All-outputs-at-once panel** (show all 7 forms simultaneously): rejected — the
  enum + one-click example chips give the same reach without a bespoke multi-output
  page layout, and keep the CLI/chat surface a single string.
- **Clipboard auto-paste, server batch, accounts**: out-of-model (no backend, no
  account) or already covered by the platform Copy/Reset buttons.
