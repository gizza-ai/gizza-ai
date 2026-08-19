import { test, expect } from './fixtures';

const tool = '/tools/churn-cohort-retention/';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  data: string,
  signups = '',
  user = 'user',
  date = 'date',
  signupDate = 'signup_date',
  granularity = 'month',
  periods = '6',
  metric = 'retention',
  values = 'percent',
  asOf = '',
  header = 'true',
  delimiter = 'comma',
  format = 'table',
): Promise<string> {
  return await page.evaluate(
    async ({ data, signups, user, date, signupDate, granularity, periods, metric, values, asOf, header, delimiter, format }) => {
      const mod = await import('/tools/churn-cohort-retention/gizza_ai_churn_cohort_retention_web.js');
      await mod.default('/tools/churn-cohort-retention/gizza_ai_churn_cohort_retention_web_bg.wasm');
      return mod.run(data, signups, user, date, signupDate, granularity, periods, metric, values, asOf, header, delimiter, format);
    },
    { data, signups, user, date, signupDate, granularity, periods, metric, values, asOf, header, delimiter, format },
  );
}

test('churn-cohort-retention page renders monthly retention counts and percents', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-data'), 'user,date\nu1,2024-01-05\nu1,2024-02-03\nu1,2024-03-01\nu2,2024-01-20\nu2,2024-03-02\nu3,2024-02-10\nu3,2024-03-11');
  await page.fill('#in-periods', '3');
  await page.selectOption('#in-values', 'both');
  await page.fill('#in-as_of', '2024-04-01');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Cohort retention', { timeout: 15_000 });
  await expect(out).toContainText('2024-01');
  await expect(out).toContainText('2 (100%)');
  await expect(out).toContainText('average');
  await expect(out).toContainText('Retention curve');
});

test('churn-cohort-retention deep link pre-fills churn JSON settings', async ({ page }) => {
  const qs = new URLSearchParams({
    data: 'user,date\nu1,2024-01-05\nu1,2024-02-03\nu2,2024-01-20\nu3,2024-02-10',
    signups: 'user,signup_date\nu1,2024-01-01\nu2,2024-01-15\nu3,2024-01-18',
    user: 'user',
    date: 'date',
    signup_date: 'signup_date',
    granularity: 'month',
    periods: '2',
    metric: 'churn',
    values: 'percent',
    as_of: '2024-03-01',
    header: 'true',
    delimiter: 'comma',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue('user,date\nu1,2024-01-05\nu1,2024-02-03\nu2,2024-01-20\nu3,2024-02-10', { timeout: 15_000 });
  await expect(page.locator('#in-signups')).toHaveValue('user,signup_date\nu1,2024-01-01\nu2,2024-01-15\nu3,2024-01-18');
  await expect(page.locator('#in-metric')).toHaveValue('churn');
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-header')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"metric"');
  await expect(page.locator('#tool-output')).toContainText('"churn"');
});

test('churn-cohort-retention wasm covers formats, enums, boundary, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  const monthly = 'user,date\nu1,2024-01-05\nu1,2024-02-03\nu1,2024-03-01\nu2,2024-01-20\nu2,2024-03-02\nu3,2024-02-10\nu3,2024-03-11';
  const table = await runWasm(page, monthly, '', 'user', 'date', 'signup_date', 'month', '3', 'retention', 'count', '2024-04-01');
  expect(table).toContain('cohort');
  expect(table).toContain('average');

  const csv = await runWasm(page, monthly, '', 'user', 'date', 'signup_date', 'month', '3', 'retention', 'percent', '2024-04-01', 'true', 'comma', 'csv');
  expect(csv).toContain('cohort,users,P0,P1,P2,P3');

  const json = JSON.parse(await runWasm(page, monthly, '', 'user', 'date', 'signup_date', 'month', '3', 'retention', 'percent', '2024-04-01', 'true', 'comma', 'json'));
  expect(json.metric).toBe('retention');
  expect(json.cohorts.length).toBe(2);

  const weekly = JSON.parse(await runWasm(page, 'user|date\nu1|2024-01-01\nu1|2024-01-08\nu2|2024-01-02\nu2|2024-01-15\nu3|2024-01-09', '', 'user', 'date', 'signup_date', 'week', '3', 'retention', 'percent', '2024-01-29', 'true', 'pipe', 'json'));
  expect(weekly.granularity).toBe('week');

  const day = await runWasm(page, 'u1;2024-01-01\nu1;2024-01-02\nu2;2024-01-01', '', '1', '2', '2', 'day', '1', 'churn', 'both', '2024-01-02', 'false', 'semicolon', 'table');
  expect(day).toContain('Cohort churn');

  const maxPeriods = JSON.parse(await runWasm(page, monthly, '', 'user', 'date', 'signup_date', 'month', '36', 'retention', 'percent', '2024-04-01', 'true', 'comma', 'json'));
  expect(maxPeriods.periods).toBe(36);

  await expect(runWasm(page, '', '')).rejects.toThrow(/activity data is empty/);
  await expect(runWasm(page, 'user,date\nu1,03\/04\/2024')).rejects.toThrow(/ambiguous/);
  await expect(runWasm(page, monthly, '', 'missing')).rejects.toThrow(/not found in the header row/);
  await expect(runWasm(page, monthly, '', 'user', 'date', 'signup_date', 'month', '37')).rejects.toThrow(/periods must be between 1 and 36/);
});

test('churn-cohort-retention ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Monthly retention from events',
    'Signup table, churn view',
    'Weekly cohorts as JSON',
  ]);
});
