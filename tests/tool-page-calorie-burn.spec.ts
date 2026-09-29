import { test, expect } from './fixtures';

const DEFAULT_SUMMARY = `Calories burned — Walking, moderate (3 mph / 4.8 km/h)

Body weight 70 kg (154.3 lb) · duration 30 min · MET 3.5 (Compendium value for this activity)
Basis: gross — all energy used during the session, resting metabolism included

Energy burned        129 kcal
Per minute           4.3 kcal/min
Per hour             257 kcal/h
Activity volume      105 MET-minutes — 4.8 such sessions reach the 500 MET-min/week minimum
Oxygen uptake        12.3 ml/kg/min — 25.7 L of oxygen over the session
Body-fat equivalent  17 g — at 7700 kcal per kg of body fat

Same 30 min at 70 kg for comparison
  Sitting, desk work                       1.5 MET      55 kcal
  Walking, moderate (3 mph / 4.8 km/h)     3.5 MET     129 kcal
  Walking, brisk (4 mph / 6.4 km/h)          5 MET     184 kcal
  Cycling, moderate (12-14 mph)              8 MET     294 kcal
  Jogging, general                           7 MET     257 kcal
  Running, 6 mph / 9.7 km/h (10 min/mi)    9.8 MET     360 kcal
  Swimming laps, freestyle moderate        8.3 MET     305 kcal
  Weight training, vigorous                  6 MET     221 kcal`;

async function runWasm(
  page: any,
  weight = 70,
  weightUnit = 'kg',
  duration = 30,
  durationUnit = 'minutes',
  activity = 'walking-moderate',
  met = 0,
  basis = 'gross',
  output = 'summary',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/calorie-burn/gizza_ai_calorie_burn_web.js');
    await mod.default('/tools/calorie-burn/gizza_ai_calorie_burn_web_bg.wasm');
    return mod.run(
      args.weight,
      args.weightUnit,
      args.duration,
      args.durationUnit,
      args.activity,
      args.met,
      args.basis,
      args.output,
    );
  }, { weight, weightUnit, duration, durationUnit, activity, met, basis, output });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('calorie-burn wasm covers units, activities, basis, custom MET, formats and boundaries', async ({ page }) => {
  await page.goto('/tools/calorie-burn/');
  await page.waitForSelector('#in-weight');

  expect((await runWasm(page)).trim()).toBe(DEFAULT_SUMMARY);

  const netSwim = await runWasm(page, 68, 'kg', 45, 'minutes', 'swimming-laps-moderate', 0, 'net', 'summary');
  expect(netSwim).toContain('Basis: net — only the energy above resting metabolism, so 8.3 - 1 = 7.3 MET is charged');
  expect(netSwim).toContain('Energy burned        391 kcal');
  expect(netSwim).toContain('Activity volume      373.5 MET-minutes');

  const table = await runWasm(page, 185, 'lb', 1, 'hours', 'weight-training-vigorous', 0, 'gross', 'table');
  expect(table).toContain('| Body weight | 185 lb (83.9 kg) |');
  expect(table).toContain('| Duration | 1 h (60 min) |');
  expect(table).toContain('| Energy burned | 529 kcal |');

  const custom = JSON.parse(await runWasm(page, 80, 'kg', 40, 'minutes', 'custom', 7.5, 'gross', 'json'));
  expect(custom).toMatchObject({
    activity: 'custom',
    activity_label: 'Custom activity',
    met: 7.5,
    met_source: 'your own MET value',
    calories: 420,
    kcal_per_hour: 630,
    comparison: expect.arrayContaining([
      expect.objectContaining({ label: 'Cycling, moderate (12-14 mph)', met: 8, calories: 448 }),
    ]),
  });

  await expect(runWasm(page, 660, 'lb', 1440, 'minutes', 'custom', 30, 'net', 'json'))
    .resolves.toContain('"met_charged": 29');
  await expect(runWasm(page, 70, 'kg', 30, 'minutes', 'badminton', 5.7, 'gross', 'summary'))
    .resolves.toContain('Badminton, social at MET 5.7');
  await expect(runWasm(page, 19, 'kg', 30, 'minutes', 'walking-moderate', 0, 'gross', 'summary'))
    .rejects.toThrow('expected body weight between 20 and 300 kg');
});

test('calorie-burn page renders exact default output and CLI example', async ({ page }) => {
  const params = new URLSearchParams({
    weight: '70',
    weight_unit: 'kg',
    duration: '30',
    duration_unit: 'minutes',
    activity: 'walking-moderate',
    met: '0',
    basis: 'gross',
    output: 'summary',
  });
  await page.goto(`/tools/calorie-burn/?${params.toString()}`);

  await expect(page.locator('#in-weight')).toHaveValue('70', { timeout: 15_000 });
  await expect(page.locator('#in-duration')).toHaveValue('30');
  await expect(page.locator('#in-activity')).toHaveValue('walking-moderate');
  await expect(page.locator('#in-basis')).toHaveValue('gross');
  await expect(page.locator('#tool-output')).toHaveText(DEFAULT_SUMMARY, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool calorie-burn');
  expect(cli).toContain('"70"');
  expect(cli).toContain("'duration=30'");
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('calorie-burn deep-link pre-fills and outputs custom JSON', async ({ page }) => {
  const params = new URLSearchParams({
    weight: '80',
    weight_unit: 'kg',
    duration: '40',
    duration_unit: 'minutes',
    activity: 'custom',
    met: '7.5',
    basis: 'gross',
    output: 'json',
  });

  await page.goto(`/tools/calorie-burn/?${params.toString()}`);
  await expect(page.locator('#in-weight')).toHaveValue('80', { timeout: 15_000 });
  await expect(page.locator('#in-activity')).toHaveValue('custom');
  await expect(page.locator('#in-met')).toHaveValue('7.5');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"calories": 420', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"activity_label": "Custom activity"');
  expect(JSON.parse(await outputText(page))).toMatchObject({ activity: 'custom', calories: 420 });
});
