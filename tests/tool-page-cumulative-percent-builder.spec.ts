import { test, expect } from './fixtures';

const sample = 'issue,count\nScratches,400\nDents,250\nMisalignment,150\nPackaging,120\nOther,80';
const output = (page) => page.locator('#tool-output').evaluate((el) => el.textContent ?? '');

test('cumulative-percent-builder page creates Pareto table with exact rows', async ({ page }) => {
  await page.goto('/tools/cumulative-percent-builder/');
  await page.fill('#in-data', sample);
  await page.selectOption('#in-header', 'yes');
  await page.fill('#in-threshold', '80');
  await page.fill('#in-decimals', '1');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('total: 1000.0 · rows: 5 · vital few at 80.0%: 3', { timeout: 15_000 });
  expect(await output(page)).toContain('1\tScratches\t400.0\t40.0%\t1\t400.0\t40.0%\tvital');
  expect(await output(page)).toContain('3\tMisalignment\t150.0\t15.0%\t3\t800.0\t80.0%\tvital');
  expect(await output(page)).toContain('pareto chart (# = share, | marks cumulative threshold crossing)');
});

test('cumulative-percent-builder deep-link applies csv tail bucket', async ({ page }) => {
  const data = encodeURIComponent('Scratches,400\nDents,250\nMisalignment,150\nPackaging,120\nColor,50\nOther,30');
  await page.goto(`/tools/cumulative-percent-builder/?data=${data}&delimiter=comma&header=no&sort=desc&threshold=80&top_n=3&decimals=0&output=csv`);

  await expect(page.locator('#in-output')).toHaveValue('csv', { timeout: 15_000 });
  await expect(page.locator('#in-top_n')).toHaveValue('3');
  await expect(page.locator('#in-header')).toHaveValue('no');
  const text = await output(page);
  expect(text).toContain('vital_few_count,3');
  expect(text).toContain('4,"Other",200,20,4,1000,100,trivial');
});

test('cumulative-percent-builder supports input order and markdown output', async ({ page }) => {
  await page.goto('/tools/cumulative-percent-builder/');
  await page.fill('#in-data', 'B\t2\nA\t8\nC\t1');
  await page.selectOption('#in-delimiter', 'tab');
  await page.selectOption('#in-header', 'no');
  await page.selectOption('#in-sort', 'input');
  await page.fill('#in-threshold', '75');
  await page.fill('#in-decimals', '0');
  await page.selectOption('#in-output', 'markdown');

  await expect(page.locator('#in-sort')).toHaveValue('input');
  await expect(page.locator('#tool-output')).toContainText('| 1 | B | 2 | 18% | 1 | 2 | 18% | vital |', { timeout: 15_000 });
});
