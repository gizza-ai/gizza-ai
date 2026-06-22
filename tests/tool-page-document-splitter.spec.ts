import { test, expect } from './fixtures';

// /tools/document-splitter/ splits a Markdown/HTML document in-browser (pure wasm).
test('document-splitter splits markdown into per-section files', async ({ page }) => {
  await page.goto('/tools/document-splitter/');
  await page.fill('#in-document', '# Intro\nWelcome text.\n\n# Setup\nRun the installer.');
  await page.selectOption('#in-format', 'markdown');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 section(s)', { timeout: 15000 });
  await expect(out).toContainText('01-intro.md');
  await expect(out).toContainText('02-setup.md');
  await expect(out).toContainText('Welcome text.');
});

test('document-splitter splits html on heading tags', async ({ page }) => {
  await page.goto('/tools/document-splitter/');
  await page.fill('#in-document', '<h1>Alpha</h1><p>one</p><h1>Beta</h1><p>two</p>');
  await page.selectOption('#in-format', 'html');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 section(s)', { timeout: 15000 });
  await expect(out).toContainText('01-alpha.html');
  await expect(out).toContainText('02-beta.html');
});
