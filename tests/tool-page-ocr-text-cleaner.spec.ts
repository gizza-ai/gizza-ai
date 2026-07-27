import { test, expect } from './fixtures';

test('ocr-text-cleaner fixes ligatures, confusables and spacing (defaults)', async ({ page }) => {
  await page.goto('/tools/ocr-text-cleaner/');
  await page.fill('#in-text', 'The ﬁnal  report says HeIIo ,world .Next');
  await expect(page.locator('#tool-output')).toHaveText('The final report says Hello, world. Next', {
    timeout: 15000,
  });
});

test('ocr-text-cleaner reflows a hyphenated, wrapped paragraph', async ({ page }) => {
  await page.goto('/tools/ocr-text-cleaner/');
  await page.fill('#in-text', 'This para-\ngraph was split\nacross lines.');
  await page.selectOption('#in-line_breaks', 'paragraphs');
  await expect(page.locator('#tool-output')).toHaveText('This paragraph was split across lines.', {
    timeout: 15000,
  });
});

test('ocr-text-cleaner "collapse to one line" joins every break', async ({ page }) => {
  await page.goto('/tools/ocr-text-cleaner/');
  await page.fill('#in-text', 'line one\nline two\n\nnew para');
  await page.selectOption('#in-line_breaks', 'all');
  await expect(page.locator('#tool-output')).toHaveText('line one line two new para', {
    timeout: 15000,
  });
});

test('ocr-text-cleaner deep-link pre-fills params and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    text: 'HeIIo ,world .Next',
    fix_ligatures: 'true',
    join_hyphenated: 'true',
    line_breaks: 'keep',
    fix_confusables: 'true',
    fix_rn: 'false',
    fix_spacing: 'true',
  });
  await page.goto(`/tools/ocr-text-cleaner/?${params.toString()}`);
  await expect(page.locator('#in-line_breaks')).toHaveValue('keep');
  await expect(page.locator('#tool-output')).toHaveText('Hello, world. Next', { timeout: 15000 });
});
