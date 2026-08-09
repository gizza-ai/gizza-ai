import { test, expect } from './fixtures';

// /tools/regex-capture-to-csv/ scans text with a regex and emits one CSV row
// per match, capture groups as columns (pure wasm, in-browser).

// Big fixtures: page.fill routes through insertText and is minutes-slow on MB
// inputs — set the value directly and dispatch the driver's "input" event.
async function setBigValue(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('named groups become CSV columns (exact output)', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'alice 30\nbob 41');
  await page.fill('#in-pattern', '(?<name>[a-z]+) (?<age>\\d+)');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('name,age', { timeout: 15000 });
  // Multi-line exact output: toHaveText normalizes whitespace, compare textContent.
  expect(await out.textContent()).toBe('name,age\nalice,30\nbob,41');
});

test('columns selects + reorders, single-character delimiter', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'alice 30\nbob 41');
  await page.fill('#in-pattern', '(?<name>[a-z]+) (?<age>\\d+)');
  await page.fill('#in-columns', 'age, name');
  await page.fill('#in-delimiter', ';');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('age;name', { timeout: 15000 });
  expect(await out.textContent()).toBe('age;name\n30;alice\n41;bob');
});

test('quoting=all with the header row unchecked (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'alice 30');
  await page.fill('#in-pattern', '(?<name>[a-z]+) (?<age>\\d+)');
  // header defaults to checked — exercise the OFF path.
  await page.uncheck('#in-header');
  await page.selectOption('#in-quoting', 'all');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('"alice","30"', { timeout: 15000 });
});

test('tab delimiter keyword + CRLF line endings', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'alice 30\nbob 41');
  await page.fill('#in-pattern', '(?<name>[a-z]+) (?<age>\\d+)');
  await page.fill('#in-delimiter', 'tab');
  await page.selectOption('#in-line_ending', 'crlf');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('name\tage', { timeout: 15000 });
  expect(await out.textContent()).toBe(
    'name\tage\r\nalice\t30\r\nbob\t41'
  );
});

test('RFC 4180 quoting of commas and embedded quotes', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'say "hi", now');
  await page.fill('#in-pattern', '(?<phrase>say .*now)');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('phrase', { timeout: 15000 });
  expect(await out.textContent()).toBe('phrase\n"say ""hi"", now"');
});

test('dotall lets one row span lines (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', '<td>one\ntwo</td>');
  await page.fill('#in-pattern', '<td>(?<cell>.+?)</td>');
  await page.check('#in-dotall');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('cell', { timeout: 15000 });
  expect(await out.textContent()).toBe('cell\n"one\ntwo"');
});

test('ignore_case + unique + sort combine', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'B 2\na 1\nb 2');
  await page.fill('#in-pattern', '(?<k>[a-z]) (?<v>\\d)');
  await page.check('#in-ignore_case');
  await page.check('#in-unique');
  await page.check('#in-sort');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('k,v', { timeout: 15000 });
  expect(await out.textContent()).toBe('k,v\nB,2\na,1\nb,2');
});

test('unnamed groups fall back to column1/column2', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'a=1 b=2');
  await page.fill('#in-pattern', '(\\w)=(\\d)');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('column1,column2', { timeout: 15000 });
  expect(await out.textContent()).toBe('column1,column2\na,1\nb,2');
});

test('an unknown column name lists the available ones', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'alice 30');
  await page.fill('#in-pattern', '(?<name>[a-z]+) (?<age>\\d+)');
  await page.fill('#in-columns', 'nope');
  const out = page.locator('#tool-output');
  await expect(out).toContainText("unknown column 'nope'", { timeout: 15000 });
  await expect(out).toContainText('name, age');
});

test('a pattern that matches nothing reports it', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.fill('#in-text', 'abc');
  await page.fill('#in-pattern', '(?<n>\\d+)');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('no matches', { timeout: 15000 });
});

test('1 MB cap: exactly at the boundary works, one byte over errors', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  // 'a=1\n' (4 bytes) + 999,996 'x' = exactly 1,000,000 bytes.
  const at = 'a=1\n' + 'x'.repeat(999_996);
  await setBigValue(page, '#in-text', at);
  await page.fill('#in-pattern', '(?<key>\\w)=(?<value>\\d)');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('key,value a,1', { timeout: 20000 });
  // One byte over the cap → actionable error.
  await setBigValue(page, '#in-text', at + 'x');
  await expect(out).toContainText('the limit is 1000000 bytes', { timeout: 20000 });
  await expect(out).toContainText('1 MB');
});

test('example chip runs the key=value dedupe+sort preset', async ({ page }) => {
  await page.goto('/tools/regex-capture-to-csv/');
  await page.click('button.tool-example-chip[data-example="1"]');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('key,value', { timeout: 15000 });
  expect(await out.textContent()).toBe(
    'key,value\nhost,db2\nhost,web1\nregion,eu\nregion,us'
  );
});

test('deep-link prefills and runs', async ({ page }) => {
  const q = new URLSearchParams({
    text: 'GET /a 200',
    pattern: '(?<method>[A-Z]+) (?<path>\\S+) (?<status>\\d{3})',
    columns: 'status, method',
    delimiter: 'pipe',
  });
  await page.goto('/tools/regex-capture-to-csv/?' + q.toString());
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('status|method 200|GET', { timeout: 15000 });
});
