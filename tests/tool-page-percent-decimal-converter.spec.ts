import { test, expect } from './fixtures';

async function setField(
  page: import('@playwright/test').Page,
  id: string,
  value: string,
) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('percent-decimal-converter converts a percent column to exact decimals', async ({ page }) => {
  await page.goto('/tools/percent-decimal-converter/');
  await setField(page, '#in-data', 'rate\n12.5%\n7%\n0.5%');
  await page.selectOption('#in-direction', 'percent_to_decimal');

  await expect(page.locator('#tool-output')).toHaveText('rate\n0.125\n0.07\n0.005', {
    timeout: 15_000,
  });
});

test('percent-decimal-converter deep-link auto-detects selected columns', async ({ page }) => {
  const qs = new URLSearchParams({
    data: 'name,rate,share\nApple,12.5%,0.25\nPear,7%,0.5',
    direction: 'auto',
    unit: 'percent',
    columns: 'rate,share',
    header: 'true',
    delimiter: 'comma',
    decimals: '-1',
    trim_zeros: 'false',
    suffix: 'true',
  });
  await page.goto(`/tools/percent-decimal-converter/?${qs.toString()}`);

  await expect(page.locator('#in-direction')).toHaveValue('auto');
  await expect(page.locator('#in-columns')).toHaveValue('rate,share');
  await expect(page.locator('#in-header')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'name,rate,share\nApple,0.125,25%\nPear,0.07,50%',
    { timeout: 15_000 },
  );
});

test('percent-decimal-converter converts decimals up with rounding and trim zeros', async ({
  page,
}) => {
  await page.goto('/tools/percent-decimal-converter/');
  await setField(page, '#in-data', 'rate\n0.333333\n0.5\n0.005');
  await page.selectOption('#in-direction', 'decimal_to_percent');
  await setField(page, '#in-decimals', '2');
  await page.check('#in-trim_zeros');

  await expect(page.locator('#tool-output')).toHaveText('rate\n33.33%\n50%\n0.5%', {
    timeout: 15_000,
  });
});

test('percent-decimal-converter covers unit enum choices', async ({ page }) => {
  await page.goto('/tools/percent-decimal-converter/');
  await setField(page, '#in-data', 'rate\n0.125');
  await page.selectOption('#in-direction', 'decimal_to_percent');
  await page.selectOption('#in-unit', 'permille');
  await expect(page.locator('#tool-output')).toHaveText('rate\n125‰', { timeout: 15_000 });

  await page.selectOption('#in-unit', 'basis_points');
  await expect(page.locator('#tool-output')).toHaveText('rate\n1250 bps', {
    timeout: 15_000,
  });
});

test('percent-decimal-converter respects non-default suffix and header checkboxes', async ({
  page,
}) => {
  await page.goto('/tools/percent-decimal-converter/');
  await setField(page, '#in-data', '0.125\n0.5');
  await page.selectOption('#in-direction', 'decimal_to_percent');
  await page.uncheck('#in-header');
  await page.uncheck('#in-suffix');

  await expect(page.locator('#tool-output')).toHaveText('12.5\n50', { timeout: 15_000 });
});

test('percent-decimal-converter supports semicolon tables and named columns', async ({ page }) => {
  await page.goto('/tools/percent-decimal-converter/');
  await setField(page, '#in-data', 'name;rate;qty\nA;12.5%;3\nB;7%;10');
  await page.selectOption('#in-direction', 'percent_to_decimal');
  await setField(page, '#in-columns', 'rate');
  await setField(page, '#in-delimiter', 'semicolon');

  await expect(page.locator('#tool-output')).toHaveText('name;rate;qty\nA;0.125;3\nB;0.07;10', {
    timeout: 15_000,
  });
});

test('percent-decimal-converter enforces decimal precision boundaries', async ({ page }) => {
  await page.goto('/tools/percent-decimal-converter/');
  await setField(page, '#in-data', 'rate\n0.3333333333333');
  await page.selectOption('#in-direction', 'decimal_to_percent');
  await setField(page, '#in-decimals', '12');
  await expect(page.locator('#tool-output')).toHaveText('rate\n33.333333333330%', {
    timeout: 15_000,
  });

  await setField(page, '#in-decimals', '13');
  await expect(page.locator('#tool-output')).toHaveText(
    'decimals must be between -1 (exact) and 12, got 13',
    { timeout: 15_000 },
  );
});
