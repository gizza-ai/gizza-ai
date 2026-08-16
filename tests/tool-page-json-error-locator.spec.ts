import { test, expect } from './fixtures';

const tool = '/tools/json-error-locator/';
const trailingComma = '{\n  "name": "Ada",\n  "tags": [1, 2,]\n}';
const unquotedKey = '{\n  name: "Ada",\n  "ok": true\n}';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('json-error-locator page reports a trailing comma with exact position and fix', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-json', trailingComma);
  await page.selectOption('#in-output', 'report');
  await page.fill('#in-context_lines', '2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Invalid JSON — 1 issue found.', { timeout: 15000 });
  await expect(out).toContainText('Line 3, column 16');
  await expect(out).toContainText('trailing comma');
  await expect(out).toContainText('JSON does not allow a comma before ]');
  await expect(out).toContainText('|   "tags": [1, 2,]');
  await expect(out).toContainText('|                ^');
});

test('json-error-locator page reports unquoted keys in scan-all mode', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-json', unquotedKey);
  await page.selectOption('#in-output', 'report');
  await page.fill('#in-context_lines', '1');
  await page.check('#in-scan_all');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Invalid JSON — 1 issue found.', { timeout: 15000 });
  await expect(out).toContainText('Line 2, column 3');
  await expect(out).toContainText('unquoted key');
  await expect(out).toContainText('the member name name is not quoted');
  await expect(out).toContainText('write "name": instead of name:');
});

test('json-error-locator query-param deep-link supports JSON output and parser-style first issue', async ({ page }) => {
  await page.goto(
    tool +
      '?json=' +
      encodeURIComponent('[1, 2, ]') +
      '&output=json&context_lines=0&scan_all=false',
  );

  await expect(page.locator('#in-json')).toHaveValue('[1, 2, ]', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-context_lines')).toHaveValue('0');
  await expect(page.locator('#in-scan_all')).not.toBeChecked();

  const parsed = JSON.parse(await outputText(page));
  expect(parsed.valid).toBe(false);
  expect(parsed.issue_count).toBe(1);
  expect(parsed.issues[0].kind).toBe('trailing-comma');
  expect(parsed.issues[0].line).toBe(1);
  expect(parsed.issues[0].column).toBe(6);
  expect(parsed.issues[0].context).toBeUndefined();
});
