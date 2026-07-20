import { test, expect } from './fixtures';

// /tools/regex-to-json/ parses each line with a named-capture regex and emits
// JSON objects keyed by group name (pure wasm, in-browser).

// Big fixtures: page.fill routes through insertText and is minutes-slow on MB
// inputs — set the value directly and dispatch the driver's "input" event.
async function setBigValue(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('regex-to-json parses lines into a pretty JSON array (exact)', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.fill('#in-text', 'a=1\nb=2');
  await page.fill('#in-pattern', '(?<key>\\w+)=(?<value>\\d+)');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"key"', { timeout: 15000 });
  // Multi-line exact output: toHaveText normalizes whitespace, compare textContent.
  const text = await out.textContent();
  expect(text).toBe(
    '[\n' +
      '  {\n    "key": "a",\n    "value": "1"\n  },\n' +
      '  {\n    "key": "b",\n    "value": "2"\n  }\n' +
      ']'
  );
});

test('regex-to-json ndjson output emits one object per line', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.fill('#in-text', 'a=1\nb=2');
  await page.fill('#in-pattern', '(?<key>\\w+)=(?<value>\\d+)');
  await page.selectOption('#in-output', 'ndjson');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"key"', { timeout: 15000 });
  expect(await out.textContent()).toBe(
    '{"key":"a","value":"1"}\n{"key":"b","value":"2"}'
  );
});

test('regex-to-json unmatched=keep emits _raw records', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.fill('#in-text', 'a=1\noops\nb=2');
  await page.fill('#in-pattern', '^(?<key>\\w+)=(?<value>\\d+)$');
  await page.selectOption('#in-unmatched', 'keep');
  await page.selectOption('#in-output', 'compact');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '[{"key":"a","value":"1"},{"_raw":"oops"},{"key":"b","value":"2"}]',
    { timeout: 15000 }
  );
});

test('regex-to-json unmatched=fail reports the offending line', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.fill('#in-text', 'a=1\noops');
  await page.fill('#in-pattern', '^(?<key>\\w+)=(?<value>\\d+)$');
  await page.selectOption('#in-unmatched', 'fail');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('line 2 does not match the pattern: oops', {
    timeout: 15000,
  });
});

test('regex-to-json all_matches + coerce_types (non-default checkboxes)', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.fill('#in-text', 'host=web1 latency=42 ok=true');
  await page.fill('#in-pattern', '(?<key>\\w+)=(?<value>\\S+)');
  await page.check('#in-all_matches');
  await page.check('#in-coerce_types');
  await page.selectOption('#in-output', 'compact');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '[{"key":"host","value":"web1"},{"key":"latency","value":42},{"key":"ok","value":true}]',
    { timeout: 15000 }
  );
});

test('regex-to-json ignore_case + Python-style (?P<name>) groups', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.fill('#in-text', 'error: boom');
  await page.fill('#in-pattern', '(?P<level>ERROR): (?P<message>.*)');
  await page.check('#in-ignore_case');
  await page.selectOption('#in-output', 'compact');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('[{"level":"error","message":"boom"}]', {
    timeout: 15000,
  });
});

test('regex-to-json rejects a pattern without named groups', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.fill('#in-text', 'a=1');
  await page.fill('#in-pattern', '(\\w+)=(\\d+)');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('no named capture groups', { timeout: 15000 });
  await expect(out).toContainText('(?<name>');
});

test('regex-to-json 1 MB cap: exactly at the boundary works, one byte over errors', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  // 'a=1\n' (4 bytes) + 999,996 'x' = exactly 1,000,000 bytes.
  const at = 'a=1\n' + 'x'.repeat(999_996);
  await setBigValue(page, '#in-text', at);
  await page.fill('#in-pattern', '^(?<key>\\w+)=(?<value>\\d+)$');
  await page.selectOption('#in-output', 'compact');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('[{"key":"a","value":"1"}]', { timeout: 20000 });
  // One byte over the cap → actionable error.
  await setBigValue(page, '#in-text', at + 'x');
  await expect(out).toContainText('too large', { timeout: 20000 });
  await expect(out).toContainText('1 MB');
});

test('regex-to-json example chip runs the key=value preset', async ({ page }) => {
  await page.goto('/tools/regex-to-json/');
  await page.click('button.tool-example-chip[data-example="2"]');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '[{"key":"host","value":"web1"},{"key":"region","value":"eu"},{"key":"latency","value":42}]',
    { timeout: 15000 }
  );
});

test('regex-to-json deep-link prefills and runs', async ({ page }) => {
  const q = new URLSearchParams({
    text: 'GET /a 200',
    pattern: '(?<method>[A-Z]+) (?<path>\\S+) (?<status>\\d{3})',
    coerce_types: 'true',
    output: 'compact',
  });
  await page.goto('/tools/regex-to-json/?' + q.toString());
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('[{"method":"GET","path":"/a","status":200}]', {
    timeout: 15000,
  });
});
