import { test, expect } from './fixtures';

// /tools/html-formatter/ pretty-prints HTML in-browser (pure wasm).
test('html-formatter indents nested elements', async ({ page }) => {
  await page.goto('/tools/html-formatter/');
  await page.fill('#in-html', '<div><p>hi</p></div>');
  await page.fill('#in-indent', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<div>', { timeout: 15000 });
  await expect(out).toContainText('  <p>');
  await expect(out).toContainText('    hi');
  await expect(out).toContainText('</div>');
});

test('html-formatter keeps void elements flat', async ({ page }) => {
  await page.goto('/tools/html-formatter/');
  await page.fill('#in-html', '<ul><li>a</li></ul>');
  await page.fill('#in-indent', '4');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('    <li>', { timeout: 15000 });
  await expect(out).toContainText('        a');
});
