import { test, expect } from './fixtures';

const DATA = '1,2.1\n2,3.9\n3,6.2\n4,7.8';

const INTERPOLATE_JSON = `{
  "mode": "smoothing",
  "criterion": "gcv",
  "lambda": 0,
  "smoothing": 1,
  "effective_df": 4,
  "n_points": 4,
  "n_input": 4,
  "merged_duplicates": 0,
  "x_min": 1,
  "x_max": 4,
  "rss": 0,
  "rmse": 0,
  "roughness": 1.464,
  "penalized_criterion": 0,
  "gcv": null,
  "cv": null,
  "points": [
    {"x": 1, "y": 2.1, "fitted": 2.1, "residual": 0, "weight": 1, "leverage": 1},
    {"x": 2, "y": 3.9, "fitted": 3.9, "residual": 0, "weight": 1, "leverage": 1},
    {"x": 3, "y": 6.2, "fitted": 6.2, "residual": 0, "weight": 1, "leverage": 1},
    {"x": 4, "y": 7.8, "fitted": 7.8, "residual": 0, "weight": 1, "leverage": 1}
  ]
}`;

const CSV_WITH_COEFFICIENTS = `metric,value
mode,smoothing
criterion,gcv
lambda,3
smoothing,0.9
effective_df,2.13834
n_points,4
n_input,4
merged_duplicates,0
rss,0.0773684
rmse,0.139076
roughness,0.000743923
penalized_criterion,0.0796002
gcv,0.0892946
cv,0.0754173

x,y,fitted,residual,weight,leverage
1,2.1,2.08489,0.015114,1,0.731307
2,3.9,4.03315,-0.133147,1,0.337865
3,6.2,5.97905,0.220952,1,0.337865
4,7.8,7.90292,-0.102919,1,0.731307

x_start,x_end,a,b,c,d
1,2,2.08489,1.94742,0,0.000839664
2,3,4.03315,1.94994,0.00251899,-0.00655738
3,4,5.97905,1.93531,-0.0171531,0.00571771`;

test('spline-smoother interpolates exactly when smoothing is 1', async ({ page }) => {
  await page.goto('/tools/spline-smoother/');
  await page.fill('#in-input', DATA);
  await page.selectOption('#in-mode', 'smoothing');
  await page.fill('#in-smoothing', '1');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toHaveText(INTERPOLATE_JSON, { timeout: 15000 });
});

test('spline-smoother deep-link can output CSV with coefficients', async ({ page }) => {
  const params = new URLSearchParams({
    input: DATA,
    mode: 'smoothing',
    smoothing: '0.9',
    lambda: '1',
    df: '5',
    criterion: 'gcv',
    weights: '',
    predict_at: '',
    resample: '0',
    coefficients: 'true',
    output: 'csv',
  });

  await page.goto(`/tools/spline-smoother/?${params.toString()}`);
  await expect(page.locator('#in-mode')).toHaveValue('smoothing');
  await expect(page.locator('#in-coefficients')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toHaveText(CSV_WITH_COEFFICIENTS, { timeout: 15000 });
});

test('spline-smoother renders an SVG chart', async ({ page }) => {
  await page.goto('/tools/spline-smoother/');
  await page.fill('#in-input', DATA);
  await page.selectOption('#in-mode', 'df');
  await page.fill('#in-df', '3');
  await page.selectOption('#in-output', 'svg');

  await expect(page.locator('#tool-output')).toContainText('<svg xmlns="http://www.w3.org/2000/svg"');
  await expect(page.locator('#tool-output')).toContainText('</svg>');
});
