import { test, expect } from './fixtures';

// /tools/readability-score/ grades text in-browser (pure wasm).
// The text field is a multiline <textarea>.
test('readability-score page reports readability indices', async ({ page }) => {
  await page.goto('/tools/readability-score/');
  await page.fill(
    '#in-text',
    'The cat sat on the mat. The dog ran in the sun. It was a good day.'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Flesch Reading Ease:', { timeout: 15000 });
  await expect(out).toContainText('Flesch-Kincaid Grade:');
  await expect(out).toContainText('Gunning-Fog Index:');
  await expect(out).toContainText('SMOG Index:');
  await expect(out).toContainText('Reading level:');
});
