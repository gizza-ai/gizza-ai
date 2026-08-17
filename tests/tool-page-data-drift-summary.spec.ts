import { test, expect } from './fixtures';

const reference = 'id,amount,country\n1,10,US\n2,12,US\n3,11,FR\n4,13,FR';
const current = 'id,amount,country\n1,40,US\n2,44,US\n3,,FR\n4,47,DE';

async function runTool(
  page,
  params: {
    reference?: string;
    current?: string;
    delimiter?: string;
    header?: string;
    columns?: string;
    method?: string;
    threshold?: string;
    bins?: string;
    maxCategories?: string;
    driftShare?: string;
    ignoreCase?: string;
    sort?: string;
    format?: string;
  } = {}
) {
  const p = {
    reference,
    current,
    delimiter: 'comma',
    header: 'true',
    columns: '',
    method: 'psi',
    threshold: '0.2',
    bins: '10',
    maxCategories: '20',
    driftShare: '0.5',
    ignoreCase: 'false',
    sort: 'drift',
    format: 'table',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/data-drift-summary/gizza_ai_data_drift_summary_web.js');
    await mod.default('/tools/data-drift-summary/gizza_ai_data_drift_summary_web_bg.wasm');
    return mod.run(
      args.reference,
      args.current,
      args.delimiter,
      args.header,
      args.columns,
      args.method,
      args.threshold,
      args.bins,
      args.maxCategories,
      args.driftShare,
      args.ignoreCase,
      args.sort,
      args.format
    );
  }, p);
}

test('data-drift-summary wasm returns an exact default table report', async ({ page }) => {
  await page.goto('/tools/data-drift-summary/');
  await page.waitForSelector('#in-reference');

  const out = await runTool(page, { columns: 'amount,country' });
  expect(out).toContain('DATA DRIFT SUMMARY');
  expect(out).toContain('2 columns compared · 2 drifted (100.0%) · DATASET DRIFT DETECTED');
  expect(out).toContain('DRIFT  amount — categorical · psi 25.1460');
  expect(out).toContain('nulls     0.0% → 25.0% (+25.0)');
  expect(out).toContain('new       DE (1)');
});

test('data-drift-summary deep-link prefills controls and page outputs drift text', async ({ page }) => {
  await page.goto('/tools/data-drift-summary/?method=jsd&threshold=0.1&bins=8&max_categories=10&drift_share=0.25&ignore_case=true&sort=name&format=markdown&columns=country');
  await page.waitForSelector('#in-reference');
  await expect(page.locator('#in-method')).toHaveValue('jsd');
  await expect(page.locator('#in-threshold')).toHaveValue('0.1');
  await expect(page.locator('#in-bins')).toHaveValue('8');
  await expect(page.locator('#in-max_categories')).toHaveValue('10');
  await expect(page.locator('#in-drift_share')).toHaveValue('0.25');
  await expect(page.locator('#in-ignore_case')).toBeChecked();
  await expect(page.locator('#in-sort')).toHaveValue('name');
  await expect(page.locator('#in-format')).toHaveValue('markdown');
  await expect(page.locator('#in-columns')).toHaveValue('country');

  await page.fill('#in-reference', reference);
  await page.fill('#in-current', current);
  await expect(page.locator('#tool-output')).toContainText('| Column | Drift | Score |', { timeout: 10_000 });
  await expect(page.locator('#tool-output')).toContainText('country');
});

test('data-drift-summary wasm covers advertised formats, delimiters, booleans and boundaries', async ({ page }) => {
  await page.goto('/tools/data-drift-summary/');
  await page.waitForSelector('#in-reference');

  const json = await runTool(page, { format: 'json', method: 'jsd', threshold: '0.1', columns: 'amount' });
  const parsed = JSON.parse(json);
  expect(parsed.summary.dataset_drift).toBe(true);
  expect(parsed.columns[0].column).toBe('amount');
  expect(parsed.summary.method).toBe('jsd');

  const csv = await runTool(page, { format: 'csv', columns: 'country', sort: 'name' });
  expect(csv).toContain('column,status,drift_score,drifted');
  expect(csv).toContain('country,compared,3.2806,true');

  const tsvRef = 'id\tcity\n1\tNew York\n2\tParis';
  const tsvCur = 'id\tcity\n1\tnew york \n2\tParis';
  const folded = await runTool(page, { reference: tsvRef, current: tsvCur, delimiter: 'tab', ignoreCase: 'true', columns: 'city' });
  expect(folded).toContain('1 columns compared · 0 drifted (0.0%)');

  const headerless = await runTool(page, { reference: '1|10\n2|12', current: '1|20\n2|22', delimiter: 'pipe', header: 'false', bins: '2', maxCategories: '2', driftShare: '1', columns: 'col2' });
  expect(headerless).toContain('col2');

  await expect(runTool(page, { bins: '1' })).rejects.toThrow(/bins must be between 2 and 50/);
});

test('data-drift-summary ships useful example presets', async ({ page }) => {
  await page.goto('/tools/data-drift-summary/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await page.click('.tool-example-chip:has-text("New and missing categories")');
  await expect(page.locator('#in-columns')).toHaveValue('plan');
  await expect(page.locator('#in-format')).toHaveValue('table');
});
