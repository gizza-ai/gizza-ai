## About this tool

APK Permission Explainer reads an Android app package (APK) or its `AndroidManifest.xml`, extracts the permissions requested by the app, and explains each permission in plain English. The report groups permissions by risk so you can quickly spot camera, location, contacts, microphone, phone, SMS, storage, overlay, app-inventory, advertising ID, and system-only requests.

Paste either:

- the APK file encoded as Base64;
- the binary `AndroidManifest.xml` encoded as Base64; or
- a decoded text manifest starting with `<manifest ...>`.

Example decoded manifest:

```xml
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.demo">
  <uses-sdk android:minSdkVersion="24" android:targetSdkVersion="34" />
  <uses-permission android:name="android.permission.CAMERA" />
  <uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" />
  <uses-permission android:name="android.permission.INTERNET" />
</manifest>
```

With the defaults, the output is a Markdown report with package metadata, a risk summary, and a permission-by-permission explanation. Use `CSV` or `JSON` when you need to paste the result into a spreadsheet or another audit script.

## Limits and edge cases

- The tool is fully local and does not upload APK contents.
- It reads the manifest only. It does not scan bytecode for tracker SDKs or hidden behavior.
- APKs are ZIP files; split APK sets, `.apks` bundles, and Android App Bundles may store manifests in different paths and are outside this tool's scope.
- Unknown vendor or app-defined permissions are preserved and marked as unknown instead of being guessed.
- A requested permission means the app can ask for or receive that permission; Android version, runtime prompts, user settings, and store policy determine whether it is actually granted.

## FAQ

<details>
<summary>Can this tell whether an APK is malware?</summary>

No. Permissions are an important clue, but they are not a full malware scan. A harmless navigation app may legitimately request location, while a malicious app can hide bad behavior behind ordinary network permissions. Treat this as a manifest audit, not a verdict.

</details>

<details>
<summary>Why are some scary-looking permissions marked normal?</summary>

Android's protection levels vary. `INTERNET` is normal because Android grants it automatically, even though network access can matter when combined with other data access. The descriptions call out those practical privacy implications where they matter.

</details>

<details>
<summary>What does signature/system mean?</summary>

Signature permissions are normally granted only to system apps or apps signed with the same certificate as the permission owner. If a regular third-party APK requests one, it usually will not receive it, but the request is still useful context during an audit.

</details>

<details>
<summary>Why paste Base64 instead of uploading a file here?</summary>

This generic page surface accepts text fields. Encoding the APK as Base64 keeps the workflow local and reproducible across the web page, CLI, and chat tool. If you already decoded the manifest with another tool, you can paste the XML directly instead.

</details>
