import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('child-growth-percentile reports CDC metrics for a 3-year-old', async ({ page }) => {
  await page.goto('/tools/child-growth-percentile/');
  await page.waitForSelector('#in-sex');
  await expect(page.locator('#in-sex')).toHaveValue('boy');
  await expect(page.locator('#in-units')).toHaveValue('metric');
  await expect(page.locator('#in-chart')).toHaveValue('auto');

  await page.selectOption('#in-sex', 'girl');
  await setField(page, '#in-age', '3y');
  await setField(page, '#in-height', '95');
  await setField(page, '#in-weight', '14');
  await setField(page, '#in-head_circumference', '0');
  await setField(page, '#in-decimals', '2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Child: girl, age 3 y 0 mo (36 months)', { timeout: 20_000 });
  await expect(out).toContainText('Reference: CDC 2-20 years growth charts (standing stature)');
  await expect(out).toContainText('Height-for-age: 95 cm ->');
  await expect(out).toContainText('Weight-for-age: 14 kg ->');
  await expect(out).toContainText('BMI-for-age: 15.51 kg/m2 ->');
  expect(await outputText(page)).toContain('BMI category: Healthy weight');
});

test('child-growth-percentile deep link supports US units and forced child chart', async ({ page }) => {
  const params = new URLSearchParams({
    sex: 'boy',
    age: '10y 6m',
    height: '55',
    weight: '72',
    head_circumference: '0',
    units: 'us',
    chart: 'child',
    decimals: '1',
  });
  await page.goto(`/tools/child-growth-percentile/?${params.toString()}`);

  await expect(page.locator('#in-units')).toHaveValue('us', { timeout: 15_000 });
  await expect(page.locator('#in-chart')).toHaveValue('child');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Child: boy, age 10 y 6 mo (126 months)', { timeout: 20_000 });
  await expect(out).toContainText('Height-for-age: 55 in ->');
  await expect(out).toContainText('Weight-for-age: 72 lb ->');
  await expect(out).toContainText('BMI-for-age:');
});

test('child-growth-percentile covers infant enum and decimal boundary', async ({ page }) => {
  await page.goto('/tools/child-growth-percentile/');
  await page.selectOption('#in-sex', 'boy');
  await setField(page, '#in-age', '0 months');
  await setField(page, '#in-height', '49.99');
  await setField(page, '#in-weight', '3.53');
  await setField(page, '#in-head_circumference', '35.8');
  await page.selectOption('#in-chart', 'infant');
  await setField(page, '#in-decimals', '4');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Reference: CDC infant growth charts', { timeout: 20_000 });
  await expect(out).toContainText('Length-for-age: 49.99 cm ->');
  await expect(out).toContainText('Head-circumference-for-age: 35.8 cm ->');
});

test('child-growth-percentile page ships runnable CLI, labels, and preset chips', async ({ page }) => {
  await page.goto('/tools/child-growth-percentile/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool child-growth-percentile');
  expect(cli).toContain('height=95');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
  await expect(page.locator('#in-units option[value="us"]')).toHaveText('US — inches and pounds');
  await expect(page.locator('#in-chart option[value="infant"]')).toHaveText('CDC infant charts (birth–36 months)');
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
});
