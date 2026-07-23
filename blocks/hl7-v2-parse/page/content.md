## About this tool

Paste an HL7 v2.x message and get a structured breakdown of its segments, fields,
components, subcomponents, and repetitions. The parser reads the field separator and
encoding characters from the `MSH` segment, handles the special `MSH-1` / `MSH-2`
offset, and can attach human-readable names for common segments and fields such as
`MSH`, `PID`, `PV1`, `OBR`, and `OBX`.

Use **JSON** when you want the full hierarchy, including repetitions and nested
components. Use **CSV** when you want a flat table of non-empty leaf values with
locations like `PID.5.1` or `OBX.5`. The tool runs locally in your browser, so the
message text is not uploaded.

### Worked example

Input:

```text
MSH|^~\&|SENDINGAPP|SENDINGFAC|RECVAPP|RECVFAC|20240101120000||ADT^A01|MSG00001|P|2.5
EVN|A01|20240101120000
PID|1||123456^^^HOSPITAL^MR||DOE^JOHN^Q||19800101|M
```

With output `json`, descriptions enabled, and HL7 unescaping enabled, the output is a
JSON array whose first object is the `MSH` segment. `MSH.9` is split into components
`ADT` and `A01`, and `PID.5` is labeled **Patient Name** with components `DOE`,
`JOHN`, and `Q`.

With output `csv`, the same message becomes rows like:

```csv
Segment,Location,Value,Description
MSH,MSH.9.1,ADT,Message Type
MSH,MSH.9.2,A01,Message Type
PID,PID.5.1,DOE,Patient Name
PID,PID.5.2,JOHN,Patient Name
```

### Limits and edge cases

- This is a parser/viewer, not a validator against a complete HL7 conformance
  profile.
- Field descriptions are curated for common segments. Unknown fields still parse,
  but may not have a description label.
- Segment separators may be carriage returns, line feeds, or CRLF.
- HL7 escape decoding covers `\F\`, `\S\`, `\T\`, `\R\`, `\E\`, `\Xhh\`, and
  `\.br\`; turn **Decode HL7 escape sequences** off to preserve raw values.
- Output is text (JSON or CSV). The tool does not convert HL7 to FHIR or edit and
  re-serialize messages.

<details>
<summary>Does this upload protected health information?</summary>

No. The parser runs as WebAssembly in your browser tab. It has no network step and
it does not send the pasted message to a server. You should still avoid pasting real
patient data into shared machines or screen recordings.

</details>

<details>
<summary>Why is MSH field numbering different from other segments?</summary>

HL7 v2 defines `MSH-1` as the field separator itself and `MSH-2` as the encoding
characters. That means the text after `MSH|^~\&|...` starts at `MSH-3`. This tool
models that offset explicitly, so locations match HL7 field numbering.

</details>

<details>
<summary>What is the difference between JSON and CSV output?</summary>

JSON preserves the whole tree: segment → field → repetition → component →
subcomponent. CSV flattens only non-empty leaf values into rows with a `Location`
column, which is easier to paste into a spreadsheet or filter for a quick audit.

</details>

<details>
<summary>Can it validate every HL7 version and field name?</summary>

No. HL7 v2 dictionaries are large and version/profile-specific. This tool includes
common segment and field names for readability, but it does not enforce a full
conformance profile or guarantee that every field code is legal for a given version.

</details>
