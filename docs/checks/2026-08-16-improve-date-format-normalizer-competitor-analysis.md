# date-format-normalizer competitor analysis (2026-08-16)

## Scope

Tool: paste arbitrary text containing dates in mixed written forms and rewrite every detected date into one target format, without changing the surrounding prose. The important model question is whether a date such as `03/04/2024` is day-first or month-first; this build resolves that from the whole text when possible and exposes the decision in report mode.

Research done 2026-08-16: web search for online bulk date format converters and date-normalization tools, plus direct comparison with the date parsing conventions those tools commonly imitate. Findings are paraphrased; no competitor copy, branding, or trademarks are reused in the shipped page.

## Competitor scan

| Source | Table stakes found | In-model decision |
| --- | --- | --- |
| eConvert-style bulk date-format changer | Paste a list of dates, pick a target layout such as `dd-mm-yyyy` or `yyyy-mm-dd`, click convert, and copy the normalized list. The control surface is simple and list-oriented. | Built the simple path, but for free text rather than only one-date-per-line input: `output_format` includes ISO, ymd, dmy, mdy, month-name, RFC 2822, unix seconds/millis, and custom strftime. `output_mode=list` covers the one-date-per-line workflow. |
| TurboToolkit-style bulk conversion tools | Broader batch features: trim/normalize whitespace, exclude invalid values, unique results, export TXT/CSV/JSON, and invalid-value placement. | In model where it is date normalization: invalid calendar dates are left untouched in text mode, omitted from list mode only by not being detected, and surfaced in report mode through detected counts. Deduping/export policy is out of model for a prose-preserving text tool and already better served by CSV/list tools. |
| WebTextTools-style date format converter | Detect common numeric, ISO, month-name, and Unix timestamp forms; live preview; direct controls for input/output patterns. | Built broad detection: ISO, year-first numeric, slash/dot/dash numeric, English month names with ordinal/weekday variants, optional adjacent times, timezone offsets, and opt-in 10/13-digit epochs. Custom output uses chrono/strftime. |
| Online-free-tools style date-time format changer | Requires an explicit source format and output format for bulk conversion, including date-time values. | Built the no-source-format path for mixed text: the detector finds every occurrence independently, then resolves the numeric day/month ambiguity globally. Explicit override remains available through `input_order`. |
| General date parsing library conventions | Common knobs are day-first/month-first hints, strftime patterns, two-digit-year pivots, timestamps, and timezone handling. | Built `input_order`, `custom_format`, `two_digit_year_pivot`, `detect_timestamps`, `keep_time`, `time_style`, and `output_timezone`. Ambiguity is not guessed silently: report mode states the chosen order and why. |

## Parameters and defaults

| Capability | Default / options | Status |
| --- | --- | --- |
| Source text | Required `text`, up to 1,000,000 bytes | In model, built. Preserves every non-date byte in text mode. |
| Target format | `output_format=iso` by default; `ymd`, `dmy`, `mdy`, `month_day_year`, `day_month_year`, `rfc2822`, `unix_seconds`, `unix_millis`, `custom` | In model, built as enum. |
| Custom output | `custom_format` strftime pattern, required only with `output_format=custom` | In model, built and validated. |
| Numeric punctuation | `separator=dash`; also `slash`, `dot`, `none`, `space` | In model, built for ymd/dmy/mdy. |
| Month and year style | `month_style=full/short`, `year_style=four/two`, `leading_zeros=true` | In model, built. |
| Ambiguous numeric input | `input_order=auto`; alternatives `day_first`, `month_first` | In model, built. Auto reads all numeric dates first and lets unambiguous values decide the rest. |
| Two-digit years | `two_digit_year_pivot=68`, range 0-99 | In model, built. |
| Times and zones | `keep_time=true`, `time_style=24h/12h`, `output_timezone=source` or UTC/IANA/fixed offset | In model, built for dates carrying explicit offsets; zone-less dates are left as written. |
| Epoch detection | `detect_timestamps=false` | In model, built behind an opt-in because long numbers are often identifiers. |
| Result shape | `output_mode=text`; alternatives `list`, `report` | In model, built. Report lists counts, order decision, line/column, original and rewritten value. |
| Locale-specific month names | — | Out of model for this release: English month names and weekdays only. Multi-locale tables are a plausible future enhancement, not required by the scanned tools. |
| Natural-language dates such as “next Friday” | — | Out of model: needs a clock, locale and relative-date policy. This tool is deterministic and does not read the current date. |
| Cloud batch jobs, saved presets/accounts, file upload | — | Out of model for this public toolkit block. The page runs locally; copy/download comes from the generic page shell. |

## UX decisions taken from the scan

- Default to the common successful workflow: paste mixed date text, get ISO 8601 text back, and keep clock times when they were present.
- Put every fixed-choice parameter in a select with example labels, because names like `ymd`, `dmy`, `unix_millis`, and `month_day_year` are easy to confuse.
- Ship example chips for the real workflows: mixed text to ISO, US dates to European dots, filename stamps, list extraction, audit report, and epoch/offset conversion.
- Make timestamp detection opt-in. The most damaging false positive in this category is rewriting an order id or phone number as a date.
- State limits and edge cases on the page: byte cap, impossible dates left alone, two-digit-year pivot, timezone conversion only for offset-bearing dates, and report mode for ambiguity review.
