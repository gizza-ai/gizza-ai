import { test, expect } from './fixtures';

const sample = [
  'https://example.com/blog',
  'https://example.com/blog/about/',
  'https://example.com/sitemap.xml',
  'https://example.com/search?q=a%20b#top',
  'https://example.com',
].join('\n');

test('url-trailing-slash-normalizer adds slashes and preserves file paths, queries and fragments', async ({ page }) => {
  await page.goto('/tools/url-trailing-slash-normalizer/');
  await page.fill('#in-urls', sample);
  await expect(page.locator('#tool-output')).toHaveText([
    'https://example.com/blog/',
    'https://example.com/blog/about/',
    'https://example.com/sitemap.xml',
    'https://example.com/search/?q=a%20b#top',
    'https://example.com/',
  ].join('\n'), { timeout: 15_000 });
});

test('url-trailing-slash-normalizer deep-link removes slashes, keeps root normalized, and can drop invalid lines', async ({ page }) => {
  const qs =
    '?urls=' + encodeURIComponent('https://example.com/blog/\nnot a url\nhttps://example.com/') +
    '&mode=remove&skip_file_paths=true&normalize_root=true&dedupe=false&on_invalid=drop&output=urls';
  await page.goto('/tools/url-trailing-slash-normalizer/' + qs);
  await expect(page.locator('#in-urls')).toHaveValue('https://example.com/blog/\nnot a url\nhttps://example.com/');
  await expect(page.locator('#in-mode')).toHaveValue('remove');
  await expect(page.locator('#in-skip_file_paths')).toBeChecked();
  await expect(page.locator('#in-normalize_root')).toBeChecked();
  await expect(page.locator('#in-dedupe')).not.toBeChecked();
  await expect(page.locator('#in-on_invalid')).toHaveValue('drop');
  await expect(page.locator('#tool-output')).toHaveText('https://example.com/blog\nhttps://example.com/', { timeout: 15_000 });
});

test('url-trailing-slash-normalizer report output and non-default checkboxes are wired', async ({ page }) => {
  await page.goto('/tools/url-trailing-slash-normalizer/');
  await page.fill('#in-urls', 'https://example.com/a\nhttps://example.com/a/\nhttps://example.com/file.txt');
  await page.selectOption('#in-mode', 'add');
  await page.uncheck('#in-skip_file_paths');
  await page.uncheck('#in-normalize_root');
  await page.check('#in-dedupe');
  await page.selectOption('#in-output', 'report');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('line,original,normalized,action', { timeout: 15_000 });
  await expect(output).toContainText('1,https://example.com/a,https://example.com/a/,added');
  await expect(output).toContainText('2,https://example.com/a/,https://example.com/a/,duplicate');
  await expect(output).toContainText('3,https://example.com/file.txt,https://example.com/file.txt/,added');
});

test('url-trailing-slash-normalizer preset chips and generated CLI example are generic', async ({ page }) => {
  await page.goto('/tools/url-trailing-slash-normalizer/');
  await page.getByRole('button', { name: 'Deduplicate a mixed list' }).click();
  await expect(page.locator('#in-dedupe')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('summary');
  await expect(page.locator('#tool-output')).toContainText('duplicates_removed,2', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool url-trailing-slash-normalizer');
  expect(cli).toContain('https://example.com/blog');
  expect(cli).toContain('https://example.com/sitemap.xml');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
