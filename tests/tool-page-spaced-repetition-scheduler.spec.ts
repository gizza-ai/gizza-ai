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

const BASIC_LOG = `card, date, grade
capital-of-peru, 2026-08-01, good
capital-of-peru, 2026-08-02, good`;

test('spaced-repetition-scheduler page renders an exact SM-2 CSV schedule', async ({ page }) => {
  await page.goto('/tools/spaced-repetition-scheduler/');
  await setField(page, '#in-reviews', BASIC_LOG);
  await setField(page, '#in-today', '2026-08-03');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('capital-of-peru', { timeout: 15_000 });
  expect(await outputText(page)).toBe(
    'card,reviews,lapses,reps,last_review,ease,interval_days,due,days_until,status\ncapital-of-peru,2,0,2,2026-08-02,2.50,6,2026-08-08,5,scheduled',
  );
});

test('spaced-repetition-scheduler deep-link runs FSRS with desired retention', async ({ page }) => {
  const params = new URLSearchParams({
    reviews: `card, date, grade
capital-of-peru, 2026-08-01, good
capital-of-peru, 2026-08-02, easy`,
    algorithm: 'fsrs',
    grade_scale: 'anki',
    today: '2026-08-03',
    desired_retention: '0.95',
    output: 'json',
    sort: 'due',
  });
  await page.goto(`/tools/spaced-repetition-scheduler/?${params.toString()}`);

  await expect(page.locator('#in-algorithm')).toHaveValue('fsrs', { timeout: 15_000 });
  await expect(page.locator('#in-grade_scale')).toHaveValue('anki');
  await expect(page.locator('#in-desired_retention')).toHaveValue('0.95');
  const out = await outputText(page);
  expect(out).toContain('"algorithm": "fsrs"');
  expect(out).toContain('"desired_retention": 0.95');
  expect(out).toContain('"difficulty": 3.68');
  expect(out).toContain('"stability": 12.73');
  expect(out).toContain('"due": "2026-08-07"');
});

test('spaced-repetition-scheduler covers due filter, forecast, and boundary validation', async ({ page }) => {
  await page.goto('/tools/spaced-repetition-scheduler/');
  await setField(
    page,
    '#in-reviews',
    `card, date, grade
capital-of-peru, 2026-08-01, good
capital-of-peru, 2026-08-02, good
new-card`,
  );
  await setField(page, '#in-today', '2026-08-03');
  await page.check('#in-only_due');
  await expect(page.locator('#tool-output')).toContainText('new-card', { timeout: 15_000 });
  expect(await outputText(page)).not.toContain('capital-of-peru');

  await page.uncheck('#in-only_due');
  await page.selectOption('#in-output', 'forecast');
  await setField(page, '#in-forecast_reviews', '50');
  await page.selectOption('#in-forecast_grade', 'easy');
  await expect(page.locator('#tool-output')).toContainText('50  6207-04-30', { timeout: 15_000 });

  await setField(page, '#in-forecast_reviews', '51');
  await expect(page.locator('#tool-output')).toContainText('forecast_reviews must be between 1 and 50', {
    timeout: 15_000,
  });
});
