import { test, expect } from './fixtures';

const DEFAULT_SUMMARY = `Heart-rate training zones — Karvonen heart-rate reserve, five-zone model

Age 30 years · resting HR 60 bpm
Maximum HR 187 bpm — Tanaka estimate (208 - 0.7 x age)
Heart-rate reserve 127 bpm — maximum minus resting

Zone 1  Recovery       50-60%  124-136 bpm  Active recovery, warm-up and cool-down; conversation stays effortless
Zone 2  Aerobic base   60-70%  136-149 bpm  Long easy endurance work; the highest share of fat as fuel
Zone 3  Tempo          70-80%  149-162 bpm  Steady aerobic development around marathon pace
Zone 4  Threshold      80-90%  162-174 bpm  Lactate-threshold and 10K-pace intervals; talking gets hard
Zone 5  VO2 max       90-100%  174-187 bpm  Short maximal intervals and sprints; minutes, not hours`;

async function runWasm(
  page: any,
  age = 30,
  restingHr = 60,
  maxHr = 0,
  maxHrFormula = 'tanaka',
  method = 'karvonen',
  model = 'five-zone',
  intensity = 0,
  output = 'summary',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/heart-rate-zones/gizza_ai_heart_rate_zones_web.js');
    await mod.default('/tools/heart-rate-zones/gizza_ai_heart_rate_zones_web_bg.wasm');
    return mod.run(
      args.age,
      args.restingHr,
      args.maxHr,
      args.maxHrFormula,
      args.method,
      args.model,
      args.intensity,
      args.output,
    );
  }, { age, restingHr, maxHr, maxHrFormula, method, model, intensity, output });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('heart-rate-zones wasm covers methods, formulas, models, formats and boundaries', async ({ page }) => {
  await page.goto('/tools/heart-rate-zones/');
  await page.waitForSelector('#in-age');

  expect((await runWasm(page)).trim()).toBe(DEFAULT_SUMMARY);

  const table = await runWasm(page, 35, 55, 0, 'tanaka', 'zoladz', 'five-zone', 0, 'table');
  expect(table).toContain('| Method | Zoladz fixed bpm offsets below maximum |');
  expect(table).toContain('| 1 | Recovery | max -55 to -45 bpm | 129-139 bpm |');
  expect(table).toContain('| 5 | VO2 max | max -15 to -5 bpm | 169-179 bpm |');

  const json = await runWasm(page, 55, 68, 0, 'fox', 'percent-max', 'aha', 0, 'json');
  expect(JSON.parse(json)).toMatchObject({
    age: 55,
    resting_hr: 68,
    max_hr: 165,
    max_hr_source: 'fox',
    method: 'percent-max',
    model: 'aha',
    zones: [
      { zone: 1, name: 'Moderate intensity', low_bpm: 83, high_bpm: 115 },
      { zone: 2, name: 'Vigorous intensity', low_bpm: 115, high_bpm: 140 },
    ],
  });

  await expect(runWasm(page, 120, 25, 250, 'oakland', 'karvonen', 'five-zone', 100, 'summary'))
    .resolves.toContain('Target at 100% intensity: 250 bpm');
  await expect(runWasm(page, 42, 55, 0, 'gulati', 'karvonen', 'three-zone', 70, 'summary'))
    .resolves.toContain('three-zone polarized model');
  await expect(runWasm(page, 42, 55, 0, 'nes', 'karvonen', 'five-zone', 0, 'summary'))
    .resolves.toContain('Nes estimate');
  await expect(runWasm(page, 42, 55, 0, 'inbar', 'karvonen', 'five-zone', 0, 'summary'))
    .resolves.toContain('Inbar estimate');
  await expect(runWasm(page, 4, 60, 0, 'tanaka', 'karvonen', 'five-zone', 0, 'summary'))
    .rejects.toThrow('expected age between 5 and 120 years, got 4');
});

test('heart-rate-zones page renders exact default output and CLI example', async ({ page }) => {
  const params = new URLSearchParams({
    age: '30',
    resting_hr: '60',
    max_hr: '0',
    max_hr_formula: 'tanaka',
    method: 'karvonen',
    model: 'five-zone',
    intensity: '0',
    output: 'summary',
  });
  await page.goto(`/tools/heart-rate-zones/?${params.toString()}`);

  await expect(page.locator('#in-age')).toHaveValue('30', { timeout: 15_000 });
  await expect(page.locator('#in-resting_hr')).toHaveValue('60');
  await expect(page.locator('#in-max_hr_formula')).toHaveValue('tanaka');
  await expect(page.locator('#in-method')).toHaveValue('karvonen');
  await expect(page.locator('#tool-output')).toHaveText(DEFAULT_SUMMARY, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool heart-rate-zones');
  expect(cli).toContain('"30"');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('heart-rate-zones deep-link pre-fills and outputs AHA json', async ({ page }) => {
  const params = new URLSearchParams({
    age: '55',
    resting_hr: '68',
    max_hr: '0',
    max_hr_formula: 'fox',
    method: 'percent-max',
    model: 'aha',
    intensity: '0',
    output: 'json',
  });

  await page.goto(`/tools/heart-rate-zones/?${params.toString()}`);
  await expect(page.locator('#in-age')).toHaveValue('55', { timeout: 15_000 });
  await expect(page.locator('#in-max_hr_formula')).toHaveValue('fox');
  await expect(page.locator('#in-method')).toHaveValue('percent-max');
  await expect(page.locator('#in-model')).toHaveValue('aha');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"max_hr": 165', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"name": "Moderate intensity"');
  expect(JSON.parse(await outputText(page))).toMatchObject({ method: 'percent-max', model: 'aha' });
});
