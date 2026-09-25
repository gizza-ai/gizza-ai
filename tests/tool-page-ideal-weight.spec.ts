import { test, expect } from './fixtures';

// /tools/ideal-weight/ estimates adult ideal-body-weight ranges locally.
test('ideal-weight renders the default metric report', async ({ page }) => {
  await page.goto('/tools/ideal-weight/');
  await page.fill('#in-height', '175');
  await page.selectOption('#in-sex', 'male');
  await page.selectOption('#in-units', 'metric');
  await page.selectOption('#in-frame', 'medium');
  await page.fill('#in-bmi_min', '18.5');
  await page.fill('#in-bmi_max', '24.9');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"height_cm": 175.0', { timeout: 15000 });
  await expect(out).toContainText('"formula": "devine"');
  await expect(out).toContainText('"kg": 70.5');
  await expect(out).toContainText('"average_kg": 70.0');
  await expect(out).toContainText('healthy BMI 18.5–24.9 range 56.7–76.3 kg');
});

test('ideal-weight honors deep-linked imperial and frame inputs', async ({ page }) => {
  const params = new URLSearchParams({
    height: '65',
    sex: 'female',
    units: 'imperial',
    frame: 'large',
    age: '16',
    bmi_min: '18.5',
    bmi_max: '23',
  });
  await page.goto(`/tools/ideal-weight/?${params.toString()}`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"height_ft_in": "5\'5\\\""', { timeout: 15000 });
  await expect(out).toContainText('"sex": "female"');
  await expect(out).toContainText('"frame": "large"');
  await expect(out).toContainText('"frame_adjustment_pct": 10.0');
  await expect(out).toContainText('age 16 is under 18');
});
