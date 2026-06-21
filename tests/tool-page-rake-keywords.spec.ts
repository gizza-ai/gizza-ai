import { test, expect } from './fixtures';

test('rake-keywords page extracts ranked keyphrases', async ({ page }) => {
  await page.goto('/tools/rake-keywords/');
  await page.fill(
    '#in-text',
    'Compatibility of systems of linear constraints over the set of natural numbers. ' +
      'Criteria of compatibility of a system of linear Diophantine equations, strict ' +
      'inequations, and nonstrict inequations are considered.'
  );
  await page.fill('#in-top_n', '5');
  await page.fill('#in-max_words', '0');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('linear diophantine equations', { timeout: 15000 });
  await expect(out).toContainText('keyphrase');
});
