import { test, expect } from './fixtures';

async function setBigTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  input: string,
  layout = 'auto',
  maxGap = '0',
  direction = 'both',
  edges = 'leave',
  naTokens = '',
  decimals = '6',
  output = 'values',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/linear-interpolate-gaps/gizza_ai_linear_interpolate_gaps_web.js');
    await mod.default('/tools/linear-interpolate-gaps/gizza_ai_linear_interpolate_gaps_web_bg.wasm');
    return mod.run(
      args.input,
      args.layout,
      args.maxGap,
      args.direction,
      args.edges,
      args.naTokens,
      args.decimals,
      args.output,
    );
  }, { input, layout, maxGap, direction, edges, naTokens, decimals, output });
}

test('linear-interpolate-gaps wasm fills values exactly', async ({ page }) => {
  await page.goto('/tools/linear-interpolate-gaps/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, '10\n\n\n\n20')).resolves.toBe('10\n12.5\n15\n17.5\n20');
  await expect(runWasm(page, 'minute,reading\n0,20\n5,\n10,\n20,50', 'auto', '0', 'both', 'leave', '', '2')).resolves.toBe('0,20\n5,27.5\n10,35\n20,50');
});

test('linear-interpolate-gaps wasm covers advertised options and errors', async ({ page }) => {
  await page.goto('/tools/linear-interpolate-gaps/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, '0,,,,4', 'values', '1', 'forward')).resolves.toBe('0\n1\n\n\n4');
  await expect(runWasm(page, '0,,,,4', 'values', '1', 'backward')).resolves.toBe('0\n\n\n3\n4');
  await expect(runWasm(page, '0,,,,4', 'values', '3')).resolves.toBe('0\n1\n2\n3\n4');
  await expect(runWasm(page, 'NA\nNA\n2\n4\nNA\nNA', 'values', '0', 'both', 'hold')).resolves.toBe('2\n2\n2\n4\n4\n4');
  await expect(runWasm(page, 'NA\nNA\n2\n4\nNA\nNA', 'values', '0', 'both', 'extrapolate')).resolves.toBe('-2\n0\n2\n4\n6\n8');
  await expect(runWasm(page, '1\n-999\n-999\n4', 'values', '0', 'both', 'leave', '-999', '0')).resolves.toBe('1\n2\n3\n4');

  const csv = await runWasm(page, '1,,3', 'values', '0', 'both', 'leave', '', '6', 'csv');
  expect(csv).toBe('index,value,status\n1,1,known\n2,2,filled\n3,3,known\n');

  const json = await runWasm(page, '1,,,,5,,', 'values', '1', 'both', 'leave', '', '6', 'json');
  expect(json).toContain('"count": 7');
  expect(json).toContain('"status": "partial"');
  expect(json).toContain('"kind": "trailing"');

  await expect(runWasm(page, '1,10\n5,20\n3,30', 'xy')).rejects.toThrow(/strictly increasing/);
});

test('linear-interpolate-gaps page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/linear-interpolate-gaps/');
  await setBigTextarea(page, '#in-input', '0,,,,4');
  await page.fill('#in-max_gap', '1');
  await page.selectOption('#in-direction', 'forward');

  await expect(page.locator('#tool-output')).toHaveText('0\n1\n\n\n4', { timeout: 15_000 });
});

test('linear-interpolate-gaps deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    input: '1\n-999\n-999\n4',
    layout: 'values',
    max_gap: '0',
    direction: 'both',
    edges: 'leave',
    na_tokens: '-999',
    decimals: '0',
    output: 'values',
  });

  await page.goto(`/tools/linear-interpolate-gaps/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('1\n-999\n-999\n4', { timeout: 15_000 });
  await expect(page.locator('#in-layout')).toHaveValue('values');
  await expect(page.locator('#in-na_tokens')).toHaveValue('-999');
  await expect(page.locator('#in-decimals')).toHaveValue('0');
  await expect(page.locator('#tool-output')).toHaveText('1\n2\n3\n4', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool linear-interpolate-gaps');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
