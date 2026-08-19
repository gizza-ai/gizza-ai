## About this tool

A cloud share link points at a **preview page**, not at the file. Paste it into an `<img>` tag, a download script, a README, or a CI job and you get an HTML page back instead of bytes. This tool rewrites that share URL into the direct URL for the file itself, for four link families:

- **Google Drive** — `drive.google.com/file/d/FILE_ID/view`, `open?id=FILE_ID`, and Docs/Sheets/Slides editor links.
- **Dropbox** — both the current `/scl/fi/…` shape and the older `/s/…` one, plus content-host links.
- **OneDrive** — `1drv.ms`, `onedrive.live.com`, and SharePoint share URLs.
- **Nextcloud / ownCloud** — public `/s/TOKEN` and `/index.php/s/TOKEN` shares on any host, including a self-hosted server on a custom port.

The provider is detected from the URL's host and path, so the default path is paste-and-go. **Link should** picks between forcing a download and serving the file inline (a hotlink you can embed). **Give me** wraps the result as a bare URL, a Markdown link, an HTML anchor, or a ready-to-run `curl` / `wget` command. **Convert each line separately** is on by default, so you can paste a whole column of links and get a column back, blank lines preserved.

Nothing leaves your browser. The conversion is string surgery on the URL you typed — no cloud provider is contacted, no link is resolved, and no file is fetched. That also means the tool cannot check whether a link is valid or whether the file is actually shared publicly.

### Worked example

Paste a Dropbox share link, leave every option at its default:

```text
https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&dl=0
```

The result is the same link with the download flag flipped, and the rest of the query preserved intact:

```text
https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&dl=1
```

The `rlkey` and `st` tokens are what authorise the share, so dropping them would break the link — they are kept and only `dl` is rewritten. Switch **Give me** to *curl command* and the same input becomes a command that saves the file under its real name:

```text
curl -L -o "report.pdf" "https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&dl=1"
```

Switch **Link should** to *Serve inline* instead and you get `?raw=1`, the form that serves the bytes in place — the one to use inside an `<img>` tag or a Markdown image.

### Limits and edge cases

- **Sharing permissions are not changed.** A rewritten link is only as public as the original share. If the file requires sign-in, the direct link will too.
- **Google Drive folder links are rejected** with an explicit error. A folder has no single direct-download URL; share the individual file instead.
- **Unrecognised hosts return an error, not a guess.** Box, pCloud, MediaFire, WeTransfer and similar services are not supported, and no link is emitted for them.
- **OneDrive is the least predictable of the four.** Two known rewrites are offered — the shares-API content URL (default) and a plain `download=1` flag — because Microsoft's behaviour varies by share type and tenant. Try the other method if the first one lands on a preview page.
- **Google Docs/Sheets/Slides editor links export, they do not download.** There is no original file to fetch, so the link renders the document as PDF, Office (`docx`/`xlsx`/`pptx`), or text (`csv` for Sheets), selected by the export dropdown.
- **The file name is only known when the URL contains it.** Dropbox and Nextcloud links usually carry it; Drive and OneDrive tokens do not, so `curl` / `wget` output falls back to the server-provided name (`curl -OJ`, `wget --content-disposition`).
- **Nextcloud folder shares need the file name.** A `/s/TOKEN` link to a folder downloads the whole folder as a zip; fill in the optional file field to address one file inside it.
- **Providers change their URL shapes.** These are undocumented rewrite rules, not public APIs; a rule that works today can stop working after a provider changes its front end.

## FAQ

<details>
<summary>Why does my share link download an HTML page instead of the file?</summary>

Because the URL points at the provider's preview page, which is an ordinary web page containing a download button. Anything that fetches a URL — `curl`, `wget`, an `<img>` tag, a CI step — receives that page's HTML. The rewritten link points at the file endpoint instead, so the response body is the file's bytes.

</details>

<details>
<summary>Does this make a private file public?</summary>

No. The rewrite only changes the shape of the URL; it never touches the share's permissions, and the tool never contacts the provider at all. If the original link asks for a sign-in, so will the converted one. Set the share to "anyone with the link" in the provider's own interface first, then convert.

</details>

<details>
<summary>What is the difference between download and inline mode?</summary>

Download mode produces a link that makes the browser save the file — the right choice for install scripts, release assets, and `curl`. Inline mode produces a hotlink that serves the bytes in place, which is what an `<img src>`, a Markdown image, or a `<video>` tag needs. Under the hood they are different endpoints per provider: `uc?export=view` for Drive, `raw=1` for Dropbox, `/preview` for Nextcloud, and the shares content endpoint for OneDrive.

</details>

<details>
<summary>Why does a Google Drive folder link produce an error?</summary>

A folder is not a file, and Drive only zips a folder from inside its own web interface — there is no stable URL that returns that zip. Emitting a link anyway would produce something that 404s, so the tool reports what is wrong and asks for a link to the individual file. Nextcloud is the exception: a folder share does have a `/download` endpoint, and naming a file in the optional field addresses one file inside it.

</details>

<details>
<summary>Can I convert a whole list of links at once?</summary>

Yes — that is the default. Paste one link per line and each is converted independently, with blank lines kept so a pasted spreadsheet column stays aligned. The links may come from different providers in the same batch, since each line is detected on its own. If any line fails, the whole batch reports that line's error rather than returning a partly-wrong list. Turn the batch option off to treat the entire input as a single URL.

</details>

<details>
<summary>My large Drive file still shows a virus-scan warning. Why?</summary>

Drive refuses to scan files above roughly 100 MB and shows an interstitial instead of the bytes. The generated download URL uses the user-content download host together with the confirm token, which is the form that normally serves such files straight through. Access rules still apply on top of that: a file that is not shared publicly will show a sign-in page no matter which URL form is used.

</details>
