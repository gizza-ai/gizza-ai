## About this tool

Email Tracking Pixel Detector inspects raw email source or HTML markup for the remote assets that can report when a message is opened. It looks for tiny `1×1` or `0×0` images, CSS-hidden images, known email-service and sales-tool tracking hosts, open/pixel/beacon URL paths, unique recipient IDs in query strings, CSS `background-image` beacons, and prefetch/preload links.

Everything runs locally in your browser. The detector never fetches the image, follows a redirect, resolves DNS, opens a mailbox, or uploads your email. A `TRACKED` result means the pasted markup declares at least one high-confidence open tracker. `LIKELY_TRACKED` means a remote asset has structural tracking signals but no known vendor match. `REMOTE_CONTENT` means the email would contact a remote host even though no tracker-specific signal was found.

### Worked example

Paste this HTML:

```html
<img src="https://track.hubspot.com/open.gif?email=a@example.com&id=abc123456789abcdef" width="1" height="1" style="display:none">
```

The summary reports `TRACKED`, lists `track.hubspot.com` as a host contacted on open, and explains the signals: known tracker domain, tiny pixel, hidden CSS, tracking path, and unique ID query.

Set **Report** to **Hosts only** when you want a paste-ready list for a mail rule, firewall, or content blocker. Add private ESP or CDN hosts in **Extra tracker domains** when your organization rewrites tracking links through a custom domain.

### Limits and edge cases

- The vendor table is a curated snapshot. Unknown hosts are still flagged when their markup looks like a tracker, and you can add custom hosts with the `vendors` field.
- Raw `.eml` input is scanned as pasted source. Encoded MIME parts that do not expose their HTML after export may need to be decoded by your mail client first.
- The tool does not perform network I/O, so it cannot inspect HTTP headers, ETags, cookies, redirects, or live reputation feeds.
- A `CLEAN` result only means no remote image/prefetch/link tracker was declared in the pasted markup. It cannot see content that your mail client would fetch or rewrite later.

## FAQ

<details>
<summary>What counts as a tracking pixel?</summary>

A high-confidence tracking pixel is usually a remote image that is tiny, hidden, hosted on a known email-tracking domain, or loaded from a URL containing open, pixel, beacon, tracking, or recipient-ID parameters. The tool reports every signal it used so you can verify the finding instead of trusting a black-box badge.

</details>

<details>
<summary>Does this block the tracker?</summary>

No. This is a detector, not a mail-client extension. It does not load or rewrite the email. Use the **Hosts only** report to copy remote hosts into a blocker or mail rule, or disable remote images in your mail client before opening suspicious messages.

</details>

<details>
<summary>Why are normal remote images reported?</summary>

Any remote image can reveal your IP address, rough location, mail client, and open time to the server that hosts it. If the image has no tracker-specific signal, the verdict is `REMOTE_CONTENT` instead of `TRACKED`, but it is still listed so you can decide whether loading it is acceptable.

</details>

<details>
<summary>Should I turn on link inspection?</summary>

Turn on **Also inspect links** when you want click-tracking hosts too. It is off by default because open tracking and click tracking answer different questions: an image can fire when you merely open a message, while a link only reports if you click it.

</details>
