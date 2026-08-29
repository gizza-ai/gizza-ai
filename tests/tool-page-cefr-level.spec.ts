import { test, expect } from './fixtures';

// /tools/cefr-level/ estimates English CEFR difficulty in-browser (pure wasm).
test('cefr-level reports an A1 summary for simple learner text', async ({ page }) => {
  await page.goto('/tools/cefr-level/');
  await page.fill('#in-text', 'I like my family. We go to school and read a book.');
  await page.fill('#in-coverage', '90');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('CEFR estimate: A1', { timeout: 15000 });
  await expect(out).toContainText('Vocabulary: A1 at 90% coverage');
  await expect(out).toContainText('Target B1: 0 word(s) above target');
});

test('cefr-level deep link can show words above a target level', async ({ page }) => {
  await page.goto('/tools/cefr-level/?text=Nevertheless%2C%20the%20methodology%20has%20significant%20implications%20for%20sustainability.&output=table&target=B1&coverage=90&unknown=estimate&proper_nouns=true');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('count\tlevel\tword\tabove_target', { timeout: 15000 });
  await expect(out).toContainText('methodology');
  await expect(out).toContainText('true');
});
