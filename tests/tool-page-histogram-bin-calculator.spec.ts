import { test, expect } from './fixtures';

const sample = ['1', '2', '2', '3', '4', '7', '8', '9', '12', '15', '22', '40'].join('\n');

test('histogram-bin-calculator compares every rule and bins the data', async ({ page }) => {
  await page.goto('/tools/histogram-bin-calculator/');
  await page.fill('#in-numbers', sample);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Recommended histogram bins', { timeout: 20000 });
  await expect(output).toContainText('Values: 12');

  // Every rule is reported side by side — that is the tool's differentiator.
  for (const rule of ['Auto', 'Sturges', 'Scott', 'Freedman–Diaconis', 'Rice', 'Square root']) {
    await expect(output).toContainText(rule);
  }
  await expect(output).toContainText('Using Auto:');
  await expect(output).toContainText('left-closed [a, b)');
  await expect(output).toContainText('Counted 12 of 12 values.');
  // ASCII bars are on by default.
  await expect(output).toContainText('#');
});

test('histogram-bin-calculator deep link emits CSV for manual bins over a fixed range', async ({
  page,
}) => {
  const numbers = [
    '3', '11', '18', '25', '27', '31', '38', '44', '49', '52',
    '55', '61', '66', '70', '74', '79', '83', '88', '92', '97',
  ].join('\n');
  const qs =
    '?numbers=' + encodeURIComponent(numbers) +
    '&rule=manual' +
    '&bins=4' +
    '&range_min=0' +
    '&range_max=100' +
    '&precision=2' +
    '&output=csv' +
    '&cumulative=true' +
    '&density=true';

  await page.goto('/tools/histogram-bin-calculator/' + qs);
  await expect(page.locator('#in-rule')).toHaveValue('manual', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#in-bins')).toHaveValue('4');

  const output = page.locator('#tool-output');
  await expect(output).toContainText(
    'bin,lower,upper,label,count,percent,cumulative_count,cumulative_percent,density',
    { timeout: 20000 },
  );
  // 0-100 split into 4 bins of 25, left-closed; values exactly on 25 land in bin 2.
  await expect(output).toContainText('1,0,25,"[0, 25)",3,15%,3,15%,0.006');
  await expect(output).toContainText('4,75,100,"[75, 100]",5,25%,20,100%,0.01');
});

test('histogram-bin-calculator honours right-closed intervals and nice edges', async ({ page }) => {
  const qs =
    '?numbers=' + encodeURIComponent(sample) +
    '&rule=sturges' +
    '&nice_edges=true' +
    '&right_closed=true' +
    '&output=report';

  await page.goto('/tools/histogram-bin-calculator/' + qs);
  await expect(page.locator('#in-rule')).toHaveValue('sturges', { timeout: 15000 });

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Using Sturges:', { timeout: 20000 });
  await expect(output).toContainText('right-closed (a, b]');
});
