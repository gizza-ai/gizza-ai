import { test, expect } from './fixtures';

const LINE_DATA = `x,y
1,3
2,5
3,7
4,9`;

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('regression-model-trainer renders an exact linear fit', async ({ page }) => {
  await page.goto('/tools/regression-model-trainer/');
  await setTextarea(page, '#in-data', LINE_DATA);
  await page.fill('#in-target', 'y');
  await page.selectOption('#in-model', 'linear');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(`Linear regression (OLS)
Target: y
Predictors (1): x
Rows: 4 used

Training fit (n = 4):
  R²    1
  RMSE  0
  MAE   0

Coefficients:
  (Intercept)  1
  x            2

Equation: y = 1 + 2·x`, { timeout: 15_000 });
});

test('regression-model-trainer deep link covers ridge, column selectors, checkbox off, and max split', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'sqft,rooms,price\n1,1,6\n2,1,8\n3,2,11\n4,2,14\n5,3,18\n6,3,20\n7,4,23\n8,4,26\n9,5,29\n10,5,32\n11,6,35\n12,6,38',
    target: '3',
    features: '1,2',
    model: 'ridge',
    alpha: '2',
    standardize: 'false',
    test_split: '0',
    cv_folds: '10',
    decimals: '3',
    header: 'yes',
    format: 'json',
  });
  await page.goto(`/tools/regression-model-trainer/?${params.toString()}`);

  await expect(page.locator('#in-target')).toHaveValue('3', { timeout: 15_000 });
  await expect(page.locator('#in-features')).toHaveValue('1,2');
  await expect(page.locator('#in-model')).toHaveValue('ridge');
  await expect(page.locator('#in-alpha')).toHaveValue('2');
  await expect(page.locator('#in-standardize')).not.toBeChecked();
  await expect(page.locator('#in-test_split')).toHaveValue('0');
  await expect(page.locator('#in-cv_folds')).toHaveValue('10');
  await expect(page.locator('#in-header')).toHaveValue('yes');
  await expect(page.locator('#in-format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"model":"ridge"', { timeout: 15_000 });
  await expect(out).toContainText('"target":"price"');
  await expect(out).toContainText('"features":["sqft","rooms"]');
  await expect(out).toContainText('"alpha":2');
  await expect(out).toContainText('"standardize":false');
  await expect(out).toContainText('"cv"');
  await expect(out).toContainText('"coefficients"');
});

test('regression-model-trainer accepts the maximum held-out split', async ({ page }) => {
  const params = new URLSearchParams({
    data: LINE_DATA,
    target: 'y',
    model: 'linear',
    test_split: '0.5',
    format: 'json',
  });
  await page.goto(`/tools/regression-model-trainer/?${params.toString()}`);

  await expect(page.locator('#in-test_split')).toHaveValue('0.5', { timeout: 15_000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"model":"linear"', { timeout: 15_000 });
  await expect(out).toContainText('"test"');
});

test('regression-model-trainer covers random forest and CSV output', async ({ page }) => {
  let data = 'a,b,y\n';
  for (let i = 1; i <= 30; i += 1) {
    data += `${i},${i % 5},${3 * i + 1}\n`;
  }

  await page.goto('/tools/regression-model-trainer/');
  await setTextarea(page, '#in-data', data);
  await page.fill('#in-target', 'last');
  await page.selectOption('#in-model', 'random_forest');
  await page.fill('#in-trees', '20');
  await page.fill('#in-max_depth', '5');
  await page.fill('#in-seed', '7');
  await page.selectOption('#in-format', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('section,name,value', { timeout: 15_000 });
  await expect(out).toContainText('model,type,random_forest');
  await expect(out).toContainText('importance,a,');
  await expect(out).toContainText('importance,b,');
});

test('regression-model-trainer generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/regression-model-trainer/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool regression-model-trainer');
  expect(cli).toContain('x,y');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
