import { test, expect } from './fixtures';

// /tools/regex-tester/ tests a regex in-browser (pure wasm) and reports every
// match with positions plus the value/span of each capture group. text is a
// multiline <textarea>; pattern is a field; ignore_case/multiline/dotall are
// checkboxes.
test('regex-tester reports matches with positions', async ({ page }) => {
  await page.goto('/tools/regex-tester/');
  await page.fill('#in-text', 'a1 b2 c3');
  await page.fill('#in-pattern', '\\d');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 matches', { timeout: 15000 });
  await expect(out).toContainText('Match 1 at 1');
});

test('regex-tester breaks out named and numbered capture groups', async ({ page }) => {
  await page.goto('/tools/regex-tester/');
  await page.fill('#in-text', '2024-01-15');
  await page.fill('#in-pattern', '(?<year>\\d{4})-(\\d{2})-(?<day>\\d{2})');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 capture groups', { timeout: 15000 });
  await expect(out).toContainText('named: year, day');
  await expect(out).toContainText('Group 1 (year)');
  await expect(out).toContainText('Group 3 (day)');
});

test('regex-tester honours the ignore-case flag', async ({ page }) => {
  await page.goto('/tools/regex-tester/');
  await page.fill('#in-text', 'Cat cat CAT');
  await page.fill('#in-pattern', 'cat');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1 match', { timeout: 15000 });
  await page.check('#in-ignore_case');
  await expect(out).toContainText('3 matches', { timeout: 15000 });
});

test('regex-tester reports an invalid pattern', async ({ page }) => {
  await page.goto('/tools/regex-tester/');
  await page.fill('#in-text', 'abc');
  await page.fill('#in-pattern', 'a(');
  await expect(page.locator('#tool-output')).toContainText('invalid regular expression', { timeout: 15000 });
});
