import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('fixed-width-to-csv page auto-detects columns and emits exact CSV', async ({ page }) => {
  await page.goto('/tools/fixed-width-to-csv/');
  await page.fill('#in-text', 'name      age city\nAda        36 London\nBo          7 Oslo');

  await expect(page.locator('#tool-output')).toContainText('Ada,36,London', { timeout: 15_000 });
  expect(await output(page)).toBe('name,age,city\nAda,36,London\nBo,7,Oslo');
});

test('fixed-width-to-csv deep link uses named spec, delimiter enum, and header names', async ({ page }) => {
  const qs = new URLSearchParams({
    text: 'Ada        36 London\nBo          7 Oslo',
    spec: 'name:10,age:4,city:*',
    header: 'names',
    delimiter: 'semicolon',
  });
  await page.goto(`/tools/fixed-width-to-csv/?${qs.toString()}`);

  await expect(page.locator('#in-header')).toHaveValue('names', { timeout: 15_000 });
  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon');
  expect(await output(page)).toBe('name;age;city\nAda;36;London\nBo;7;Oslo');
});

test('fixed-width-to-csv page supports checkboxes, comments, tab output, and CRLF', async ({ page }) => {
  await page.goto('/tools/fixed-width-to-csv/');
  await page.fill('#in-text', 'REPORT 2026\n# generated\nAda  36\n\nBo    7');
  await page.selectOption('#in-header', 'none');
  await page.fill('#in-skip_lines', '1');
  await page.fill('#in-comment', '#');
  await page.fill('#in-delimiter', 'tab');
  await page.selectOption('#in-newline', 'crlf');
  await page.uncheck('#in-skip_blank');

  await expect(page.locator('#tool-output')).toContainText('Ada\t36', { timeout: 15_000 });
  expect(await output(page)).toBe('Ada\t36\r\n\t\r\nBo\t7');
});

test('fixed-width-to-csv page reports invalid specs', async ({ page }) => {
  await page.goto('/tools/fixed-width-to-csv/');
  await page.fill('#in-text', 'Ada  36');
  await page.fill('#in-spec', '0-4');
  await page.selectOption('#in-header', 'none');

  await expect(page.locator('#tool-output')).toContainText('1-based', { timeout: 15_000 });
});
