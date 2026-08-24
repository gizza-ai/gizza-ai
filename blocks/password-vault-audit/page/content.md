## About the password vault audit

This tool audits a whole password list or password-manager export at once. Paste a CSV, Bitwarden JSON export, or a simple one-password-per-line list and it reports the issues that matter across the vault: reused passwords, duplicate saved logins, common passwords, weak or short passwords, similar variants such as `Summer2024!` / `Summer2025!`, stale items where the export includes modified dates, and saved `http://` URLs.

Passwords are **masked by default**. Instead of echoing the secret back, findings show a short non-reversible fingerprint plus the password length, which is enough to correlate a reuse group without putting plaintext secrets into a report you might share. Everything runs locally in the browser.

### Supported input formats

- **Auto-detect** reads leading `{` or `[` as Bitwarden JSON, a multi-column header row with a password column as CSV, and everything else as a plain list.
- **CSV export** works with common Bitwarden, LastPass, KeePass/KeePassXC, Chrome, 1Password, Dashlane and generic headers. It looks for columns like `password`, `username`, `name`, `url`, `totp`, and modified-date fields.
- **Bitwarden JSON** reads `items[]`, login username/password/uris/totp, and revision dates.
- **List** treats each non-empty line as one password and uses synthetic names like `line 3`.

### Worked example

Given this CSV:

```csv
name,username,password,url,totp
Email,ada@example.com,P@ssw0rd,http://mail.example.com,
Bank,ada,CorrectHorseBatteryStaple!,https://bank.example.com,otpauth://totp/Bank
Shop,ada,CorrectHorseBatteryStaple!,https://shop.example.com,
```

the report shows the vault score, counts the three entries and two distinct passwords, and raises findings for the common/leetspeak password, the reused password shared by Bank and Shop, the insecure `http://` URL on Email, and any missing-TOTP findings if that optional check is enabled.

### Limits & edge cases

- **Up to 5000 entries per run.** Split larger exports before auditing.
- **No live breach lookup.** The common-password check uses a bundled offline list of well-known weak passwords; it does not query Have I Been Pwned or any network service.
- **Strength scores are heuristics.** They are useful for triage, not a cryptographic proof. Reuse and known-common findings should be fixed first.
- **Dates depend on the export.** Stale-password findings only appear when the source includes a revision or modified date.
- **Masking is safest.** Turning off **Mask passwords in output** can expose secrets in the report; leave it on unless you are working with test data.

## FAQ

<details>
<summary>Does this upload my password vault?</summary>

No. The web page runs the audit in WebAssembly in your browser. The pasted text is not uploaded, stored, or sent to a third-party API. The CLI uses the same local core logic.

</details>

<details>
<summary>Why are passwords masked in the output?</summary>

An audit report often gets copied into an issue tracker or chat. Masking keeps plaintext passwords out of that report while still showing when two entries share the same secret by using the same short fingerprint and length.

</details>

<details>
<summary>Can it read my password manager export?</summary>

It reads Bitwarden JSON directly and CSV exports whose header names a password column. Common column names from Bitwarden, LastPass, KeePass/KeePassXC, Chrome, 1Password, Dashlane and generic exports are auto-detected. If auto-detect guesses wrong, set **Input format** to CSV, Bitwarden JSON, or List.

</details>

<details>
<summary>Is this the same as a password strength checker?</summary>

No. A single-password strength checker scores one secret. This audit looks across a vault and finds cross-entry problems such as reuse, duplicate saved logins, password variants, insecure saved URLs, stale entries, and missing stored TOTP fields.

</details>

<details>
<summary>Does a clean report mean my passwords were never breached?</summary>

No. The bundled common-password check only catches well-known weak passwords offline. A clean report means no issue was found by these local rules; it is not a live breach-database search and cannot prove a password was never leaked.

</details>
