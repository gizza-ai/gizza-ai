import { test, expect } from './fixtures';

const HTML =
  '<a href="https://example.com/a">Example A</a> <a href="/local/page">local</a> <a href="https://example.com/a">dup</a> <h2 id="intro">Intro</h2>';

const MD = '# Getting Started\n\nSee [Example A](https://example.com/a) and [rel](./x.md).';

// /tools/link-extractor/ parses pasted HTML or Markdown in-browser (pure wasm via
// scraper + pulldown-cmark) and lists hyperlinks + in-page anchors.
test('link-extractor page extracts HTML links and anchors', async ({ page }) => {
  await page.goto('/tools/link-extractor/');
  await page.fill('#in-input', HTML);
  await page.selectOption('#in-source', 'html');
  await expect(page.locator('#tool-output')).toContainText('https://example.com/a', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('[relative]');
  await expect(page.locator('#tool-output')).toContainText('#intro');
});

test('link-extractor page resolves a base URL and dedups', async ({ page }) => {
  await page.goto('/tools/link-extractor/');
  await page.fill('#in-input', HTML);
  await page.selectOption('#in-source', 'html');
  await page.fill('#in-base_url', 'https://example.com/docs/');
  await page.check('#in-dedup');
  const out = page.locator('#tool-output');
  // relative /local/page is resolved to absolute, and the duplicate is gone.
  await expect(out).toContainText('https://example.com/local/page', { timeout: 15000 });
  await expect(out).toContainText('2 link(s)');
});

test('link-extractor page parses Markdown via deep-link to JSON', async ({ page }) => {
  const qs = '?input=' + encodeURIComponent(MD) + '&source=markdown&output=json';
  await page.goto('/tools/link-extractor/' + qs);
  await expect(page.locator('#in-input')).toHaveValue(MD, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"link_count": 2');
  await expect(page.locator('#tool-output')).toContainText('getting-started');
});
