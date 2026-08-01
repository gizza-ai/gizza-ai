import { test, expect } from './fixtures';

const SAMPLE_VCF = `##fileformat=VCFv4.2
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tNA001\tNA002
chr1\t100\trs1\tA\tG\t50\tPASS\tDP=30;AF=0.5\tGT:DP\t0/1:20\t1/1:10
chr2\t200\t.\tC\tT\t99\tq10\tDP=12\tGT:DP\t0/0:12\t./.`;

const EXPECTED_LONG = `CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tDP\tAF\tSAMPLE\tGT\tDP
chr1\t100\trs1\tA\tG\t50\tPASS\t30\t0.5\tNA001\t0/1\t20
chr1\t100\trs1\tA\tG\t50\tPASS\t30\t0.5\tNA002\t1/1\t10
chr2\t200\t.\tC\tT\t99\tq10\t12\t.\tNA001\t0/0\t12
chr2\t200\t.\tC\tT\t99\tq10\t12\t.\tNA002\t./.\t.`;

test('genomic-vcf-to-tsv page flattens VCF into long sample rows', async ({ page }) => {
  await page.goto('/tools/genomic-vcf-to-tsv/');
  await page.fill('#in-input', SAMPLE_VCF);

  await expect(page.locator('#tool-output')).toHaveText(EXPECTED_LONG, {
    timeout: 15_000,
  });
});

test('genomic-vcf-to-tsv supports wide layout with selected INFO keys', async ({ page }) => {
  await page.goto('/tools/genomic-vcf-to-tsv/');
  await page.fill('#in-input', SAMPLE_VCF);
  await page.selectOption('#in-layout', 'wide');
  await page.fill('#in-info_fields', 'DP,AF');
  await page.check('#in-prefix_info');
  await page.fill('#in-missing', 'NA');

  await expect(page.locator('#tool-output')).toContainText(
    'CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO_DP\tINFO_AF\tNA001_GT\tNA001_DP\tNA002_GT\tNA002_DP',
    { timeout: 15_000 },
  );
  await expect(page.locator('#tool-output')).toContainText(
    'chr1\t100\trs1\tA\tG\t50\tPASS\t30\t0.5\t0/1\t20\t1/1\t10',
  );
});

test('genomic-vcf-to-tsv deep link prefills pass-only site table', async ({ page }) => {
  const params = new URLSearchParams({
    input: SAMPLE_VCF,
    layout: 'long',
    include_info: 'false',
    include_samples: 'false',
    pass_only: 'true',
    header: 'false',
  });

  await page.goto(`/tools/genomic-vcf-to-tsv/?${params.toString()}`);
  await expect(page.locator('#in-pass_only')).toBeChecked({ timeout: 15_000 });
  await expect(page.locator('#in-include_info')).not.toBeChecked();
  await expect(page.locator('#in-include_samples')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('chr1\t100\trs1\tA\tG\t50\tPASS', {
    timeout: 15_000,
  });
});
