import { test, expect } from './fixtures';

// /tools/html-preview-bundler/ bundles HTML+CSS+JS in-browser (pure wasm).
test('bundles a fragment into a full document', async ({ page }) => {
  await page.goto('/tools/html-preview-bundler/');
  await page.fill('#in-html', '<h1>Hi</h1>');
  await page.fill('#in-css', 'h1{color:red}');
  await page.fill('#in-js', 'console.log(1)');
  await page.fill('#in-title', 'Demo');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<!DOCTYPE html>', { timeout: 15000 });
  await expect(out).toContainText('<title>Demo</title>');
  await expect(out).toContainText('h1{color:red}');
  await expect(out).toContainText('<h1>Hi</h1>');
  await expect(out).toContainText('console.log(1)');
});

test('injects into an existing full document', async ({ page }) => {
  await page.goto('/tools/html-preview-bundler/');
  await page.fill(
    '#in-html',
    '<!DOCTYPE html><html><head><title>T</title></head><body><p>x</p></body></html>',
  );
  await page.fill('#in-css', '.a{}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<title>T</title>', { timeout: 15000 });
  await expect(out).toContainText('<style>');
});
