import { test, expect } from './fixtures';

test('dna-reverse-complement page renders a real reverse complement', async ({ page }) => {
  await page.goto('/tools/dna-reverse-complement/');
  await page.fill('#in-sequence', 'ATGGCCATTGTAATGGGCCGC');
  await page.selectOption('#in-operation', 'reverse_complement');
  await page.selectOption('#in-output_alphabet', 'auto');
  await page.fill('#in-line_width', '0');
  await page.selectOption('#in-on_invalid', 'error');

  await expect(page.locator('#tool-output')).toContainText('GCGGCCCATTACAATGGCCAT', { timeout: 15000 });
});

test('dna-reverse-complement deep-link supports FASTA wrapping, RNA output, and stats', async ({ page }) => {
  const sequence = '>seq1\nATGGCCATTGTAATGGGCCGC';
  await page.goto(
    `/tools/dna-reverse-complement/?sequence=${encodeURIComponent(sequence)}&operation=reverse_complement&output_alphabet=rna&preserve_case=false&line_width=10&on_invalid=error&show_stats=true`,
  );

  await expect(page.locator('#in-sequence')).toHaveValue(sequence, { timeout: 15000 });
  await expect(page.locator('#in-show_stats')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('>seq1', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('GCGGCCCAUU');
  await expect(page.locator('#tool-output')).toContainText('ACAAUGGCCA');
  await expect(page.locator('#tool-output')).toContainText('# sequences: 1');
  await expect(page.locator('#tool-output')).toContainText('# gc_content:');
});

test('dna-reverse-complement reports invalid characters and clears stale output', async ({ page }) => {
  await page.goto('/tools/dna-reverse-complement/');
  await page.fill('#in-sequence', 'ACGT');
  await expect(page.locator('#tool-output')).toContainText('ACGT', { timeout: 15000 });

  await page.fill('#in-sequence', 'ACGT*');
  await expect(page.locator('#tool-output')).toContainText('invalid sequence character', { timeout: 15000 });
  await expect(page.locator('#tool-output')).not.toContainText('ACGT\nACGT');
});
