import { test, expect } from './fixtures';

test('matrix-text-parser page parses whitespace rows to JSON with shape', async ({ page }) => {
  await page.goto('/tools/matrix-text-parser/');
  await page.fill('#in-matrix', '1 2 3\n4 5 6');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"shape": [', { timeout: 20_000 });
  await expect(output).toContainText('2,');
  await expect(output).toContainText('3');
  await expect(output).toContainText('"matrix": [');
  await expect(output).toContainText('4');
});

test('matrix-text-parser deep link handles ragged CSV with header and aligned output', async ({ page }) => {
  const qs =
    '?matrix=' + encodeURIComponent('x,y,z\n1,2,3\n4,5') +
    '&input_format=delimited' +
    '&delimiter=comma' +
    '&output=aligned' +
    '&cells=auto' +
    '&fractions=true' +
    '&header=true' +
    '&ragged=pad' +
    '&fill=0' +
    '&indent=2';

  await page.goto('/tools/matrix-text-parser/' + qs);
  await expect(page.locator('#in-output')).toHaveValue('aligned', { timeout: 15_000 });
  await expect(page.locator('#in-header')).toBeChecked();
  await expect(page.locator('#in-ragged')).toHaveValue('pad');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('x  y  z', { timeout: 20_000 });
  await expect(output).toContainText('1  2  3');
  await expect(output).toContainText('4  5  0');
});

test('matrix-text-parser page renders LaTeX input as a bare array', async ({ page }) => {
  await page.goto('/tools/matrix-text-parser/?matrix=' + encodeURIComponent('\\begin{bmatrix} 1 & 2 \\\\ 3 & 4 \\end{bmatrix}') + '&output=array&indent=0');
  await expect(page.locator('#in-output')).toHaveValue('array', { timeout: 15_000 });

  const output = page.locator('#tool-output');
  await expect(output).toHaveText('[[1,2],[3,4]]', { timeout: 20_000 });
});
