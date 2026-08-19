import { test, expect } from './fixtures';

const CSV = 'name,age,city,joined\nAda,36,London,1815-12-10\nAlan,41,Wilmslow,1912-06-23';

const CSV_REPORT = `Format:       CSV (comma-separated values) (csv)
Confidence:   100%
Encoding:     utf-8 (declared: input pasted as text)
Line endings: LF (\\n, Unix)
Size:         73 bytes, 3 lines (3 sampled)

Delimiter:    "," (comma)
Quote char:   none detected
Header row:   likely
Columns:      4
Data rows:    2

Delimiter scores:
  ","     comma         4 cols  100% consistent
  "\\t"    tab           1 cols  100% consistent
  ";"     semicolon     1 cols  100% consistent
  "|"     pipe          1 cols  100% consistent
  ":"     colon         1 cols  100% consistent
  "~"     tilde         1 cols  100% consistent
  " "     space         1 cols  100% consistent

Columns:
   1  name    text
   2  age     integer
   3  city    text
   4  joined  date

Preview:
  name | age | city     | joined
  Ada  | 36  | London   | 1815-12-10
  Alan | 41  | Wilmslow | 1912-06-23

Notes:
  - Encoding is UTF-8 by construction because the data arrived as text. To detect the original encoding of a file, paste its bytes with input_form=base64 or hex.`;

const CARET_JSON = `{
  "bytes": 17,
  "column_names": [
    "column_1",
    "column_2",
    "column_3"
  ],
  "columns": 3,
  "confidence": 100,
  "data_rows": 3,
  "delimiter": "^",
  "delimiter_name": "caret",
  "delimiter_scores": [
    {
      "columns": 1,
      "consistency": 100,
      "delimiter": ",",
      "name": "comma"
    },
    {
      "columns": 1,
      "consistency": 100,
      "delimiter": "\\t",
      "name": "tab"
    },
    {
      "columns": 1,
      "consistency": 100,
      "delimiter": ";",
      "name": "semicolon"
    },
    {
      "columns": 1,
      "consistency": 100,
      "delimiter": "|",
      "name": "pipe"
    },
    {
      "columns": 1,
      "consistency": 100,
      "delimiter": ":",
      "name": "colon"
    },
    {
      "columns": 1,
      "consistency": 100,
      "delimiter": "~",
      "name": "tilde"
    },
    {
      "columns": 1,
      "consistency": 100,
      "delimiter": " ",
      "name": "space"
    },
    {
      "columns": 3,
      "consistency": 100,
      "delimiter": "^",
      "name": "caret"
    }
  ],
  "encoding": {
    "bom": false,
    "label": "utf-8",
    "source": "declared: input pasted as text"
  },
  "format": "delimited",
  "format_label": "Delimited text",
  "header": "unknown",
  "line_ending": "lf",
  "lines": 3,
  "notes": [
    "Encoding is UTF-8 by construction because the data arrived as text. To detect the original encoding of a file, paste its bytes with input_form=base64 or hex.",
    "Column types and header detection are off (detect_types=false)."
  ],
  "quote_char": null,
  "sampled_lines": 3
}`;

const PARQUET_JSON = `{
  "bytes": 10,
  "confidence": 100,
  "encoding": {
    "bom": false,
    "label": "binary",
    "source": "magic bytes"
  },
  "format": "parquet",
  "format_label": "Apache Parquet",
  "notes": [
    "The PAR1 marker is present at both the start and the end of the input.",
    "Binary containers are identified by their signature only; field-level structure is not parsed here."
  ]
}`;

test('data-format-sniffer detects CSV with exact report output', async ({ page }) => {
  await page.goto('/tools/data-format-sniffer/');
  await page.fill('#in-data', CSV);

  await expect(page.locator('#tool-output')).toHaveText(CSV_REPORT, { timeout: 15_000 });
});

test('data-format-sniffer deep-link toggles non-default controls for custom delimiter JSON', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'a^b^c\n1^2^3\n4^5^6',
    input_form: 'text',
    sample_lines: '100',
    extra_delimiters: '^',
    comment_prefix: '',
    detect_types: 'false',
    preview_rows: '0',
    output: 'json',
  });

  await page.goto(`/tools/data-format-sniffer/?${params.toString()}`);
  await expect(page.locator('#in-extra_delimiters')).toHaveValue('^');
  await expect(page.locator('#in-detect_types')).not.toBeChecked();
  await expect(page.locator('#in-preview_rows')).toHaveValue('0');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText(CARET_JSON, { timeout: 15_000 });
});

test('data-format-sniffer detects parquet bytes from hex input', async ({ page }) => {
  await page.goto('/tools/data-format-sniffer/');
  await page.fill('#in-data', '50 41 52 31 00 00 50 41 52 31');
  await page.selectOption('#in-input_form', 'hex');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toHaveText(PARQUET_JSON, { timeout: 15_000 });
});
