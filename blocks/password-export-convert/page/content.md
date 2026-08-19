## About this tool

Password managers all export the same handful of facts — a name, a username, a password, a URL, a
note, sometimes a 2FA secret and a folder — and every one of them writes those facts under
different column names. That mismatch is why an export from one manager usually imports into
another as a pile of blank rows, or drops your two-factor secrets on the floor.

This converter reads six export layouts and writes all six, in any direction:

| Format | Shape it reads and writes |
|---|---|
| Bitwarden JSON | `folders` + `items`, with logins, secure notes, cards and identities |
| Bitwarden CSV | `folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp` |
| KeePass / KeePassXC CSV | `Group,Title,Username,Password,URL,Notes,TOTP,Icon,Last Modified,Created` |
| LastPass CSV | `url,username,password,totp,extra,name,grouping,fav` |
| Chrome / Edge / Safari CSV | `name,url,username,password,note` |
| Plain spreadsheet CSV | `folder,name,url,username,password,notes,totp,favorite,type` |

Every entry is read into one neutral shape — folder, name, username, password, URL, notes, TOTP,
favourite, kind — and then written out in the target's own column order, so nothing has to be
hand-edited afterwards. Folders become KeePass groups and LastPass groupings and back again.
Two-factor secrets land in the TOTP column wherever the target has one, and get folded into the
note for Chrome CSV, which does not. Anything the target has no column for — Bitwarden custom
fields, card numbers and expiry dates, identity details, extra URIs, a stray column your old
manager invented — is appended to the entry's notes instead of vanishing.

### A worked example

Paste this Bitwarden JSON export, leave the source format on **Detect it for me**, and pick
**KeePass / KeePassXC — CSV** as the target:

```json
{"encrypted":false,"folders":[{"id":"f1","name":"Work"}],"items":[{"id":"i1","folderId":"f1","type":1,"name":"Example Mail","notes":"recovery codes in the safe","favorite":true,"login":{"uris":[{"uri":"https://mail.example.com"}],"username":"demo-user@example.com","password":"sample-passphrase-1","totp":"JBSWY3DPEHPK3PXP"}}]}
```

Out comes a file KeePassXC imports directly:

```csv
"Group","Title","Username","Password","URL","Notes","TOTP","Icon","Last Modified","Created"
"Work","Example Mail","demo-user@example.com","sample-passphrase-1","https://mail.example.com","recovery codes in the safe","JBSWY3DPEHPK3PXP","0","",""
```

Save it, import it, then delete the file — a plain-text export of your vault is exactly as
sensitive as the vault itself.

### Limits worth knowing

- **5000 entries per run.** Larger vaults should be split and converted in batches.
- **Unencrypted exports only.** A password-protected Bitwarden JSON export is rejected with a
  message telling you to re-export with that option turned off; this tool converts formats, it
  does not decrypt.
- **Text in, text out.** Paste the export's contents; there is no file picker, and `.1pux` or
  `.kdbx` files are archives and databases rather than pasteable text, so they are out of scope.
- **The output is the file and nothing else** — no header comment, no summary line — so it can be
  saved and imported verbatim. Identifiers are derived from entry names rather than generated
  randomly, so converting the same export twice gives byte-identical output.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>Is my password export uploaded anywhere?</summary>

No. The converter is a WebAssembly module that runs inside this browser tab, and the conversion
happens on the text in the box — there is no upload, no server round-trip, no analytics on what
you paste. You can prove it to yourself by opening your browser's network panel before converting,
or by loading the page, going offline, and converting anyway.

That said, a decrypted export is a plain-text copy of every password you own. Convert it, import
it into the new manager, then delete both files and empty the trash.

</details>

<details>
<summary>Do two-factor (TOTP) secrets survive the conversion?</summary>

Yes, as long as **Carry two-factor (TOTP) secrets across** is left on. This is the single most
common thing lost in a migration: several popular converters drop the TOTP column outright, so the
new manager imports the password but not the authenticator code.

Bitwarden, KeePass/KeePassXC, LastPass and the generic spreadsheet format all have a TOTP column,
so the secret moves straight across. Chrome's CSV format has no such column — rather than throw
the secret away, it is written into that entry's note, where you can copy it into an authenticator
by hand. Turning the option off strips 2FA secrets from the output entirely, which is what you
want if you are exporting a list to share.

</details>

<details>
<summary>What happens to my folders, custom fields and secure notes?</summary>

Folders map to whatever the target calls them — a Bitwarden folder becomes a KeePass group and a
LastPass grouping, and back the other way. Entries with no folder stay at the top level unless you
name one in the folder box, which is handy when you want everything filed under `Imported`.

Custom fields, card details, identity details and secondary URIs have no column in most target
formats, so with **Keep custom fields, card and identity details in the notes** on (the default)
they are appended to the entry's note as labelled lines. Secure notes come across as entries with
no password. If the manager you are importing into rejects rows with an empty password column,
turn on **Drop entries that have no password** and those rows are left out.

</details>

<details>
<summary>My export has commas and quotes inside the passwords — will it break?</summary>

No. Both the reading and the writing side go through a real RFC 4180 CSV parser, so a password
containing a comma, a double quote or even an embedded newline is parsed as one value and re-quoted
correctly on the way out. This is a genuine failure mode elsewhere: managers disagree about CSV
escaping, and the usual symptom is a run of entries importing with empty or shifted fields.

The one thing that does break it is an export whose header row has been edited or removed by
hand. If auto-detection then guesses wrong, pick the source format from the dropdown yourself.

</details>

<details>
<summary>Why does auto-detection pick the wrong format sometimes?</summary>

Detection works off the shape of the file: a leading `{` or `[` means Bitwarden JSON, and
otherwise the header row decides — `login_password` means a Bitwarden CSV, `grouping` plus `extra`
means LastPass, `Title` plus `Group` means KeePass, `name`/`url`/`note` means Chrome, and anything
else carrying a password or username column is read as a generic spreadsheet.

If you renamed or reordered the header cells, two dialects can look alike, and the guess goes
wrong. Set **Format you are converting from** explicitly and the guess is skipped. If the header
is missing entirely, the conversion is refused rather than silently mangled — add the header row
back and try again.

</details>

<details>
<summary>Can it convert a 1Password .1pux file or write a .kdbx database?</summary>

Not directly. A `.1pux` file is a zip archive rather than pasteable text, and a `.kdbx` file is an
encrypted database that needs a master key to write — that is a different job from converting
between text export formats.

The practical route for both is one hop through a format that is text: export 1Password to CSV and
convert that, or convert to KeePass CSV and let KeePassXC import it into your `.kdbx` database.
Firefox's CSV export works too — it is Chrome's layout plus some extra metadata columns, and the
reader accepts it as-is.

</details>
