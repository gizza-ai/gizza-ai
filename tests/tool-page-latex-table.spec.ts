import { test, expect } from './fixtures';

test('latex-table booktabs default', async ({ page }) => {
  await page.goto('/tools/latex-table/');
  await page.fill('#in-data', 'a,b\n1,2\n3,4');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\begin{tabular}{ll}', { timeout: 15000 });
  await expect(out).toContainText('\\toprule');
  await expect(out).toContainText('a & b \\\\');
  await expect(out).toContainText('\\midrule');
  await expect(out).toContainText('1 & 2 \\\\');
  await expect(out).toContainText('\\bottomrule');
});

test('latex-table grid style adds bars and hlines', async ({ page }) => {
  await page.goto('/tools/latex-table/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.selectOption('#in-style', 'grid');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\begin{tabular}{|l|l|}', { timeout: 15000 });
  await expect(out).toContainText('\\hline');
});

test('latex-table no-header checkbox omits midrule', async ({ page }) => {
  await page.goto('/tools/latex-table/');
  await page.fill('#in-data', '1,2\n3,4');
  await page.uncheck('#in-header');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\toprule', { timeout: 15000 });
  await expect(out).not.toContainText('\\midrule');
});

test('latex-table bold header checkbox wraps cells', async ({ page }) => {
  await page.goto('/tools/latex-table/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.check('#in-bold_header');
  await expect(page.locator('#tool-output')).toContainText(
    '\\textbf{a} & \\textbf{b}',
    { timeout: 15000 },
  );
});

test('latex-table caption + label wrap in a table float', async ({ page }) => {
  await page.goto('/tools/latex-table/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.fill('#in-caption', 'My results');
  await page.fill('#in-label', 'tab:res');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\begin{table}[ht]', { timeout: 15000 });
  await expect(out).toContainText('\\caption{My results}');
  await expect(out).toContainText('\\label{tab:res}');
});

test('latex-table query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/latex-table/?data=' + encodeURIComponent('a,b\n1,2') + '&style=plain',
  );
  await expect(page.locator('#in-data')).toHaveValue('a,b\n1,2', { timeout: 15000 });
  await expect(page.locator('#in-style')).toHaveValue('plain');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('a & b \\\\', { timeout: 15000 });
  await expect(out).not.toContainText('rule');
});
