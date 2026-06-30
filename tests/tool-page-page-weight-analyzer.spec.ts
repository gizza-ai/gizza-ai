import { test, expect } from './fixtures';

const HTML = `<!doctype html>
<html>
<head>
  <link rel="stylesheet" href="/main.css">
  <link rel="stylesheet" href="/print.css" media="print">
  <script src="/analytics.js"></script>
  <script src="/app.js" defer></script>
  <style>body{color:red}</style>
</head>
<body>
  <img src="hero.jpg">
  <img src="below.jpg" loading="lazy">
  <script>console.log(1)</script>
</body>
</html>`;

test('page-weight-analyzer reports scripts, styles, requests, and estimates', async ({ page }) => {
  await page.goto('/tools/page-weight-analyzer/');
  await page.fill('#in-html', HTML);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Page Weight Analysis', { timeout: 15000 });
  await expect(out).toContainText('External: 2  (parser-blocking: 1');
  await expect(out).toContainText('External <link>: 2  (render-blocking: 1)');
  await expect(out).toContainText('Images:         2  (lazy-loaded: 1)');
  await expect(out).toContainText('Estimated network requests:');
});

test('page-weight-analyzer can list external resource URLs', async ({ page }) => {
  await page.goto('/tools/page-weight-analyzer/?list_resources=true');
  await page.fill('#in-html', HTML);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('External resources', { timeout: 15000 });
  await expect(out).toContainText('Scripts (2)');
  await expect(out).toContainText('- /analytics.js');
  await expect(out).toContainText('Stylesheets (2)');
  await expect(out).toContainText('- /main.css');
  await expect(out).toContainText('Images (2)');
  await expect(out).toContainText('- hero.jpg');
});

test('page-weight-analyzer returns JSON output', async ({ page }) => {
  await page.goto('/tools/page-weight-analyzer/?output=json');
  await page.fill('#in-html', HTML);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"scripts":{"total":3,"external":2', { timeout: 15000 });
  await expect(out).toContainText('"stylesheets":{"total":3,"external":2,"inline":1,"inline_bytes":15,"render_blocking":1');
  await expect(out).toContainText('"lazy_images":1');
});

test('page-weight-analyzer query-param deep-link prefills controls', async ({ page }) => {
  await page.goto('/tools/page-weight-analyzer/?output=json&list_resources=true');
  await expect(page.locator('#in-output')).toHaveValue('json', { timeout: 15000 });
  await expect(page.locator('#in-list_resources')).toBeChecked();
});
