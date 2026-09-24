import { test, expect } from './fixtures';

const MATRIX_A = '40,10\n5,45';
const MATRIX_B = '45,5\n8,42';

async function runWasm(
  page: any,
  matrixA = MATRIX_A,
  matrixB = MATRIX_B,
  labels = '',
  nameA = 'Model A',
  nameB = 'Model B',
  inputFormat = 'auto',
  orientation = 'actual_rows',
  separator = 'auto',
  header = 'auto',
  beta = '1',
  sortBy = 'class',
  significance = 'false',
  confidenceLevel = '95',
  matrixDelta = 'false',
  decimals = '4',
  percent = 'false',
  format = 'json',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/confusion-matrix-comparator/gizza_ai_confusion_matrix_comparator_web.js');
    await mod.default('/tools/confusion-matrix-comparator/gizza_ai_confusion_matrix_comparator_web_bg.wasm');
    return mod.run(
      args.matrixA,
      args.matrixB,
      args.labels,
      args.nameA,
      args.nameB,
      args.inputFormat,
      args.orientation,
      args.separator,
      args.header,
      args.beta,
      args.sortBy,
      args.significance,
      args.confidenceLevel,
      args.matrixDelta,
      args.decimals,
      args.percent,
      args.format,
    );
  }, { matrixA, matrixB, labels, nameA, nameB, inputFormat, orientation, separator, header, beta, sortBy, significance, confidenceLevel, matrixDelta, decimals, percent, format });
}

const BASE_JSON = `{
  "name_a": "Model A",
  "name_b": "Model B",
  "classes": ["0", "1"],
  "beta": 1.0000,
  "observations_a": 100,
  "observations_b": 100,
  "correct_a": 85,
  "correct_b": 87,
  "verdict": "Model B is ahead — macro F1 +0.0203, accuracy +0.0200",
  "overall":
  {
    "accuracy": { "a": 0.8500, "b": 0.8700, "delta": 0.0200 },
    "balanced_accuracy": { "a": 0.8500, "b": 0.8700, "delta": 0.0200 },
    "macro_precision": { "a": 0.8535, "b": 0.8713, "delta": 0.0178 },
    "macro_recall": { "a": 0.8500, "b": 0.8700, "delta": 0.0200 },
    "macro_fscore": { "a": 0.8496, "b": 0.8699, "delta": 0.0203 },
    "weighted_precision": { "a": 0.8535, "b": 0.8713, "delta": 0.0178 },
    "weighted_recall": { "a": 0.8500, "b": 0.8700, "delta": 0.0200 },
    "weighted_fscore": { "a": 0.8496, "b": 0.8699, "delta": 0.0203 },
    "micro_fscore": { "a": 0.8500, "b": 0.8700, "delta": 0.0200 },
    "cohens_kappa": { "a": 0.7000, "b": 0.7400, "delta": 0.0400 },
    "matthews_correlation": { "a": 0.7035, "b": 0.7413, "delta": 0.0378 }
  },
  "per_class": [
    { "class": "0", "support_a": 50, "support_b": 50, "precision_a": 0.8889, "precision_b": 0.8491, "precision_delta": -0.0398, "recall_a": 0.8000, "recall_b": 0.9000, "recall_delta": 0.1000, "fscore_a": 0.8421, "fscore_b": 0.8738, "fscore_delta": 0.0317 },
    { "class": "1", "support_a": 50, "support_b": 50, "precision_a": 0.8182, "precision_b": 0.8936, "precision_delta": 0.0754, "recall_a": 0.9000, "recall_b": 0.8400, "recall_delta": -0.0600, "fscore_a": 0.8571, "fscore_b": 0.8660, "fscore_delta": 0.0088 }
  ],
  "binary":
  {
    "positive_class": "1",
    "precision": { "a": 0.8182, "b": 0.8936, "delta": 0.0754 },
    "recall": { "a": 0.9000, "b": 0.8400, "delta": -0.0600 },
    "specificity": { "a": 0.8000, "b": 0.9000, "delta": 0.1000 },
    "fscore": { "a": 0.8571, "b": 0.8660, "delta": 0.0088 }
    ,"counts": { "tp_a": 45, "tp_b": 42, "fp_a": 10, "fp_b": 5, "fn_a": 5, "fn_b": 8, "tn_a": 40, "tn_b": 45 }
  },
  "accuracy_test": null,
  "notes": []
}`;

test('confusion-matrix-comparator wasm computes baseline JSON exactly', async ({ page }) => {
  await page.goto('/tools/confusion-matrix-comparator/');
  await page.waitForSelector('#in-matrix_a');

  await expect(runWasm(page)).resolves.toBe(BASE_JSON);
});

test('confusion-matrix-comparator wasm covers label-pair input, F2, percent output and hidden delta grid', async ({ page }) => {
  await page.goto('/tools/confusion-matrix-comparator/');
  await page.waitForSelector('#in-matrix_a');

  const out = await runWasm(
    page,
    'cat,cat\ncat,dog\ndog,dog\ndog,dog\nfox,fox\nfox,cat',
    'cat,cat\ncat,cat\ndog,dog\ndog,fox\nfox,fox\nfox,fox',
    '',
    'v1',
    'v2',
    'labels',
    'actual_rows',
    'comma',
    'no',
    '2',
    'regression',
    'false',
    '99',
    'false',
    '2',
    'true',
    'json',
  );
  expect(out).toContain('"beta": 2.00');
  expect(out).toContain('"name_a": "v1"');
  expect(out).toContain('"name_b": "v2"');
  expect(out).toContain('"classes": ["cat", "dog", "fox"]');
  expect(out).not.toContain('"delta_matrix"');
});

test('confusion-matrix-comparator page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/confusion-matrix-comparator/');
  await page.locator('#in-matrix_a').fill(MATRIX_A);
  await page.locator('#in-matrix_b').fill(MATRIX_B);
  await page.locator('#in-significance').uncheck();
  await page.locator('#in-matrix_delta').uncheck();
  await page.locator('#in-format').selectOption('json');
  await expect(page.locator('#tool-output')).toHaveText(BASE_JSON, { timeout: 15_000 });
});

test('confusion-matrix-comparator deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    matrix_a: MATRIX_A,
    matrix_b: MATRIX_B,
    significance: 'false',
    matrix_delta: 'false',
    format: 'json',
  });

  await page.goto(`/tools/confusion-matrix-comparator/?${params.toString()}`);
  await expect(page.locator('#in-matrix_a')).toHaveValue(MATRIX_A, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText(BASE_JSON, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool confusion-matrix-comparator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
