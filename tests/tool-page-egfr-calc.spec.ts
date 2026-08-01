import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').replace(/\s+$/, '');
}

test('egfr-calc page computes the default CKD-EPI 2021 case exactly', async ({ page }) => {
  await page.goto('/tools/egfr-calc/');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"egfr": 92.0', { timeout: 15000 });
  expect(await outputText(page)).toBe(`{
  "egfr": 92.0,
  "unit": "mL/min/1.73 m²",
  "equation": "ckd_epi_2021",
  "creatinine_mg_dl": 1.0,
  "age": 50.0,
  "sex": "male",
  "gfr_stage": "G1",
  "stage_description": "Normal or high",
  "summary": "eGFR 92 mL/min/1.73 m² (CKD-EPI 2021) — GFR category G1 (Normal or high)"
}`);
});

test('egfr-calc page supports SI units and female G3a example', async ({ page }) => {
  await page.goto('/tools/egfr-calc/');
  await page.fill('#in-creatinine', '106.1');
  await page.selectOption('#in-creatinine_unit', 'µmol/L');
  await page.fill('#in-age', '60');
  await page.selectOption('#in-sex', 'female');
  await page.selectOption('#in-equation', 'ckd_epi_2021');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"egfr": 52.0', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('"creatinine_mg_dl": 1.2');
  expect(text).toContain('"gfr_stage": "G3a"');
  expect(text).toContain('CKD-EPI 2021');
});

test('egfr-calc page deep-link prefills and computes 2009 comparison', async ({ page }) => {
  await page.goto(
    '/tools/egfr-calc/?creatinine=1.0&creatinine_unit=mg%2FdL&age=50&sex=male&equation=ckd_epi_2009',
  );

  await expect(page.locator('#in-creatinine')).toHaveValue('1.0', { timeout: 15000 });
  await expect(page.locator('#in-creatinine_unit')).toHaveValue('mg/dL');
  await expect(page.locator('#in-age')).toHaveValue('50');
  await expect(page.locator('#in-sex')).toHaveValue('male');
  await expect(page.locator('#in-equation')).toHaveValue('ckd_epi_2009');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"equation": "ckd_epi_2009"', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('"egfr": 87.0');
  expect(text).toContain('CKD-EPI 2009');
});
