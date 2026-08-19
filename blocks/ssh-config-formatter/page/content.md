## About this tool

SSH Config Formatter reads an OpenSSH **client** configuration — the file at `~/.ssh/config`, or a system-wide `/etc/ssh/ssh_config` — and does two things with it.

It **normalizes** the text: every `Keyword=Value` becomes `Keyword Value`, keywords get their canonical OpenSSH spelling (`hostname` → `HostName`, `identityfile` → `IdentityFile`), mixed tabs and spaces become one consistent indent, and blank lines land between blocks. Optionally it aligns values into a column, sorts the keywords inside each block, and deletes the repeated keywords SSH already ignores.

It also **lints** the file for the mistakes that silently change behaviour. SSH does not "last one wins" like most config formats — it uses the **first** value it obtains for each keyword as it reads top to bottom, so a `Host *` block near the top quietly overrides everything below it. The checker flags duplicate `Host` patterns, blocks whose patterns are already matched by an earlier block, a wildcard block that is not last, keywords that are unknown, deprecated, or only valid in `sshd_config`, missing values, out-of-range ports, and invalid `yes`/`no` or fixed-choice values.

Everything runs locally in your browser. Your config — hostnames, users, jump hosts, key paths — is never uploaded, and the tool never resolves DNS or connects to anything.

### Worked example

Paste this:

```
Host  web
hostname=10.0.0.5
   User deploy
  port 2222
```

With **Output** = *Formatted config* and the default 2-space indent you get:

```
Host web
  HostName 10.0.0.5
  User deploy
  Port 2222

# ssh-config-formatter: no issues found
```

Now switch **Output** to *Lint report* on a config with a leading wildcard:

```
Host *
  User root

Host web
  HostName 10.0.0.5
```

The report says the `Host *` block matches every host but two blocks follow it, so `User root` wins over anything below — move the wildcard block to the bottom of the file.

Set **Output** to *Host list only* for a paste-ready list of every alias you have configured, or *Structured JSON* to feed the parsed blocks, issue list and stats into a script.

### Limits and edge cases

- **Client config only.** `sshd_config` keywords such as `PermitRootLogin` or `AllowUsers` are reported as server-only rather than validated; this tool is not an `sshd` linter.
- **Maximum 10,000 lines.** Longer input is rejected with an error instead of being truncated.
- **`Include` is not followed.** The tool sees only the text you paste, so duplicate and shadow checks cannot span included files. Each `Include` line is reported as an info note.
- **Shadow detection is pattern-level.** It reports that an earlier block already matches the same hosts; whether a specific keyword is actually overridden depends on which keywords each block sets.
- **Values are not resolved.** Host names are not looked up, key files are not opened, and `ProxyCommand` strings are not executed or parsed. Unknown-but-valid values (ciphers, algorithm lists, paths) are passed through untouched.
- **Comments move with their directive.** A `#` line is treated as documentation for the line below it, so sorting keeps them together. OpenSSH does not support a comment at the end of a directive line, and the tool flags that case rather than stripping it.

## FAQ

<details>
<summary>Does SSH really use the first matching value, not the last?</summary>

Yes. From `ssh_config(5)`: for each keyword, the **first obtained value** is used. `ssh` reads the file top to bottom and every `Host`/`Match` block whose patterns match the host contributes, but only the first setting of a given keyword takes effect. That is why the conventional layout puts specific hosts first and a `Host *` defaults block last — and why this tool warns when a wildcard block appears before other blocks.

</details>

<details>
<summary>Why is my keyword reported as unknown when it works?</summary>

The keyword table covers OpenSSH 9.x `ssh_config`. A keyword can be flagged because it is misspelled (`HostNmae`), because it belongs to `sshd_config`, because OpenSSH removed it (`Protocol`, `RSAAuthentication`) or renamed it (`ChallengeResponseAuthentication` → `KbdInteractiveAuthentication`), or because a wrapper tool reads it and `ssh` itself does not. For that last case, list the keyword under `IgnoreUnknown` so real `ssh` skips it instead of aborting.

</details>

<details>
<summary>What does "Remove duplicate keywords SSH ignores" delete?</summary>

Only repeats inside the same block of a keyword that takes a single value — for example a second `User` line, which `ssh` already discards because the first one won. Keywords that legitimately repeat are never touched: `IdentityFile`, `CertificateFile`, `LocalForward`, `RemoteForward`, `DynamicForward`, `SendEnv`, `SetEnv`, `PermitRemoteOpen` and `Include` each add a value per line. Duplicates are reported as warnings whether or not you enable removal.

</details>

<details>
<summary>Is it safe to paste my real SSH config here?</summary>

The tool compiles to WebAssembly and runs entirely inside the page — there is no upload, no network request, and no analytics on the config text. It still only ever sees what you paste, so private keys do not belong in the box: `~/.ssh/config` references key files by path, and those paths are all this tool needs.

</details>

<details>
<summary>Can it check a config for syntax errors before I connect?</summary>

It catches the classes of error `ssh` rejects at parse time — missing values, non-numeric or out-of-range `Port`, a `yes`/`no` keyword given something else, an invalid `StrictHostKeyChecking` value, an empty `Host` line — plus the semantic problems `ssh` accepts silently. Set **Report from severity** to *Errors only* for just the fatal ones. For a live check against your real file, `ssh -G <host>` prints the effective configuration OpenSSH itself computes.

</details>
