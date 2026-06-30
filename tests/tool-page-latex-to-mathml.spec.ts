import { test, expect } from './fixtures';

test('latex-to-mathml page converts a fraction (default block)', async ({ page }) => {
  await page.goto('/tools/latex-to-mathml/');

  await page.fill('#in-latex', '\\frac{1}{2}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<math', { timeout: 15000 });
  await expect(out).toContainText('display="block"');
  await expect(out).toContainText('<mfrac>');
  await expect(out).toContainText('</math>');
});

test('latex-to-mathml page inline display + superscript', async ({ page }) => {
  await page.goto('/tools/latex-to-mathml/');

  await page.fill('#in-latex', 'E = mc^2');
  await page.selectOption('#in-display', 'inline');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('display="inline"', { timeout: 15000 });
  await expect(out).toContainText('<msup>');
});

test('latex-to-mathml page pretty-print indents the output', async ({ page }) => {
  await page.goto('/tools/latex-to-mathml/');

  await page.fill('#in-latex', '\\frac{1}{2}');
  await page.check('#in-pretty');
  const out = page.locator('#tool-output');
  // pretty output puts <mfrac> on its own indented line
  await expect(out).toContainText('\n  <mfrac>', { timeout: 15000 });
});
