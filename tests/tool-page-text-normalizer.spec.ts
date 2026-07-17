import { test, expect } from './fixtures';

test('text-normalizer collapses whitespace by default', async ({ page }) => {
  await page.goto('/tools/text-normalizer/');
  await page.fill('#in-text', '  hello   world  ');
  await expect(page.locator('#tool-output')).toHaveText('hello world', { timeout: 15000 });
});

test('text-normalizer NLP prep: NFKC + fold + strip accents + straighten punctuation', async ({ page }) => {
  await page.goto('/tools/text-normalizer/');
  await page.fill('#in-text', '  Café  “RÉSUMÉ”…  ');
  await page.selectOption('#in-form', 'nfkc');
  await page.selectOption('#in-case', 'fold');
  await page.check('#in-strip_accents');
  await page.check('#in-punctuation');
  await expect(page.locator('#tool-output')).toHaveText('cafe "resume"...', { timeout: 15000 });
});

test('text-normalizer flattens fancy fonts with NFKC', async ({ page }) => {
  await page.goto('/tools/text-normalizer/');
  await page.fill('#in-text', '\u{1D407}\u{1D41E}\u{1D425}\u{1D425}\u{1D428} ﬁle ＡＢＣ');
  await page.selectOption('#in-form', 'nfkc');
  await expect(page.locator('#tool-output')).toHaveText('Hello file ABC', { timeout: 15000 });
});

test('text-normalizer deep-links pre-fill and auto-run', async ({ page }) => {
  await page.goto('/tools/text-normalizer/?text=hello%20%20%20world&case=upper');
  await expect(page.locator('#tool-output')).toHaveText('HELLO WORLD', { timeout: 15000 });
});
