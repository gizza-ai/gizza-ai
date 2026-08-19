import { test, expect } from './fixtures';

const tool = '/tools/stepwise-feature-selection/';
const data = '1,5,5.19\n2,3,7.61\n3,8,11.29\n4,1,13.85\n5,9,16.95\n6,2,20.4\n7,7,22.51\n8,4,26.33\n9,6,28.73\n10,10,32.14';
const marketing = 'ads,price,temp,sales\n2,19,21,30.4\n5,15,14,49.0\n3,18,30,37.0\n8,12,18,67.1\n6,14,25,56.4\n9,11,11,75.1\n4,17,27,40.4\n7,13,16,62.7\n10,10,23,79.7\n5,16,29,48.4\n8,12,13,67.8\n3,20,19,31.9';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').trim();
}

test('stepwise-feature-selection page selects the real predictor with exact report fragments', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', data);
  await page.fill('#in-labels', 'x1,x2,y');
  await page.fill('#in-target', 'y');
  await page.selectOption('#in-direction', 'forward');
  await page.selectOption('#in-criterion', 'aic');

  await expect(page.locator('#tool-output')).toContainText('forward selection, AIC', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('+ x1');
  await expect(page.locator('#tool-output')).toContainText('Selected model (1 of 2 predictors)');
  await expect(page.locator('#tool-output')).toContainText('Dropped (1): x2');
});

test('stepwise-feature-selection page supports headers, backward direction and BIC', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', marketing);
  await page.fill('#in-target', 'sales');
  await page.check('#in-header');
  await page.selectOption('#in-direction', 'backward');
  await page.selectOption('#in-criterion', 'bic');
  await page.fill('#in-decimals', '3');

  await expect(page.locator('#tool-output')).toContainText('backward elimination, BIC', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('target: sales');
  await expect(page.locator('#tool-output')).toContainText('- temp');
  await expect(page.locator('#tool-output')).toContainText('Selected model (2 of 3 predictors)');
});

test('stepwise-feature-selection page supports p-value thresholds and forced predictors', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', data);
  await page.fill('#in-labels', 'x1,x2,y');
  await page.fill('#in-force', 'x2');
  await page.selectOption('#in-criterion', 'pvalue');
  await page.selectOption('#in-direction', 'both');
  await page.fill('#in-alpha_enter', '0.20');
  await page.fill('#in-alpha_remove', '0.30');

  await expect(page.locator('#tool-output')).toContainText('bidirectional stepwise, p-value', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Forced in: x2');
  await expect(page.locator('#tool-output')).toContainText('Dropped (0): none');
});

test('stepwise-feature-selection query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    tool +
      '?data=' +
      encodeURIComponent(data) +
      '&labels=' +
      encodeURIComponent('x1,x2,y') +
      '&target=y&direction=forward&criterion=bic&decimals=2',
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#in-labels')).toHaveValue('x1,x2,y');
  await expect(page.locator('#in-direction')).toHaveValue('forward');
  await expect(page.locator('#in-criterion')).toHaveValue('bic');
  expect(await outputText(page)).toContain('forward selection, BIC');
  expect(await outputText(page)).toContain('Dropped (1): x2');
});
