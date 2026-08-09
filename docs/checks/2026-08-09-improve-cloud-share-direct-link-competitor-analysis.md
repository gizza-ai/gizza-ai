# cloud-share-direct-link — competitor analysis (2026-08-09)

Scan run BEFORE implementation, per `/create-next-tool` step 4. All notes are **paraphrased**
observations of behaviour and feature surface. No competitor copy, branding, wording, or
trademark text was copied into this repo; all page copy for this tool is original.

## Search

One web search for the tool's function: *"direct download link generator Dropbox OneDrive Google
Drive Nextcloud share link converter"*. The result set is dominated by three shapes:
a Chrome extension, a handful of single-box "paste link → Generate" web tools, and how-to
articles that publish the rewrite rules themselves. Notably, **none of the top results advertise
Nextcloud/ownCloud support** — that is the clearest differentiation opening.

## Competitors profiled

### 1. DigitBin — "Get Direct Download Link" (how-to + rules article)
- **Providers:** Google Drive, Dropbox, OneDrive, Box (article also indexes pCloud in its title
  but does not document a rule for it).
- **Rules published:**
  - Google Drive — rewrite `file/d/<id>/view?usp=sharing` into an `uc?id=<id>&export=download`
    form.
  - Dropbox — swap the `www.dropbox.com` host for the `dl.dropboxusercontent.com` content host,
    leaving the rest of the path intact.
  - OneDrive — replace whatever follows `?` with a `download=1` flag (or append it to the existing
    query).
  - Box — reassemble as an `index.php?rm=box_download_shared_file&shared_name=…&file_id=f_…`
    query, extracting the shared-name token and the numeric file id from the share URL.
- **Honesty note worth copying as a *practice*, not as text:** it openly flags the OneDrive rule as
  unreliable in their own testing. Our page states the same class of caveat in its own words.
- **UX:** none — it is an article, the "tool" is manual string editing.

### 2. SyncWithTech — "Direct Download Link Generator"
- **Providers:** Google Drive primarily; Dropbox named but undocumented.
- **Input:** single text box, one share link, obtained via Drive's *Get Link*.
- **Output:** a link that bypasses the Drive interstitial page.
- **Differentiating capability:** for Google **Docs/Sheets/Slides** editor links it emits an
  `export`-style link whose format token the user can swap (`pdf` → `doc`/`xlsx`/`pptx`). This is a
  real capability, not copy — treated as a table stake below.
- **Controls:** text field, Generate button, clipboard copy, and a bulk path via a separate Sheets
  add-on (out of model for us — that is an account-bound server integration).
- **Stated limit:** Drive's >100 MB virus-scan interstitial requires an extra confirm click.

### 3. AirForm — "Dropbox Download Link Generator" (plus per-provider sibling pages)
- **Providers:** Dropbox, Google Drive, OneDrive, one landing page each.
- **Input:** one "paste your share link" field.
- **Controls:** a single *Generate* button. **No options at all** — no provider select, no
  download-vs-embed choice, no batch.
- **Stated limits:** the generated link only works if the share's permissions already allow it;
  and it warns that image files behave badly through the OneDrive path.
- **Rules:** not disclosed.

### 4. (replacement for an unreachable entry)
`ezytoolz.com/utility/direct-link-generator/` was in the result set but served only a category
index — no tool, no documented behaviour. Per the scan rule it was **replaced**, by the
`Download-Link-Generator` Chrome extension (open-source, Chrome Web Store listed): Dropbox +
Google Drive + OneDrive, one-click copy of the generated link, dark mode. Its README documents no
rewrite rules or options; the feature surface is "extract link from the page you're on, copy it".

## Table stakes — every one lands in the descriptor or the out-of-model list

| # | Table stake (from the scan) | In/out of model | Where it lands |
|---|---|---|---|
| 1 | Google Drive share link → direct download | in | `provider=google_drive`, `mode=download` |
| 2 | Dropbox share link → direct download | in | `provider=dropbox`, `mode=download` |
| 3 | OneDrive share link → direct download | in | `provider=onedrive`, `mode=download` |
| 4 | Auto-detect the provider from the URL (no picker needed) | in | `provider=auto` (**default**) |
| 5 | Manual provider override when detection is ambiguous | in | `provider` enum |
| 6 | Inline/hotlink ("embed this image") link, distinct from a download link | in | `mode=inline` |
| 7 | Google Docs/Sheets/Slides export links with a selectable format | in | `docs_export` = `pdf`/`office`/`txt` |
| 8 | Large Drive files skipping the virus-scan interstitial | in | default Drive download URL uses the `usercontent` download host with the confirm token |
| 9 | Copy the result in one click | in | shipped by the shared page chrome (Copy result button) |
| 10 | Stated caveat that the share's permissions must already be public | in | page copy + FAQ |
| 11 | Stated caveat that OneDrive rewriting is the least reliable of the four | in | page copy + FAQ + `onedrive_style` giving the user both known methods |
| 12 | Batch / bulk conversion of many links | in | `per_line` (**default true**) — competitors offer this only via a server-side add-on |
| 13 | Bulk conversion driven from a spreadsheet add-on / account | **out** | needs an account + a server integration; listed, not built |
| 14 | Reading the file's real name/size by calling the provider | **out** | needs a network call + often auth; this tool is pure string rewriting, offline |
| 15 | Box / pCloud / MediaFire providers | **out (scope)** | the backlog row names four providers; Box's rule needs two tokens parsed out of a URL shape we cannot test. Listed as not-supported on the page rather than guessed at |
| 16 | Browser-extension "convert the page I'm on" | **out** | this repo ships wasm tools, not extensions |

## Gaps we close that no scanned competitor covers

- **Nextcloud / ownCloud** public share links (`/s/<token>` and `/index.php/s/<token>`), including
  addressing a single file **inside** a shared folder via the `path`/`files` query pair. Absent
  from every competitor found.
- **Ready-to-run output forms** beyond a bare URL: Markdown link, HTML anchor, `curl`, and `wget`
  command lines (`output` param). Competitors stop at "here is a URL, press copy".
- **Batch by default** — paste a column of links, get a column back, blank lines preserved.
- **Explicit failure instead of a silently-wrong URL**: a Google Drive *folder* link, or a URL from
  no recognised provider, returns an error naming what was expected. Competitors happily emit a
  URL that 404s.

## Decisions

- Default `mode=download`, `provider=auto`, `per_line=true`, `output=url` — the paste-and-go path
  matches the one-box competitors with zero configuration, and every extra capability is opt-in.
- `onedrive_style` defaults to the shares/content API form (base64url-encoded share URL) rather
  than the `download=1` query flag, because the flag is the method every source describes as
  flaky. The flag remains available as the second enum value.
- Box/pCloud/MediaFire deliberately not guessed at (item 15); the page names the four supported
  providers so a user is never left wondering why their link failed.
