import { test, expect } from './fixtures';

async function setBigTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('exponential-smoother renders the default alpha JSON report', async ({ page }) => {
  await page.goto('/tools/exponential-smoother/');
  await setBigTextarea(page, '#in-series', '12\n14\n13\n17\n16\n20\n19\n24');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"alpha": 0.3', { timeout: 15000 });
  await expect(out).toContainText('"level": 19.5755');
  await expect(out).toContainText('13.1765');
  await expect(out).toContainText('"rmse": 3.62333');
});

test('exponential-smoother deep link applies span mode and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/exponential-smoother/?series=1%2Cna%2C3&mode=alpha&alpha=0.5&span=5&halflife=3&com=2&adjust=false&ignore_na=true&min_periods=0&forecast=2&output=csv');
  await expect(page.locator('#in-mode')).toHaveValue('alpha', { timeout: 15000 });
  await expect(page.locator('#in-alpha')).toHaveValue('0.5');
  await expect(page.locator('#in-adjust')).not.toBeChecked();
  await expect(page.locator('#in-ignore_na')).toBeChecked();
  await expect(page.locator('#in-forecast')).toHaveValue('2');
  await expect(page.locator('#in-output')).toHaveValue('csv');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('index,value,smoothed,error', { timeout: 15000 });
  await expect(out).toContainText('1,1,1,');
  await expect(out).toContainText('2,,1,');
  await expect(out).toContainText('3,3,2,2');
  await expect(out).toContainText('4,,2,');
  await expect(out).toContainText('5,,2,');
});

test('exponential-smoother can output an SVG chart', async ({ page }) => {
  await page.goto('/tools/exponential-smoother/');
  await page.selectOption('#in-output', 'svg');
  await setBigTextarea(page, '#in-series', '10, 12, 13, 15, 16, 18');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg xmlns="http://www.w3.org/2000/svg"', { timeout: 15000 });
  await expect(out).toContainText('Exponentially smoothed series');
});
