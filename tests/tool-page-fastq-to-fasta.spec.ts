import { test, expect } from './fixtures';

const twoReads = '@read1 sample\nACGTACGTNN\n+\nIIIIIIII##\n@read2\nACGT\n+\n!!!!';

test('fastq-to-fasta page converts two reads to exact FASTA, stripping quality', async ({ page }) => {
  await page.goto('/tools/fastq-to-fasta/');
  await page.fill('#in-fastq', twoReads);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>read1 sample', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>read1 sample\nACGTACGTNN\n>read2\nACGT\n');
});

test('fastq-to-fasta deep link applies quality + length filter with quality offset select', async ({ page }) => {
  const qs =
    '?fastq=' + encodeURIComponent(twoReads) +
    '&min_length=5' +
    '&min_quality=20' +
    '&quality_offset=33';
  await page.goto('/tools/fastq-to-fasta/' + qs);

  await expect(page.locator('#in-min_length')).toHaveValue('5', { timeout: 15_000 });
  await expect(page.locator('#in-min_quality')).toHaveValue('20');
  await expect(page.locator('#in-quality_offset')).toHaveValue('33');

  const out = page.locator('#tool-output');
  // read2 (4 bases, mean quality 0) is dropped; read1 survives.
  await expect(out).toContainText('>read1 sample', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>read1 sample\nACGTACGTNN\n');
});

test('fastq-to-fasta page renames headers and uppercases via checkboxes', async ({ page }) => {
  await page.goto('/tools/fastq-to-fasta/');
  await page.fill('#in-fastq', '@read1 sample\nacgtacgtnn\n+\nIIIIIIII##\n@read2\nacgt\n+\nIIII');
  await page.check('#in-rename');
  await page.check('#in-uppercase');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>1\nACGTACGTNN\n>2\nACGT\n');
});

test('fastq-to-fasta page wraps sequences at the requested width', async ({ page }) => {
  await page.goto('/tools/fastq-to-fasta/');
  await page.fill('#in-fastq', '@read1\nACGTACGTAC\n+\nIIIIIIIIII');
  await page.fill('#in-wrap', '4');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('>read1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('>read1\nACGT\nACGT\nAC\n');
});

test('fastq-to-fasta page reports malformed FASTQ clearly', async ({ page }) => {
  await page.goto('/tools/fastq-to-fasta/');
  await page.fill('#in-fastq', '@r\nACGT\n+');
  await expect(page.locator('#tool-output')).toContainText('multiple of 4', { timeout: 15_000 });
});
