import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('period-predictor page predicts a single 28-day cycle (exact JSON)', async ({ page }) => {
  await page.goto('/tools/period-predictor/');

  await page.fill('#in-last_period', '2026-07-01');
  await page.fill('#in-cycle_length', '28');
  await page.fill('#in-period_length', '5');
  await page.fill('#in-luteal_phase', '14');
  await page.fill('#in-cycles', '1');

  // Wait for the wasm to produce output, then assert the exact pretty-printed JSON.
  await expect(page.locator('#tool-output')).toContainText('"next_period_start": "2026-07-29"', {
    timeout: 15000,
  });

  const expected = [
    '{',
    '  "last_period": "2026-07-01",',
    '  "cycle_length": 28,',
    '  "period_length": 5,',
    '  "luteal_phase": 14,',
    '  "next_period_start": "2026-07-29",',
    '  "cycles": [',
    '    {',
    '      "cycle": 1,',
    '      "period_start": "2026-07-29",',
    '      "period_start_weekday": "Wednesday",',
    '      "period_end": "2026-08-02",',
    '      "ovulation_date": "2026-07-15",',
    '      "fertile_window_start": "2026-07-10",',
    '      "fertile_window_end": "2026-07-15"',
    '    }',
    '  ],',
    '  "summary": "Next period expected 2026-07-29 (Wednesday). 1 cycle predicted on a 28-day cycle."',
    '}',
  ].join('\n');

  expect(await outputText(page)).toBe(expected);
});

test('period-predictor page honours a query-param deep link across six cycles', async ({ page }) => {
  await page.goto(
    '/tools/period-predictor/?last_period=2026-07-01&cycle_length=28&period_length=5&luteal_phase=14&cycles=6',
  );

  // Deep-linked values populate every control.
  await expect(page.locator('#in-last_period')).toHaveValue('2026-07-01', { timeout: 15000 });
  await expect(page.locator('#in-cycle_length')).toHaveValue('28');
  await expect(page.locator('#in-cycles')).toHaveValue('6');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"next_period_start": "2026-07-29"', { timeout: 15000 });
  // Six cycles, earliest first — the last predicted start crosses into December.
  await expect(out).toContainText('"period_start": "2026-08-26"');
  await expect(out).toContainText('"period_start": "2026-12-16"');
  await expect(out).toContainText(
    '"summary": "Next period expected 2026-07-29 (Wednesday). 6 cycles predicted on a 28-day cycle."',
  );
});
