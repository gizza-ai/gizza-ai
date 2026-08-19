# email-tracker-pixel-detector — competitor analysis (2026-08-18)

Scan performed **before** implementation, per `create-next-tool` step 4. Everything below is
paraphrased from public product/marketing/documentation pages; **no competitor copy, branding,
wording, or trademarked asset was reused**. Vendor names appear only as factual identifiers of
tracking infrastructure (which is the tool's whole job) and as market references.

## Search + sources

Queries: "email tracking pixel detector tool", "detect tracking pixels in email HTML source known
tracker domains list open tracking".

Competitors reviewed (all reachable):

| # | Competitor | Shape | Notes |
|---|-----------|-------|-------|
| 1 | Trocker (browser extension, Gmail/webmail) | detect + block | reports a per-message count of blocked trackers and the domains that tried to track; covers both pixel trackers and click-tracked links |
| 2 | Ugly Email (browser extension, Gmail) | detect only | marks messages whose markup matches a known-tracker signature and surfaces the **platform name** (ESP/sales tool) behind it |
| 3 | PixelBlock (browser extension, Gmail) | detect + block | binary indicator ("this message contains a tracker"); no vendor attribution, no counts |
| 4 | Gblock article / tool (Gmail) | detect + block | documents the manual raw-HTML heuristics (`track`, `pixel`, `beacon`, `1x1`, `width="1"`, `display:none`) and names vendor host patterns (e.g. HubSpot sidekick hosts, Mandrill `…/track`, Postmark click hosts, Salesforce Marketing Cloud hosts) |
| 5 | Proton Mail enhanced tracking protection / Sender Audit write-up | block + report | shows a per-message count of blocked trackers and cleaned links; the write-up documents the fuller signal set: 1×1 transparent GIFs, `display:none`, CSS `background-image` beacons, `<link rel=prefetch>` beacons, per-recipient unique-ID URLs, ETag/cookie fingerprinting, and link-wrapping redirect hosts; cites a ~400-domain tracker database spanning ESPs, CRMs, analytics, ad networks, shorteners and sales-engagement tools |

Category reality check: **every** competitor is a browser extension or a mail client/provider
feature that hooks a live inbox. There is no established *paste-the-message-in* web tool for this,
which is exactly the niche a browser-local, no-account, no-upload tool fills — and it is the only
shape that works for a raw `.eml` a user exported from any client.

## Table-stakes extracted, and where each landed

| Capability seen in competitors | Verdict | Where it landed |
|---|---|---|
| Flag 1×1 / 0×0 / tiny images | **in-model — built** | `TinyPixel` signal, from `width`/`height` attributes *and* from `style="width:1px"` |
| Flag CSS-hidden images (`display:none`, `visibility:hidden`, `opacity:0`, off-screen position) | **in-model — built** | `Hidden` signal |
| Identify the tracking **vendor/platform** by domain (Ugly Email's headline feature) | **in-model — built** | 90+ built-in host→vendor table (ESPs, CRMs, sales-engagement, analytics, ad networks, newsletter platforms), matched on registrable-domain suffix; reported per pixel and rolled up in the header |
| User-extendable tracker domain list | **in-model — built** | `vendors` param (comma/space/newline separated; tag-list control on the page) |
| Detect open-tracking **URL path/query** patterns (`/track/open`, `/o/`, `open.gif`, `?e=<id>`) | **in-model — built** | `TrackingPath` + `UniqueId` signals |
| Per-message **count** of trackers (Trocker, Proton) | **in-model — built** | header counts: trackers / suspected / other remote images / embedded |
| List of **domains that would be contacted** on open (Trocker, Sender Audit) | **in-model — built** | dedicated `hosts` report mode — a paste-ready host list for a blocklist/filter rule |
| CSS `background-image` beacons (Sender Audit) | **in-model — built** | scanned in `style` attributes and inside `<style>` blocks |
| `<link rel="prefetch"/"preload">` beacons (Sender Audit) | **in-model — built** | scanned as remote assets with their own signal |
| Click/link-wrapping tracker detection (Trocker, Gblock, Proton "cleaned links") | **in-model — built (opt-in)** | `include_links` param; off by default so the primary report stays about images |
| Distinguish embedded (`cid:`/`data:`) images, which cannot report an open | **in-model — built** | classified `embedded`, never counted as remote |
| Quoted-printable / base64 raw `.eml` handling (needed for any exported message) | **in-model — built** | `mail-parser` decodes the HTML part for `format = raw`/`auto` |
| Blocking / stripping the tracker before render | **in-model — built** | `report = clean` is out of scope, but the report states the concrete mitigation per host; see "rejected" below |
| Binary "tracked / not tracked" badge | **in-model — built** | 4-band verdict `TRACKED` / `LIKELY_TRACKED` / `REMOTE_CONTENT` / `CLEAN` |
| ETag / cookie fingerprinting analysis | **out-of-model** | requires actually issuing the HTTP request and reading response headers; this tool never performs network I/O |
| Live inbox integration / auto-flag in Gmail | **out-of-model** | needs an extension + OAuth mailbox access; gizza tools are local, no-account, no-server |
| Actually **blocking** the pixel in a mail client | **out-of-model** | requires the mail client render path; the tool reports what to block instead |
| Reputation / blocklist status lookups for tracker domains | **out-of-model** | needs a live feed + network I/O; the built-in vendor table is a curated snapshot instead |
| Per-recipient identity de-anonymisation ("who is `u=abc123`") | **out-of-model** | vendor-private identifier space; the tool reports *that* a unique ID is present, not who it is |

### Considered, rejected

- **Rewriting the email with the trackers stripped** (a `clean` output mode). In-model
  technically, but it turns a detector into an HTML rewriter, doubles the surface area, and
  would silently overlap the sanitizer/HTML-stripping family. The report names every offending
  URL and host so a user can strip or block them precisely.
- **Rating links for phishing** while `include_links` is on. Deliberately limited to
  *click-tracking* attribution (who learns you clicked); phishing risk scoring is a different
  question with its own dedicated tool shape, and duplicating it here would produce two
  divergent verdicts on the same link.

## UX patterns adopted

- **Preset chips** (`[[example]]`) for the three shapes users actually paste: a marketing email
  with a vendor pixel, a sales-tool pixel hidden in a plain-looking message, and a clean message.
- **Tag-list control** for the extra-vendor-domain field (a vocabulary of hostnames, not prose).
- **`hosts` report mode** so the output is directly pasteable into a content blocker — the
  competitor extensions show domains but give you no way to export them.
- Every signal is spelled out per image ("1×1 image", "hidden with display:none", "known tracker
  domain (Vendor)"), because a detector that only says "tracked" cannot be verified by the user.

## Honest limits (stated on the page, not just in code)

- The vendor table is a curated snapshot, not the ~400-domain commercial databases the extensions
  license; unknown hosts still get flagged on structural signals, and `vendors` extends the table.
- No network I/O: the tool never loads an image, resolves DNS, follows a redirect, or reads an
  ETag. A `CLEAN` result means no remote asset was declared in the markup it was given.
- A remote image with no tracking signal is still reported: any remote load leaks your IP,
  approximate location, client and open time to whoever hosts it.
