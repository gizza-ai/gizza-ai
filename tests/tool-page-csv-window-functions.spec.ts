import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('csv-window-functions page — running total partitioned and ordered', async ({ page }) => {
  await page.goto('/tools/csv-window-functions/');
  await page.fill('#in-data', 'region,day,sales\nW,2,5\nE,1,10\nW,1,3\nE,2,20');
  await page.selectOption('#in-function', 'running_total');
  await page.fill('#in-column', 'sales');
  await page.fill('#in-partition_by', 'region');
  await page.fill('#in-order_by', 'day');
  await page.fill('#in-output_column', '');
  await page.check('#in-has_header');
  await page.selectOption('#in-delimiter', ',');
  await expect(page.locator('#tool-output')).toContainText('running_total_sales', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    'region,day,sales,running_total_sales\nW,1,3,3\nW,2,5,8\nE,1,10,10\nE,2,20,30',
  );
});

test('csv-window-functions page — rank descending with ties (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/csv-window-functions/');
  await page.fill('#in-data', 'name,score\nA,90\nB,90\nC,80\nD,70');
  await page.selectOption('#in-function', 'rank');
  await page.fill('#in-column', 'score');
  await page.check('#in-descending');
  await expect(page.locator('#in-descending')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('name,score,rank', { timeout: 15000 });
  expect(await outputText(page)).toBe('name,score,rank\nA,90,1\nB,90,1\nC,80,3\nD,70,4');
});

test('csv-window-functions page — deep-link 3-row moving average', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'day,sales\n1,10\n2,20\n3,30\n4,40',
    function: 'moving_average',
    column: 'sales',
    window: '3',
    output_column: 'avg3',
    has_header: 'true',
    delimiter: ',',
  });
  await page.goto('/tools/csv-window-functions/?' + params.toString());
  await expect(page.locator('#tool-output')).toContainText('day,sales,avg3', { timeout: 15000 });
  // trailing 3-row mean: [10]->10, [10,20]->15, [10,20,30]->20, [20,30,40]->30
  expect(await outputText(page)).toBe('day,sales,avg3\n1,10,10\n2,20,15\n3,30,20\n4,40,30');
});
