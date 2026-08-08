import { test, expect } from './fixtures';

const simpleNexus = '#NEXUS\nbegin data;\n  dimensions ntax=2 nchar=8;\n  format datatype=dna gap=-;\n  matrix\n    Alpha  ACGTACGT\n    Beta   ACGTTCGT\n  ;\nend;';

const interleavedMatchchar = '#NEXUS\nbegin characters;\n  dimensions ntax=2 nchar=8;\n  format datatype=dna gap=- matchchar=. interleave;\n  matrix\n    Alpha  ACGT\n    Beta   ....\n\n    Alpha  TGCA\n    Beta   ....\n  ;\nend;';

const quotedLabels = '#NEXUS\nbegin data;\n  dimensions ntax=2 nchar=4;\n  matrix\n    \'Homo sapiens\' ACGT\n    Pan_troglodytes ACGT\n  ;\nend;';

test('nexus-to-fasta page converts DATA matrix to exact FASTA', async ({ page }) => {
  await page.goto('/tools/nexus-to-fasta/');
  await page.fill('#in-nexus', simpleNexus);
  await page.fill('#in-wrap', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>Alpha', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>Alpha\nACGTACGT\n>Beta\nACGTTCGT\n');
});

test('nexus-to-fasta deep link applies layout select and expands matchchar', async ({ page }) => {
  const qs =
    '?nexus=' + encodeURIComponent(interleavedMatchchar) +
    '&layout=interleaved' +
    '&wrap=0';
  await page.goto('/tools/nexus-to-fasta/' + qs);

  await expect(page.locator('#in-layout')).toHaveValue('interleaved', { timeout: 15_000 });
  await expect(page.locator('#in-wrap')).toHaveValue('0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>Beta', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>Alpha\nACGTTGCA\n>Beta\nACGTTGCA\n');
});

test('nexus-to-fasta page applies case and underscore checkbox states', async ({ page }) => {
  await page.goto('/tools/nexus-to-fasta/');
  await page.fill('#in-nexus', quotedLabels);
  await page.fill('#in-wrap', '0');
  await page.selectOption('#in-case', 'lower');
  await page.check('#in-underscores_to_spaces');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>Homo sapiens', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>Homo sapiens\nacgt\n>Pan troglodytes\nacgt\n');
});

test('nexus-to-fasta page reports malformed NEXUS clearly', async ({ page }) => {
  await page.goto('/tools/nexus-to-fasta/');
  await page.fill('#in-nexus', '#NEXUS\nbegin trees;\n tree one = (a,b);\nend;');
  await expect(page.locator('#tool-output')).toContainText("no 'begin data;'", { timeout: 15_000 });
});
