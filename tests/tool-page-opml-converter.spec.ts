import { test, expect } from './fixtures';

const OPML = `<opml version="2.0"><head><title>My Subs</title></head><body><outline text="Tech"><outline type="rss" text="Daily News" xmlUrl="https://news.example.org/feed"/></outline></body></opml>`;

test('opml-converter converts OPML to JSON with default settings', async ({ page }) => {
  await page.goto('/tools/opml-converter/');
  // Defaults: from=opml, to=json, pretty checked.
  await page.fill('#in-input', OPML);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"title": "My Subs"', { timeout: 15000 });
  await expect(out).toContainText('"text": "Tech"');
  await expect(out).toContainText('"xmlUrl": "https://news.example.org/feed"');
});

test('opml-converter converts OPML to CSV with a category column', async ({ page }) => {
  await page.goto('/tools/opml-converter/?to=csv');
  await page.fill('#in-input', OPML);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('category,', { timeout: 15000 });
  // The feed carries its folder as the category.
  await expect(out).toContainText('Tech,Daily News');
});

test('opml-converter round-trips JSON back to OPML', async ({ page }) => {
  await page.goto('/tools/opml-converter/?from=json&to=opml');
  await page.fill('#in-input', '{"title":"My Subs","outlines":[{"type":"rss","text":"Daily News","xmlUrl":"https://news.example.org/feed"}]}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<title>My Subs</title>', { timeout: 15000 });
  await expect(out).toContainText('xmlUrl="https://news.example.org/feed"');
});

test('opml-converter compact output drops indentation when pretty is off', async ({ page }) => {
  await page.goto('/tools/opml-converter/');
  await page.fill('#in-input', OPML);
  // pretty is a default-true checkbox — uncheck it for compact single-line JSON.
  await page.uncheck('#in-pretty');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"title":"My Subs"', { timeout: 15000 });
});

test('opml-converter query-param deep-link prefills the controls', async ({ page }) => {
  await page.goto('/tools/opml-converter/?from=json&to=opml');
  await expect(page.locator('#in-from')).toHaveValue('json', { timeout: 15000 });
  await expect(page.locator('#in-to')).toHaveValue('opml');
});
