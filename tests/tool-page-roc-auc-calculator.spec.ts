import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  data: string,
  labels = '',
  inputFormat = 'auto',
  columnOrder = 'auto',
  separator = 'auto',
  header = 'auto',
  positiveLabel = '',
  optimize = 'youden',
  costRatio = '1',
  threshold = '',
  confidenceLevel = '95',
  tableRows = '0',
  plot = 'false',
  decimals = '4',
  percent = 'false',
  format = 'json',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/roc-auc-calculator/gizza_ai_roc_auc_calculator_web.js');
    await mod.default('/tools/roc-auc-calculator/gizza_ai_roc_auc_calculator_web_bg.wasm');
    return mod.run(
      args.data,
      args.labels,
      args.inputFormat,
      args.columnOrder,
      args.separator,
      args.header,
      args.positiveLabel,
      args.optimize,
      args.costRatio,
      args.threshold,
      args.confidenceLevel,
      args.tableRows,
      args.plot,
      args.decimals,
      args.percent,
      args.format,
    );
  }, { data, labels, inputFormat, columnOrder, separator, header, positiveLabel, optimize, costRatio, threshold, confidenceLevel, tableRows, plot, decimals, percent, format });
}

const PERFECT_JSON = `{
  "observations": 4,
  "positives": 2,
  "negatives": 2,
  "positive_label": "1",
  "negative_label": "0",
  "positive_class_source": "auto-detected from a common positive label",
  "distinct_scores": 4,
  "curve_points": 5,
  "auc": 1.0000,
  "auc_trapezoidal": 1.0000,
  "gini": 1.0000,
  "standard_error": 0.0000,
  "confidence_level": 95,
  "ci_lower": 1.0000, "ci_upper": 1.0000,
  "z": null,
  "p_value": null,
  "brier_score": 0.0250,
  "interpretation": "perfect separation — every positive outranks every negative",
  "criterion": "youden",
  "cost_ratio": 1.0000,
  "chosen_cutoff":
  {
    "threshold": 0.8,
    "sensitivity": 1.0000,
    "specificity": 1.0000,
    "fpr": 0.0000,
    "ppv": 1.0000,
    "npv": 1.0000,
    "accuracy": 1.0000,
    "f1": 1.0000,
    "youden_j": 1.0000,
    "mcc": 1.0000,
    "distance_to_corner": 0.0000,
    "cost": 0.0000,
    "tp": 2, "fp": 0, "tn": 2, "fn": 0
  },
  "your_threshold": null,
  "threshold_table": [
  ]
}`;

test('roc-auc-calculator wasm computes a perfect ROC AUC exactly', async ({ page }) => {
  await page.goto('/tools/roc-auc-calculator/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, '0.9,1\n0.8,1\n0.2,0\n0.1,0')).resolves.toBe(PERFECT_JSON);
});

test('roc-auc-calculator wasm covers columns, F1, threshold and percent rates', async ({ page }) => {
  await page.goto('/tools/roc-auc-calculator/');
  await page.waitForSelector('#in-data');

  const out = await runWasm(
    page,
    '0.9\n0.8\n0.2\n0.1',
    'case\ncase\ncontrol\ncontrol',
    'columns',
    'auto',
    'newline',
    'no',
    'case',
    'f1',
    '1',
    '0.5',
    '90',
    '2',
    'false',
    '2',
    'true',
    'text',
  );
  expect(out).toContain('AUC                             1.00');
  expect(out).toContain('Chosen cutoff — maximum F1 score');
  expect(out).toContain('Your threshold');
  expect(out).toContain('100.00%');
});

test('roc-auc-calculator page renders exact JSON output and honors controls', async ({ page }) => {
  await page.goto('/tools/roc-auc-calculator/');
  await setTextarea(page, '#in-data', '0.9,1\n0.8,1\n0.2,0\n0.1,0');
  await page.locator('#in-format').selectOption('json');
  await page.locator('#in-plot').setChecked(false);
  await page.locator('#in-table_rows').fill('0');
  await expect(page.locator('#tool-output')).toHaveText(PERFECT_JSON, { timeout: 15_000 });
});

test('roc-auc-calculator deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    data: '0.9,1\n0.8,1\n0.2,0\n0.1,0',
    format: 'json',
    plot: 'false',
    table_rows: '0',
    decimals: '4',
  });

  await page.goto(`/tools/roc-auc-calculator/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(params.get('data')!, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText(PERFECT_JSON, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool roc-auc-calculator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
