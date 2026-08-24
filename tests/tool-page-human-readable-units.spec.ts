import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('human-readable-units formats bytes with the default decimal scale', async ({ page }) => {
  await page.goto('/tools/human-readable-units/');
  await setField(page, '#in-value', '1500000000');

  await expect(page.locator('#tool-output')).toHaveText('1.5 GB', { timeout: 15_000 });
});

test('human-readable-units deep-link renders a millisecond duration with three segments', async ({ page }) => {
  const qs = new URLSearchParams({
    value: '11722000',
    kind: 'duration',
    input_unit: 'millisecond',
    base: 'decimal',
    style: 'short',
    precision: '1',
    max_units: '3',
    trim_zeros: 'true',
    detail: 'value',
  });
  await page.goto(`/tools/human-readable-units/?${qs.toString()}`);

  await expect(page.locator('#in-kind')).toHaveValue('duration');
  await expect(page.locator('#in-input_unit')).toHaveValue('millisecond');
  await expect(page.locator('#in-max_units')).toHaveValue('3');
  await expect(page.locator('#tool-output')).toHaveText('3h 15m 22s', { timeout: 15_000 });
});

test('human-readable-units covers byte scale enum choices and fixed decimals', async ({ page }) => {
  await page.goto('/tools/human-readable-units/');
  await setField(page, '#in-value', '1073741824');

  await page.selectOption('#in-base', 'decimal');
  await expect(page.locator('#tool-output')).toHaveText('1.1 GB', { timeout: 15_000 });

  await page.selectOption('#in-base', 'binary-iec');
  await expect(page.locator('#tool-output')).toHaveText('1 GiB', { timeout: 15_000 });

  await page.selectOption('#in-base', 'binary-jedec');
  await page.uncheck('#in-trim_zeros');
  await expect(page.locator('#tool-output')).toHaveText('1.0 GB', { timeout: 15_000 });
});

test('human-readable-units covers kind/style/detail enum choices', async ({ page }) => {
  await page.goto('/tools/human-readable-units/');

  await setField(page, '#in-value', '1200000');
  await page.selectOption('#in-kind', 'number');
  await page.selectOption('#in-style', 'long');
  await expect(page.locator('#tool-output')).toHaveText('1.2 million', { timeout: 15_000 });

  await page.selectOption('#in-detail', 'breakdown');
  await expect(page.locator('#tool-output')).toContainText('Formatted: 1.2 million', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('Scientific: 1.2e6');

  await page.selectOption('#in-style', 'narrow');
  await page.selectOption('#in-detail', 'value');
  await expect(page.locator('#tool-output')).toHaveText('1.2M', { timeout: 15_000 });
});

test('human-readable-units validates precision and max_units boundaries', async ({ page }) => {
  await page.goto('/tools/human-readable-units/');
  await setField(page, '#in-value', '1000');
  await setField(page, '#in-precision', '6');
  await expect(page.locator('#tool-output')).toHaveText('1 kB', { timeout: 15_000 });

  await setField(page, '#in-precision', '7');
  await expect(page.locator('#tool-output')).toContainText('precision must be between 0 and 6', {
    timeout: 15_000,
  });

  await setField(page, '#in-precision', '1');
  await page.selectOption('#in-kind', 'duration');
  await setField(page, '#in-max_units', '7');
  await expect(page.locator('#tool-output')).toHaveText('16m 40s', { timeout: 15_000 });

  await setField(page, '#in-max_units', '8');
  await expect(page.locator('#tool-output')).toContainText('max_units must be between 1 and 7', {
    timeout: 15_000,
  });
});

test('human-readable-units enforces the magnitude cap boundary', async ({ page }) => {
  await page.goto('/tools/human-readable-units/');
  await setField(page, '#in-value', '1e21');
  await expect(page.locator('#tool-output')).toHaveText('1 ZB', { timeout: 15_000 });

  await setField(page, '#in-value', '2e21');
  await expect(page.locator('#tool-output')).toContainText('value out of range', { timeout: 15_000 });
});
