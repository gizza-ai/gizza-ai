import { test, expect } from './fixtures';

const SAM = '@HD\tVN:1.6\nr001\t99\tchr1\t7\t60\t8M2I4M1D3M\t=\t37\t39\tTTAGATAAAGGATACTG\t*\tNM:i:1\tAS:i:30\nr002\t0\tchr1\t9\t30\t3S6M1P1I4M\t*\t0\t0\tAAAAGATAAGGATA\t*\tNM:i:0';

test('sam-to-csv page converts SAM records to CSV with decoded flags and tags', async ({ page }) => {
  await page.goto('/tools/sam-to-csv/');
  await page.fill('#in-input', SAM);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('QNAME,FLAG,RNAME,POS,MAPQ,CIGAR');
  await expect(out).toContainText('FLAG_SUMMARY,NM,AS');
  await expect(out).toContainText('"PAIRED,PROPER_PAIR,MATE_REVERSE,READ1",1,30');
  await expect(out).toContainText('r002,0,chr1,9,30');
});

test('sam-to-csv deep-link emits compact TSV with computed columns', async ({ page }) => {
  const input = encodeURIComponent('r001\t99\tchr1\t7\t60\t8M2I4M1D3M\t=\t37\t39\tTTAGATAAAGGATACTG\t*\tNM:i:1');
  await page.goto(`/tools/sam-to-csv/?input=${input}&delimiter=tab&flags=none&tags=none&include_seq=false&computed=true`);
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  await expect(page.locator('#in-computed')).toBeChecked();
  await expect(page.locator('#in-include_seq')).not.toBeChecked();
  const out = (await page.locator('#tool-output').textContent())!;
  expect(out.split('\n')[0]).toBe('QNAME\tFLAG\tRNAME\tPOS\tMAPQ\tCIGAR\tRNEXT\tPNEXT\tTLEN\tEND\tREF_SPAN\tREAD_LEN\tSTRAND');
  expect(out).toContain('r001\t99\tchr1\t7\t60\t8M2I4M1D3M\t=\t37\t39\t22\t16\t17\t+');
});

test('sam-to-csv advertised flag bits and filters stay wired', async ({ page }) => {
  await page.goto('/tools/sam-to-csv/');
  await page.fill('#in-input', 'r1\t0\tchr1\t10\t60\t4M\t*\t0\t0\tACGT\t*\nr2\t4\t*\t0\t0\t*\t*\t0\t0\tACGT\t*\nr3\t256\tchr1\t20\t60\t4M\t*\t0\t0\tACGT\t*\nr4\t0\tchr1\t30\t5\t4M\t*\t0\t0\tACGT\t*');
  await page.selectOption('#in-flags', 'bits');
  await page.selectOption('#in-tags', 'none');
  await page.check('#in-mapped_only');
  await page.check('#in-primary_only');
  await page.fill('#in-min_mapq', '10');
  await expect(page.locator('#in-min_mapq-slider')).toHaveValue('10');
  const out = (await page.locator('#tool-output').textContent())!;
  expect(out).toContain('FLAG_PAIRED');
  expect(out).toContain('FLAG_SUPPLEMENTARY');
  expect(out).toContain('r1,0,chr1,10,60,4M');
  expect(out).not.toContain('r2,');
  expect(out).not.toContain('r3,');
  expect(out).not.toContain('r4,');
});

test('sam-to-csv validates malformed input with a clear error', async ({ page }) => {
  await page.goto('/tools/sam-to-csv/');
  await page.fill('#in-input', 'r1\tbad\tchr1\t10\t60\t4M\t*\t0\t0\tACGT\t*');
  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/);
  await expect(out).toContainText("FLAG 'bad'");
});

test('sam-to-csv preset and generated CLI example are generic', async ({ page }) => {
  await page.goto('/tools/sam-to-csv/');
  await page.getByRole('button', { name: 'Compact TSV' }).click();
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  await expect(page.locator('#in-flags')).toHaveValue('none');
  await expect(page.locator('#in-tags')).toHaveValue('none');
  await expect(page.locator('#in-include_seq')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('r001\t99\tchr1');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool sam-to-csv');
  expect(cli).toContain('@HD');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
