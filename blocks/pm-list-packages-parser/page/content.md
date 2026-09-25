## About this tool

`pm list packages` is the Android package-manager command behind many QA, debloat and device-inventory workflows. The raw output is useful, but it mixes package names with APK paths and optional decorations such as installer, UID and version code. This parser turns a pasted command result into a clean table you can sort, filter, export and paste into a ticket.

Paste output from commands such as:

```sh
adb shell pm list packages -f
adb shell pm list packages -f -i -U --show-versioncode
adb shell pm list packages -d
adb shell pm list packages -s
```

Example package dump:

```text
package:/data/app/~~kJ2vQ==/com.example.notes-9QeL==/base.apk=com.example.notes
package:/data/app/~~7bR1w==/com.android.chrome-Ax2T==/base.apk=com.android.chrome
package:/system/priv-app/Settings/Settings.apk=com.android.settings
package:/vendor/app/CarrierHelper/CarrierHelper.apk=com.carrier.helper
```

With the default table output, packages under `/data/app` are labelled as user apps and packages under system partitions such as `/system`, `/product`, `/vendor`, `/odm`, `/oem` and `/apex` are labelled as system apps. Use the optional `pm list packages -s` field when you need to identify preinstalled apps that now run an update from `/data/app`; those rows become `system-updated` instead of being mistaken for ordinary user apps.

Example disabled-package paste:

```text
package:com.carrier.helper
```

A single `pm list packages -f` run does not include enabled/disabled state. When you paste the `-d` output into the disabled list field, the table can mark those rows as disabled and the enabled/disabled filters become meaningful.

## Output modes

- `table` gives an aligned text report with a summary line and optional grouped sections.
- `list` returns only package names, one per line, for shell scripts and allow/deny lists.
- `csv` keeps a stable spreadsheet-friendly schema.
- `json` returns structured rows and summary metadata.
- `markdown` produces a pipe table that is easy to paste into an issue or wiki.

## Limits and edge cases

- Each pasted package field is capped at 5,000 package lines. Real devices usually list hundreds, not thousands.
- Lines must contain package-manager output, for example `package:com.example.app` or `package:/data/app/.../base.apk=com.example.app`.
- Shell prompts, blank lines and `#` comments are ignored so you can paste a copied terminal snippet.
- Enabled/disabled status is unknown unless you provide a `pm list packages -d` paste.
- Display names, icons, permissions and version names are not present in `pm list packages` output; use other Android commands or APK inspection tools for those.
- The tool parses text only. It does not connect to a phone, run ADB, disable packages or uninstall anything.

## FAQ

<details>
<summary>Why are some Google or OEM apps shown as user apps?</summary>

If a preinstalled app has been updated, Android may report the active APK under `/data/app`. Paste `adb shell pm list packages -s` into the system list field so the parser can promote those rows to `system-updated`.

</details>

<details>
<summary>Can this tell which packages are safe to remove?</summary>

No. It classifies and filters package-manager output, but it does not maintain a device-specific safety database. Use the output as an inventory, then check your device model, ROM and enterprise policy before disabling or uninstalling packages.

</details>

<details>
<summary>Why does the disabled filter show nothing?</summary>

The main package list does not contain a disabled flag. Run `adb shell pm list packages -d` and paste that result into the disabled-package field; then the disabled and enabled filters can use that extra list.

</details>

<details>
<summary>Which command should I run first?</summary>

Start with `adb shell pm list packages -f -i -U --show-versioncode` if your Android build supports those flags. The parser will still accept simpler `package:com.example.app` lines, but paths and decorations give it more columns to work with.

</details>
