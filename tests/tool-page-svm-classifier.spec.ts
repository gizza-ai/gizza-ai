import { test, expect } from './fixtures';

const tool = '/tools/svm-classifier/';
const sample = `x,y,label
1,1,a
2,1,a
1,2,a
8,8,b
9,8,b
8,9,b`;

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
    data: sample,
    target: 'label',
    features: 'x,y',
    kernel: 'linear',
    c: '1',
    gamma: 'scale',
    degree: '3',
    coef0: '0',
    scaling: 'standard',
    class_weight: 'none',
    multiclass: 'ovo',
    tol: '0.001',
    max_iter: '100000',
    cv_folds: '0',
    test_split: '0',
    seed: '42',
    predict: '1,1\n9,9',
    header: 'auto',
    decimals: '4',
    format: 'text',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/svm-classifier/gizza_ai_svm_classifier_web.js');
    await mod.default('/tools/svm-classifier/gizza_ai_svm_classifier_web_bg.wasm');
    return mod.run(
      args.data,
      args.target,
      args.features,
      args.kernel,
      args.c,
      args.gamma,
      args.degree,
      args.coef0,
      args.scaling,
      args.class_weight,
      args.multiclass,
      args.tol,
      args.max_iter,
      args.cv_folds,
      args.test_split,
      args.seed,
      args.predict,
      args.header,
      args.decimals,
      args.format,
    );
  }, p);
}

test('svm-classifier page renders a linear margin and predictions', async ({ page }) => {
  await page.goto(tool);
  await setField(page, '#in-data', sample);
  await setField(page, '#in-target', 'label');
  await setField(page, '#in-features', 'x,y');
  await page.selectOption('#in-kernel', 'linear');
  await setField(page, '#in-predict', '1,1\n9,9');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Support vector machine — linear kernel', { timeout: 20_000 });
  await expect(output).toContainText('Training:         6 / 6 = 100.00%');
  await expect(output).toContainText('Margin width:');
  await expect(output).toContainText('Support vectors:  3 of 6 training rows');
  await expect(output).toContainText('1  a');
  await expect(output).toContainText('2  b');
});

test('svm-classifier deep link can prefill JSON RBF output', async ({ page }) => {
  const qs = new URLSearchParams({
    data: sample,
    target: 'label',
    features: 'x,y',
    kernel: 'rbf',
    c: '1',
    gamma: 'auto',
    degree: '3',
    coef0: '0',
    scaling: 'standard',
    class_weight: 'none',
    multiclass: 'ovo',
    tol: '0.001',
    max_iter: '100000',
    cv_folds: '0',
    test_split: '0',
    seed: '42',
    predict: '',
    header: 'auto',
    decimals: '4',
    format: 'json',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(sample, { timeout: 15_000 });
  await expect(page.locator('#in-kernel')).toHaveValue('rbf');
  await expect(page.locator('#in-gamma')).toHaveValue('auto');
  await expect(page.locator('#in-format')).toHaveValue('json');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"kernel": "RBF"', { timeout: 20_000 });
  await expect(output).toContainText('"train_accuracy": 1.0000');
});

test('svm-classifier wasm covers kernels, enums, non-default options, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  const csv = await runWasm(page, { format: 'csv', kernel: 'poly', degree: '2', coef0: '1' });
  expect(csv).toContain('model,kernel,polynomial');
  expect(csv).toContain('model,degree,2');

  const categorical = await runWasm(page, {
    data: 'color,size,label\nred,small,yes\nred,large,yes\ngreen,small,no\ngreen,large,no',
    features: 'color,size',
    kernel: 'rbf',
    gamma: 'auto',
    class_weight: 'balanced',
    scaling: 'minmax',
    predict: 'red,small\ngreen,large',
    format: 'text',
  });
  expect(categorical).toContain('0 numeric, 4 one-hot');
  expect(categorical).toContain('Scaling:          minmax');
  expect(categorical).toContain('Predictions');

  const json = JSON.parse(await runWasm(page, { format: 'json', decimals: '12', test_split: '0.5', seed: '9' }));
  expect(json.kernel).toBe('linear');
  expect(json.train_accuracy).toBeGreaterThan(0.9);
  expect(json.n_support_vectors).toBeGreaterThan(0);

  await expect(runWasm(page, { data: 'x,label\n1,a\n2,a\n3,a', features: 'x' })).rejects.toThrow(/only one class/);
});
