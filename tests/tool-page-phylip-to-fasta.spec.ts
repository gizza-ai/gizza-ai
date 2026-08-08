import { test, expect } from './fixtures';

const interleaved = '3 12\nAlpha     ACGT\nBeta      ACGA\nGamma     TCGA\n\nACGTACGT\nACGTACGT\nACGTACGT';
const sequentialLongNames = '2 8\nHomo_sapiens ACGTACGT\nPan_troglodytes ACGTTCGT';
const gapped = '2 10\nAlpha     AC--GTAC-T\nBeta      ACGTGT--CT';

test('phylip-to-fasta page converts interleaved PHYLIP to exact FASTA', async ({ page }) => {
  await page.goto('/tools/phylip-to-fasta/');
  await page.fill('#in-phylip', interleaved);
  await page.fill('#in-wrap', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>Alpha', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '>Alpha\nACGTACGTACGT\n>Beta\nACGAACGTACGT\n>Gamma\nTCGAACGTACGT\n',
  );
});

test('phylip-to-fasta deep link applies layout and name-style selects', async ({ page }) => {
  const qs =
    '?phylip=' + encodeURIComponent(sequentialLongNames) +
    '&layout=sequential' +
    '&name_style=relaxed' +
    '&wrap=0';
  await page.goto('/tools/phylip-to-fasta/' + qs);

  await expect(page.locator('#in-layout')).toHaveValue('sequential', { timeout: 15_000 });
  await expect(page.locator('#in-name_style')).toHaveValue('relaxed');
  await expect(page.locator('#in-wrap')).toHaveValue('0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>Homo_sapiens', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>Homo_sapiens\nACGTACGT\n>Pan_troglodytes\nACGTTCGT\n');
});

test('phylip-to-fasta page strips gaps and uppercases with non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/phylip-to-fasta/');
  await page.fill('#in-phylip', gapped.toLowerCase());
  await page.fill('#in-wrap', '0');
  await page.check('#in-remove_gaps');
  await page.check('#in-uppercase');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>alpha', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>alpha\nACGTACT\n>beta\nACGTGTCT\n');
});

test('phylip-to-fasta page reports malformed PHYLIP clearly', async ({ page }) => {
  await page.goto('/tools/phylip-to-fasta/');
  await page.fill('#in-phylip', '1 12\nAlpha     ACGT\n');
  await expect(page.locator('#tool-output')).toContainText('header declares 12', { timeout: 15_000 });
});
