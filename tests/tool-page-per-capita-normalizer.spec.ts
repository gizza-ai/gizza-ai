import { test, expect } from './fixtures';

const tool = '/tools/per-capita-normalizer/';
const sample = 'region,cases,population\nNorthbridge,120,400000\nEastvale,45,150000\nWestport,18,900000';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('per-capita-normalizer page computes per-100k rates with exact rows', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', sample);
  await page.selectOption('#in-delimiter', 'comma');
  await page.selectOption('#in-header', 'yes');
  await page.selectOption('#in-per', '100000');
  await page.fill('#in-decimals', '2');

  await expect(page.locator('#tool-output')).toContainText('per 100000 · rows: 3', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('1\tEastvale\t45\t150000\t30.00\t2.38\tok');
  await expect(page.locator('#tool-output')).toContainText('3\tWestport\t18\t900000\t2.00\t0.16\tunstable');
});

test('per-capita-normalizer page supports custom bases and CSV output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', 'A,50,200');
  await page.selectOption('#in-header', 'no');
  await page.selectOption('#in-per', 'custom');
  await page.fill('#in-custom_per', '250');
  await page.selectOption('#in-sort', 'input');
  await page.fill('#in-unstable_below', '0');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('basis,250', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('rank,label,count,population,rate_per_250,index,flag');
  await expect(page.locator('#tool-output')).toContainText('1,"A",50,200,62.50,1.00,ok');
});

test('per-capita-normalizer page scales population units and emits markdown', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', '4\t8');
  await page.selectOption('#in-delimiter', 'tab');
  await page.selectOption('#in-header', 'no');
  await page.selectOption('#in-per', '100000');
  await page.selectOption('#in-population_unit', 'thousands');
  await page.fill('#in-decimals', '1');
  await page.fill('#in-unstable_below', '0');
  await page.selectOption('#in-output', 'markdown');

  await expect(page.locator('#tool-output')).toContainText('| rank | label | count | population | rate_per_100000 | index | flag |', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('| 1 | row 1 | 4 | 8000 | 50.0 | 1.00 | ok |');
});

test('per-capita-normalizer query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    tool +
      '?data=' +
      encodeURIComponent(sample) +
      '&delimiter=comma&header=yes&per=1000&decimals=1&sort=input&unstable_below=0',
  );

  await expect(page.locator('#in-data')).toHaveValue(sample, { timeout: 15000 });
  await expect(page.locator('#in-per')).toHaveValue('1000');
  await expect(page.locator('#in-sort')).toHaveValue('input');
  expect(await outputText(page)).toContain('per 1000 · rows: 3');
  expect(await outputText(page)).toContain('1\tNorthbridge\t120\t400000\t0.3\t2.38\tok');
});
