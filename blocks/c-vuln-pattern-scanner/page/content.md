## About this tool

C and C++ bugs usually start as recognizable code shapes: `printf(user_input)`, `strcpy` into a fixed buffer, `%s` scanned with no field width, allocation sizes computed with unchecked multiplication, a pointer used after `free()`, an index that runs one element past the end. Paste **C/C++ code** here and the scanner reports each one with a line number, a severity, a rule code, a **CWE id** and a short fix hint — in your browser, with nothing uploaded.

**Language** is `auto`, `c` or `cpp`. `auto` reads the paste as C++ when it sees markers such as `std::`, `class`, `namespace`, `template` or `#include <iostream>`, otherwise C. The choice matters because the `CPP-STREAM` rule (unbounded `std::cin >> buf` into a `char` array) only fires for C++.

**Rule family** narrows what runs: `all` (every rule), `memory` (buffers, bounds, allocation, lifetime), `injection` (format strings, `system()`, temp-file and TOCTOU races), `crypto` (weak randomness, broken algorithms) or `banned` (the dangerous-libc list). **Minimum severity** — `all`, `low`, `medium`, `high`, `critical` — collapses a large paste down to what is likely exploitable; `all` and `low` are equivalent. **Ignore rule codes** takes a comma- or space-separated, case-insensitive list such as `BOUNDED-COPY, MEM-LEAK` to mute a rule that is noisy in your codebase without raising the severity floor. **Output format** is `text` (readable report with a severity roll-up header), `json` (language, profile, per-severity counts and a findings array) or `csv` (`line,severity,code,cwe,message,source`, RFC-4180 quoted, ready for a spreadsheet or a CI job). **Show the matching source line** is on by default; turn it off for one compact line per finding that is easier to diff or grep.

Worked example — paste:

```c
void f(char *s) { char b[8]; strcpy(b, s); printf(s); gets(b); }
```

and the report opens with `C/C++ vulnerability scan (c) · 3 findings · 1 critical · 2 high · 0 medium · 0 low`, then lists `BANNED-COPY (CWE-120)` for the unbounded `strcpy`, `FORMAT-STRING (CWE-134)` for `printf` with a non-literal format, and `GETS (CWE-242)` for the `gets()` call — each with the offending source line underneath. Set **Minimum severity** to `critical` and only the `GETS` finding survives; switch **Output format** to `csv` and the same three rows arrive as `1,critical,GETS,CWE-242,...`.

Two things keep the noise down. Comments and the bodies of string and character literals are masked before the rules run (the quotes are kept), so `// remember to drop strcpy` or `puts("gets is banned")` does not produce a finding. And a `// vuln-scan: ignore` comment suppresses findings on its own line and the line immediately after it, so you can annotate a reviewed exception in the source instead of maintaining an ignore list.

Limits and edge cases: input is capped at **200,000 bytes** (200 KB) — larger pastes are rejected rather than silently truncated. This is a lexical scan, not a compiler: the code is never compiled, preprocessed, linked or executed, and there is no control-flow, data-flow, type or scope information. That means anything hidden behind a macro is invisible (a `SAFE_COPY(dst, src)` macro that expands to `strcpy` will not fire), code inside `#if 0` or a disabled `#ifdef` branch is still scanned because no preprocessing happens, and multi-line constructs split across statements can be missed. Conversely a safe wrapper that deliberately uses a risky API name will still be flagged. Findings mean "worth a human look", never "proven vulnerability", and a clean report is not a proof of safety — compile with `-Wall -Wextra`, run sanitizers and fuzzers, and review the code before shipping anything security-sensitive.

## FAQ

<details>
<summary>Does this compile, run or upload my code?</summary>

No, none of the three. The scan is pure text processing that happens locally in your browser via WebAssembly — no compiler, preprocessor, linker, shell, build system or network request is involved. That makes it safe to paste a snippet from an untrusted bug report, and it means the tool works offline once the page has loaded.

</details>

<details>
<summary>What do the rule codes and CWE ids mean?</summary>

Every finding carries a stable rule code and the matching Common Weakness Enumeration id, so you can cross-reference it against MITRE's catalogue or feed it into an existing triage process. Critical: `GETS` (CWE-242), `BUFFER-OVERRUN` (CWE-787). High: `BANNED-COPY` (CWE-120), `SCANF-UNBOUNDED` (CWE-120), `FORMAT-STRING` (CWE-134), `COMMAND-EXEC` (CWE-78), `USE-AFTER-FREE` (CWE-416), `SIZEOF-POINTER` (CWE-467), `CPP-STREAM` (CWE-120). Medium: `OFF-BY-ONE` (CWE-193), `INT-OVERFLOW` (CWE-190), `SIGN-CONVERSION` (CWE-195), `UNBOUNDED-ALLOC` (CWE-770), `UNCHECKED-ALLOC` (CWE-476), `TEMP-FILE` (CWE-377), `TOCTOU` (CWE-367), `WEAK-RANDOM` (CWE-330), `WEAK-CRYPTO` (CWE-327). Low: `BOUNDED-COPY` (CWE-120), `MEM-LEAK` (CWE-401). Any of those codes can go in the **Ignore rule codes** box.

</details>

<details>
<summary>How do I silence a finding I have already reviewed?</summary>

Two ways. Put `// vuln-scan: ignore` in the source on the finding's line or the line directly above it, and that finding disappears — the annotation travels with the code and survives a re-paste. Or list the rule code in the **Ignore rule codes** field (`BOUNDED-COPY, MEM-LEAK`, comma- or space-separated, case-insensitive; unknown codes are ignored) to mute it for the whole scan. Raising **Minimum severity** is the blunter option when you just want the top of the list.

</details>

<details>
<summary>Why did it flag safe code, or miss something obviously wrong?</summary>

Because the rules are lexical patterns with no understanding of your program. A bounds-checked `memcpy`, a `strcpy` into a buffer that a caller already sized, or a format string validated elsewhere all still match the shape and get reported. In the other direction, a dangerous call reached through a macro, a function pointer, a template, or a construct split across several lines has no visible pattern to match, and nothing inside `#if 0` is skipped or resolved because no preprocessing runs. Treat the output as a review prompt.

</details>

<details>
<summary>Can I use JSON or CSV output in CI?</summary>

Yes — that is what they are for. `json` gives you `language`, `profile`, `min_severity`, a `summary` object with per-severity counts, and a `findings` array where each entry has `line`, `code`, `cwe`, `severity`, `message` and `source`. `csv` emits the header `line,severity,code,cwe,message,source` with RFC-4180 quoting, so it opens cleanly in a spreadsheet or pipes into a review bot. Turning off **Show the matching source line** blanks the `source` field in both, which makes the output stable to diff across commits.

</details>

<details>
<summary>Does this replace clang-tidy, CodeQL, sanitizers or fuzzing?</summary>

No. Those tools parse the code, understand types and data flow, see your build flags, and in the case of sanitizers and fuzzers observe real runtime behaviour. This page is a fast pre-review pass you can run on a snippet in a browser tab, with no toolchain and no project setup. Use it to catch the obvious shapes early, then rely on the real tooling before a release or during incident response.

</details>
