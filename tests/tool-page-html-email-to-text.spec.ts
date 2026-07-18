import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const NEWSLETTER = '<h2>This week</h2><p>Read the <a href="https://example.com/blog">new blog</a>.</p>';

test('html-email-to-text renders inline links by default', async ({ page }) => {
  await page.goto('/tools/html-email-to-text/');
  await page.fill('#in-html', NEWSLETTER);
  await expect(page.locator('#tool-output')).toHaveText(
    'This week\n\nRead the new blog (https://example.com/blog).',
    { timeout: 15000 },
  );
});

test('html-email-to-text supports footnote link mode', async ({ page }) => {
  await page.goto('/tools/html-email-to-text/');
  await page.fill('#in-html', '<p>See the <a href="https://example.com/docs">docs</a> and <a href="https://example.com/pricing">pricing</a>.</p>');
  await page.selectOption('#in-links', 'footnote');
  await expect(page.locator('#tool-output')).toHaveText(
    'See the docs[1] and pricing[2].\n\n[1] https://example.com/docs\n[2] https://example.com/pricing',
    { timeout: 15000 },
  );
});

test('html-email-to-text drops URLs in text-only mode', async ({ page }) => {
  await page.goto('/tools/html-email-to-text/');
  await page.fill('#in-html', '<p>Please <a href="https://example.com/x">click here</a>.</p>');
  await page.selectOption('#in-links', 'text');
  await expect(page.locator('#tool-output')).toHaveText('Please click here.', { timeout: 15000 });
});

test('html-email-to-text wrap boundary keeps every line within the requested width', async ({ page }) => {
  await page.goto('/tools/html-email-to-text/');
  await page.fill('#in-html', '<p>one two three four five six seven eight nine ten</p>');
  await page.fill('#in-wrap', '12');
  await expect(page.locator('#tool-output')).toContainText('\n', { timeout: 15000 });
  const out = await output(page);
  for (const line of out.split('\n')) {
    expect(line.length).toBeLessThanOrEqual(12);
  }
});

test('html-email-to-text deep-links pre-fill params and auto-run', async ({ page }) => {
  const params = new URLSearchParams({
    html: '<p>See <a href="https://example.com/docs">docs</a>.</p>',
    links: 'footnote',
    wrap: '0',
  });
  await page.goto(`/tools/html-email-to-text/?${params.toString()}`);
  await expect(page.locator('#in-html')).toHaveValue('<p>See <a href="https://example.com/docs">docs</a>.</p>', { timeout: 15000 });
  await expect(page.locator('#in-links')).toHaveValue('footnote');
  await expect(page.locator('#tool-output')).toHaveText('See docs[1].\n\n[1] https://example.com/docs', { timeout: 15000 });
});
