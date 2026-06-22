import { test, expect } from './fixtures';

test('parse-query-string page parses pairs and structured view', async ({ page }) => {
  await page.goto('/tools/parse-query-string/');

  await page.fill('#in-query', 'name=John+Doe&color=red&color=blue&user[age]=30');
  const out = page.locator('#tool-output');
  // Ordered pairs (4 pairs; + decoded to a space by default).
  await expect(out).toContainText('Parameters (4):', { timeout: 15000 });
  await expect(out).toContainText('name = John Doe');
  // Structured: repeated key -> array, bracket -> nested object.
  await expect(out).toContainText('"color"');
  await expect(out).toContainText('"red"');
  await expect(out).toContainText('"age": "30"');
});

test('parse-query-string keeps + literal when checkbox unchecked', async ({ page }) => {
  await page.goto('/tools/parse-query-string/');
  await page.fill('#in-query', 'q=a+b');
  // Default checked = + as space.
  await expect(page.locator('#tool-output')).toContainText('q = a b', { timeout: 15000 });
  // Uncheck -> + kept literal.
  await page.uncheck('#in-plus_as_space');
  await expect(page.locator('#tool-output')).toContainText('q = a+b', { timeout: 15000 });
});

test('parse-query-string strips a leading ? and handles empty bracket arrays', async ({ page }) => {
  await page.goto('/tools/parse-query-string/');
  await page.fill('#in-query', '?tag[]=x&tag[]=y');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Parameters (2):', { timeout: 15000 });
  await expect(out).toContainText('"x"');
  await expect(out).toContainText('"y"');
});
