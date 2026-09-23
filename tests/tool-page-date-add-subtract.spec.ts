import { test, expect } from './fixtures';

const tool = '/tools/date-add-subtract/';

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    date: '2026-06-19',
    operation: 'add',
    years: '',
    months: '',
    weeks: '',
    days: '90',
    hours: '',
    minutes: '',
    seconds: '',
    skip_weekends: 'false',
    weekend_days: 'sat-sun',
    holidays: '',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/date-add-subtract/gizza_ai_date_add_subtract_web.js');
    await mod.default('/tools/date-add-subtract/gizza_ai_date_add_subtract_web_bg.wasm');
    return mod.run(
      args.date,
      args.operation,
      args.years,
      args.months,
      args.weeks,
      args.days,
      args.hours,
      args.minutes,
      args.seconds,
      args.skip_weekends,
      args.weekend_days,
      args.holidays,
    );
  }, p);
}

test('date-add-subtract page renders a 90-day date shift', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-date', '2026-06-19');
  await page.fill('#in-days', '90');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"result": "2026-09-17"', { timeout: 20_000 });
  await expect(output).toContainText('"weekday": "Thursday"');
  await expect(output).toContainText('90 days after 2026-06-19 is Thursday, 17 September 2026');
});

test('date-add-subtract deep link can prefill subtract and weekend options', async ({ page }) => {
  const qs = new URLSearchParams({
    date: '2026-09-17',
    operation: 'subtract',
    years: '0',
    months: '0',
    weeks: '0',
    days: '90',
    hours: '0',
    minutes: '0',
    seconds: '0',
    skip_weekends: 'true',
    weekend_days: 'fri-sat',
    holidays: '2026-07-03',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-date')).toHaveValue('2026-09-17', { timeout: 15_000 });
  await expect(page.locator('#in-operation')).toHaveValue('subtract');
  await expect(page.locator('#in-days')).toHaveValue('90');
  await expect(page.locator('#in-skip_weekends')).toBeChecked();
  await expect(page.locator('#in-weekend_days')).toHaveValue('fri-sat');
  await expect(page.locator('#in-holidays')).toHaveValue('2026-07-03');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"business_day_mode": true', { timeout: 20_000 });
  await expect(output).toContainText('"operation": "subtract"');
});

test('date-add-subtract wasm covers enum choices, business days, caps, and date forms', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-date');

  const add = JSON.parse(await runWasm(page));
  expect(add.result).toBe('2026-09-17');
  expect(add.weekday).toBe('Thursday');

  const subtract = JSON.parse(await runWasm(page, { date: '2026-09-17', operation: 'subtract' }));
  expect(subtract.result).toBe('2026-06-19');
  expect(subtract.calendar_days_moved).toBe(-90);

  const business = JSON.parse(await runWasm(page, { days: '1', skip_weekends: 'true' }));
  expect(business.result).toBe('2026-06-22');
  expect(business.business_day_mode).toBe(true);
  expect(business.skipped_days).toBe(2);

  const gulfWeekend = JSON.parse(
    await runWasm(page, { date: '2026-06-18', days: '1', skip_weekends: 'true', weekend_days: 'fri-sat' }),
  );
  expect(gulfWeekend.result).toBe('2026-06-21');

  const altDate = JSON.parse(await runWasm(page, { date: 'June 19, 2026', days: '1' }));
  expect(altDate.input).toBe('2026-06-19');
  expect(altDate.result).toBe('2026-06-20');

  const cap = JSON.parse(await runWasm(page, { years: '10000', days: '0' }));
  expect(cap.result).toBe('+12026-06-19');

  await expect(runWasm(page, { days: '1.5' })).rejects.toThrow(/days must be a whole number/);
  await expect(runWasm(page, { days: '200001', skip_weekends: 'true' })).rejects.toThrow(
    /business-day mode supports at most/,
  );
});
