import { test, expect } from './fixtures';

test('bibtex-format page pretty-prints an entry', async ({ page }) => {
  await page.goto('/tools/bibtex-format/');

  await page.fill(
    '#in-bibtex',
    '@ARTICLE{einstein1905, title={Zur Elektrodynamik}, author="Einstein, Albert", year=1905}',
  );
  // Default: sort=none, lowercase_type checkbox is checked (default true).
  await expect(page.locator('#tool-output')).toHaveText(
    '@article{einstein1905,\n  title = {Zur Elektrodynamik},\n  author = "Einstein, Albert",\n  year = 1905\n}',
    { timeout: 15000 },
  );
});

test('bibtex-format page sorts entries by key', async ({ page }) => {
  await page.goto('/tools/bibtex-format/');
  await page.fill('#in-bibtex', '@book{zeta, t={Z}}\n@article{alpha, t={A}}');
  await page.selectOption('#in-sort', 'key');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('@article{alpha', { timeout: 15000 });
  const text = await out.innerText();
  expect(text.indexOf('alpha')).toBeLessThan(text.indexOf('zeta'));
});

test('bibtex-format page aligns the = signs', async ({ page }) => {
  await page.goto('/tools/bibtex-format/');
  await page.fill('#in-bibtex', '@article{k, a={1}, title={2}}');
  await page.check('#in-align_values');
  await expect(page.locator('#tool-output')).toContainText('a     = {1}', {
    timeout: 15000,
  });
});

test('bibtex-format page lowercase_type off keeps @TYPE casing', async ({ page }) => {
  await page.goto('/tools/bibtex-format/');
  await page.fill('#in-bibtex', '@ARTICLE{k, title={X}}');
  // Default checkbox is checked (lowercase_type=true) -> uncheck for the off path.
  await page.uncheck('#in-lowercase_type');
  await expect(page.locator('#tool-output')).toContainText('@ARTICLE{k,', {
    timeout: 15000,
  });
});

test('bibtex-format flags a duplicate cite key by default', async ({ page }) => {
  await page.goto('/tools/bibtex-format/');
  // check_duplicates checkbox is checked by default (default true).
  await page.fill('#in-bibtex', '@article{k, x={1}}\n@book{K, y={2}}');
  await expect(page.locator('#tool-output')).toContainText('duplicate cite key', {
    timeout: 15000,
  });
  // Unchecking lets it format both entries.
  await page.uncheck('#in-check_duplicates');
  await expect(page.locator('#tool-output')).toContainText('@book{K,', {
    timeout: 15000,
  });
});

test('bibtex-format query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/bibtex-format/?bibtex=' +
      encodeURIComponent('@misc{x, note={n}}') +
      '&sort=key',
  );
  await expect(page.locator('#in-bibtex')).toHaveValue('@misc{x, note={n}}', {
    timeout: 15000,
  });
  await expect(page.locator('#in-sort')).toHaveValue('key');
  await expect(page.locator('#tool-output')).toContainText('@misc{x,', {
    timeout: 15000,
  });
});
