import { test, expect } from './fixtures';

test('citation-generator page formats an MLA article', async ({ page }) => {
  await page.goto('/tools/citation-generator/');

  await page.selectOption('#in-style', 'mla');
  await page.fill('#in-title', 'On the nature of things');
  await page.fill('#in-authors', 'Doe, Jane; Smith, John');
  await page.fill('#in-year', '2021');
  await page.fill('#in-container', 'Journal of Studies');
  await page.fill('#in-volume', '12');
  await page.fill('#in-issue', '3');
  await page.fill('#in-pages', '45-67');
  await page.fill('#in-doi', '10.1000/xyz');

  await expect(page.locator('#tool-output')).toHaveText(
    'Doe, Jane, and John Smith. "On the Nature of Things." Journal of Studies, vol. 12, no. 3, 2021, pp. 45-67. https://doi.org/10.1000/xyz.',
    { timeout: 15000 },
  );
});

test('citation-generator page formats an APA book', async ({ page }) => {
  await page.goto('/tools/citation-generator/');

  // style default is apa (select first option).
  await page.fill('#in-title', 'A Study of Things');
  await page.fill('#in-authors', 'Doe, Jane');
  await page.fill('#in-year', '2020');
  await page.fill('#in-publisher', 'Penguin');

  await expect(page.locator('#tool-output')).toHaveText(
    'Doe, J. (2020). A study of things. Penguin.',
    { timeout: 15000 },
  );
});

test('citation-generator page formats a Harvard book', async ({ page }) => {
  await page.goto('/tools/citation-generator/');

  await page.selectOption('#in-style', 'harvard');
  await page.fill('#in-title', 'A Study of Things');
  await page.fill('#in-authors', 'Doe, Jane');
  await page.fill('#in-year', '2020');
  await page.fill('#in-publisher', 'Penguin');

  await expect(page.locator('#tool-output')).toHaveText(
    'Doe, J. (2020) A Study of Things. Penguin.',
    { timeout: 15000 },
  );
});
