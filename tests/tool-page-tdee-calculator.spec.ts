import { test, expect } from './fixtures';

const EXPECTED_DEFAULT = `{
  "bmr": 1649.0,
  "tdee": 2556.0,
  "activity": "moderate",
  "activity_multiplier": 1.55,
  "formula": "mifflin_st_jeor",
  "energy_unit": "calories",
  "bmi": 22.9,
  "bmi_category": "normal",
  "goals": {
    "mild_loss": 2306.0,
    "loss": 2056.0,
    "extreme_loss": 1556.0,
    "maintain": 2556.0,
    "mild_gain": 2806.0,
    "gain": 3056.0
  },
  "tdee_by_activity": [
    {
      "level": "sedentary",
      "multiplier": 1.2,
      "tdee": 1979.0
    },
    {
      "level": "light",
      "multiplier": 1.375,
      "tdee": 2267.0
    },
    {
      "level": "moderate",
      "multiplier": 1.55,
      "tdee": 2556.0
    },
    {
      "level": "very_active",
      "multiplier": 1.725,
      "tdee": 2845.0
    },
    {
      "level": "extra_active",
      "multiplier": 1.9,
      "tdee": 3133.0
    }
  ],
  "summary": "BMR 1649 kcal (mifflin-st-jeor); TDEE 2556 kcal/day at moderate activity (×1.55)"
}`;

test('tdee-calculator page computes default metric Mifflin-St Jeor result', async ({ page }) => {
  await page.goto('/tools/tdee-calculator/');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"tdee": 2556.0', { timeout: 15_000 });
  expect(await out.textContent()).toBe(EXPECTED_DEFAULT);
});

test('tdee-calculator deep link supports imperial Harris-Benedict in kilojoules', async ({ page }) => {
  const params = new URLSearchParams({
    age: '28',
    sex: 'female',
    weight: '150',
    height: '65',
    units: 'imperial',
    activity: 'light',
    formula: 'harris_benedict',
    body_fat: '20',
    energy_unit: 'kilojoules',
  });

  await page.goto(`/tools/tdee-calculator/?${params.toString()}`);
  await expect(page.locator('#in-units')).toHaveValue('imperial', { timeout: 15_000 });
  await expect(page.locator('#in-formula')).toHaveValue('harris_benedict');
  await expect(page.locator('#in-energy_unit')).toHaveValue('kilojoules');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"energy_unit": "kilojoules"', { timeout: 15_000 });
  await expect(out).toContainText('"formula": "harris_benedict"');
  await expect(out).toContainText('"activity": "light"');
  await expect(out).toContainText('"tdee": 8439.0');
  await expect(out).toContainText('BMR 6138 kJ (harris-benedict); TDEE 8439 kJ/day at light activity (×1.375)');
});
