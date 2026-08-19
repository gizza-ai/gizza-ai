import { test, expect } from './fixtures';

const lineData = `1,2
2,4
3,5
4,4
5,5`;

const lineOutput = `y = 0.6000·x + 2.2000

Model
  fit                 linear (degree 1)
  points              5
  R²                  0.6000
  adjusted R²         0.4667
  Pearson r           0.7746
  RMSE                0.6928
  residual std error  0.8944 on 3 DF

Coefficients
  term       estimate  std error
  intercept  2.2000  0.9381
  x          0.6000  0.2828

Residuals
  min -0.8000  median -0.2000  max 1.0000

Predictions
  x = 6.0000  ->  y = 5.8000
  x = 7.0000  ->  y = 6.4000
`;

test('least-squares-regression page emits exact linear fit output', async ({ page }) => {
  await page.goto('/tools/least-squares-regression/');
  await page.fill('#in-data', lineData);
  await page.fill('#in-predict_x', '6,7');

  await expect(page.locator('#tool-output')).toHaveText(lineOutput, { timeout: 15_000 });
});

test('least-squares-regression honours deep-link quadratic params', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'x,y\n0,1\n1,6\n2,17\n3,34\n4,57',
    degree: '2',
    header: 'auto',
    intercept: 'true',
    predict_x: '5',
    decimals: '3',
    format: 'text',
  });
  await page.goto(`/tools/least-squares-regression/?${params.toString()}`);

  await expect(page.locator('#in-degree')).toHaveValue('2', { timeout: 15_000 });
  await expect(page.locator('#in-decimals')).toHaveValue('3');
  await expect(page.locator('#tool-output')).toContainText('y = 3.000·x² + 2.000·x + 1.000');
  await expect(page.locator('#tool-output')).toContainText('x = 5.000  ->  y = 86.000');
});

test('least-squares-regression covers csv output and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/least-squares-regression/');
  await page.fill('#in-data', '1,2\n2,4\n3,6.5');
  await page.uncheck('#in-intercept');
  await page.fill('#in-decimals', '6');
  await page.selectOption('#in-format', 'csv');

  await expect(page.locator('#tool-output')).toContainText('x,2.107143,0.056469', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('df_residual,2');
});
