import { test, expect } from './fixtures';

const tool = '/tools/pagerank-ranker/';
const sample = `home -> docs
home -> pricing
docs -> api
api -> docs
pricing -> home`;

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    input: sample,
    input_format: 'edge-list',
    directed: 'true',
    weighted: 'false',
    damping: '0.85',
    max_iter: '100',
    tolerance: '0.000001',
    dangling: 'redistribute',
    personalization: '',
    top: '0',
    decimals: '6',
    format: 'text',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/pagerank-ranker/gizza_ai_pagerank_ranker_web.js');
    await mod.default('/tools/pagerank-ranker/gizza_ai_pagerank_ranker_web_bg.wasm');
    return mod.run(
      args.input,
      args.input_format,
      args.directed,
      args.weighted,
      args.damping,
      args.max_iter,
      args.tolerance,
      args.dangling,
      args.personalization,
      args.top,
      args.decimals,
      args.format,
    );
  }, p);
}

test('pagerank-ranker page renders a ranked link graph', async ({ page }) => {
  await page.goto(tool);
  await setField(page, '#in-input', sample);
  await page.selectOption('#in-input_format', 'edge-list');
  await page.selectOption('#in-format', 'text');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('PageRank — 4 nodes, 5 edges', { timeout: 20_000 });
  await expect(output).toContainText('docs');
  await expect(output).toContainText('api');
  await expect(output).toContainText('Total PageRank mass: 1.000000');
});

test('pagerank-ranker deep link prefills matrix JSON output', async ({ page }) => {
  const matrix = '0 1 1\n0 0 1\n1 0 0';
  const qs = new URLSearchParams({
    input: matrix,
    input_format: 'matrix',
    directed: 'true',
    weighted: 'false',
    damping: '0.85',
    max_iter: '100',
    tolerance: '0.000001',
    dangling: 'redistribute',
    personalization: '',
    top: '0',
    decimals: '6',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(matrix, { timeout: 15_000 });
  await expect(page.locator('#in-input_format')).toHaveValue('matrix');
  await expect(page.locator('#in-format')).toHaveValue('json');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"source_format": "adjacency matrix"', { timeout: 20_000 });
  await expect(output).toContainText('"node": "3"');
  await expect(output).toContainText('"converged": true');
});

test('pagerank-ranker wasm covers formats, enums, checkboxes, matrices, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  const weightedCsv = await runWasm(page, {
    input: 'seed -> hub : 1\nhub -> big : 9\nhub -> small : 1',
    weighted: 'true',
    format: 'csv',
    top: '2',
    decimals: '4',
  });
  expect(weightedCsv).toContain('rank,node,pagerank,share_percent');
  expect(weightedCsv).toContain('big');
  expect(weightedCsv.split('\n').filter(Boolean)).toHaveLength(3);

  const matrixJson = JSON.parse(await runWasm(page, {
    input: '0 1 1\n0 0 1\n1 0 0',
    input_format: 'matrix',
    directed: 'true',
    format: 'json',
  }));
  expect(matrixJson.source_format).toBe('adjacency matrix');
  expect(matrixJson.ranking[0].node).toBe('3');

  const undirected = await runWasm(page, {
    input: 'hub - a\nhub - b\nhub - c',
    directed: 'false',
    dangling: 'self-loop',
    format: 'text',
  });
  expect(undirected).toContain('undirected');
  expect(undirected).toContain('hub');

  const dropped = await runWasm(page, {
    input: 'a -> b\nb -> c',
    dangling: 'drop',
    format: 'text',
  });
  expect(dropped).toContain('dangling: drop');

  await expect(runWasm(page, { input: 'a -> b : -2', weighted: 'true' })).rejects.toThrow(
    /negative weight/,
  );
});
