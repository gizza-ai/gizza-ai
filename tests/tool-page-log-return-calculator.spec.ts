import { test, expect } from './fixtures';

const tool = '/tools/log-return-calculator/';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  prices: string,
  hasHeader = 'false',
  periodsPerYear = '252',
  unit = 'percent',
  decimals = '4',
  output = 'summary',
): Promise<string> {
  return await page.evaluate(
    async (args) => {
      const mod = await import('/tools/log-return-calculator/gizza_ai_log_return_calculator_web.js');
      await mod.default('/tools/log-return-calculator/gizza_ai_log_return_calculator_web_bg.wasm');
      return mod.run(
        args.prices,
        args.hasHeader,
        args.periodsPerYear,
        args.unit,
        args.decimals,
        args.output,
      );
    },
    { prices, hasHeader, periodsPerYear, unit, decimals, output },
  );
}

test('log-return-calculator page renders exact CSV output for a two-price series', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-prices'), '100\n120');
  await page.selectOption('#in-periods_per_year', '252');
  await page.selectOption('#in-unit', 'percent');
  await page.fill('#in-decimals', '4');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toHaveText(
    'label,price,log_return_percent,simple_return_percent,cumulative_log_return_percent\n1,100,,,\n2,120,18.2322,20.0000,18.2322',
    { timeout: 15000 },
  );
});

test('log-return-calculator deep link skips a header row and uses decimal table output', async ({ page }) => {
  const qs = new URLSearchParams({
    prices: 'date,nav\n2024-01-31,100\n2024-02-29,105\n2024-03-31,103',
    has_header: 'true',
    periods_per_year: '12',
    unit: 'decimal',
    decimals: '6',
    output: 'table',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-has_header')).toBeChecked({ timeout: 15000 });
  await expect(page.locator('#in-periods_per_year')).toHaveValue('12');
  await expect(page.locator('#in-unit')).toHaveValue('decimal');
  await expect(page.locator('#in-decimals')).toHaveValue('6');
  await expect(page.locator('#in-output')).toHaveValue('table');
  await expect(page.locator('#tool-output')).toContainText('+0.048790');
  await expect(page.locator('#tool-output')).toContainText('-0.019231');
});

test('log-return-calculator wasm covers enum choices accepted formats and cap boundaries', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-prices');

  expect(await runWasm(page, '100\n120', 'false', '252', 'decimal', '6', 'summary')).toContain(
    'total log return:        +0.182322',
  );
  expect(await runWasm(page, 'date,nav\n2024-01-31,100\n2024-02-29,105', 'true', '12', 'percent', '4', 'csv')).toBe(
    'label,price,log_return_percent,simple_return_percent,cumulative_log_return_percent\n2024-01-31,100,,,\n2024-02-29,105,4.8790,5.0000,4.8790',
  );
  expect(await runWasm(page, '$1,000.00\n€1,100.00', 'false', '252', 'percent', '10', 'table')).toContain(
    '+9.5310180000%',
  );
  await expect(runWasm(page, '100\n120', 'false', '252', 'percent', '11', 'summary')).rejects.toThrow(
    /decimals must be between 0 and 10/,
  );

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/log-return-calculator/gizza_ai_log_return_calculator_web.js');
    await mod.default('/tools/log-return-calculator/gizza_ai_log_return_calculator_web_bg.wasm');
    const atCap = Array.from({ length: 2000 }, (_, i) => String(100 + i)).join('\n');
    const overCap = atCap + '\n2100';
    const call = (prices: string) => {
      try {
        return { ok: true, value: mod.run(prices, 'false', '252', 'percent', '4', 'summary').slice(0, 32) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toBe('Log returns for 2000 prices → 19');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/maximum is 2000/);
});

test('log-return-calculator page ships workflow example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Daily closes → summary',
    'Monthly NAVs → CSV',
    'One-line series → decimal log values',
    'Symmetry check: 50 → 70 → 50',
  ]);
});
