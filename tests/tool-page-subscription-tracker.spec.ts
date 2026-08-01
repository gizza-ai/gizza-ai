import { test, expect } from './fixtures';

const SAMPLE = [
  'Netflix: 15.99 monthly',
  'Spotify: 10.99',
  'Amazon Prime: 139 yearly',
  'Adobe: 59.99 quarterly',
].join('\n');

const SAMPLE_OUTPUT =
  'Subscription spend — 4 active plans\n\n' +
  'Subscription  Billed      Monthly   Annual  Share\n' +
  'Adobe         $59.99/qtr   $20.00  $239.96  34.1%\n' +
  'Netflix       $15.99/mo    $15.99  $191.88  27.3%\n' +
  'Amazon Prime  $139.00/yr   $11.58  $139.00  19.8%\n' +
  'Spotify       $10.99/mo    $10.99  $131.88  18.8%\n\n' +
  'Monthly total:  $58.56\n' +
  'Annual total:   $702.72\n' +
  '5-year total:   $3,513.60\n' +
  'Per day:        $1.93\n\n' +
  'Biggest spend: Adobe at $239.96/yr ($20.00/mo). Cancelling it saves $239.96/year.';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').replace(/\s+$/, '');
}

test('subscription-tracker page totals subscriptions with exact output', async ({ page }) => {
  await page.goto('/tools/subscription-tracker/');
  await page.fill('#in-subscriptions', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Subscription spend — 4 active plans', { timeout: 15000 });
  expect(await outputText(page)).toBe(SAMPLE_OUTPUT);
});

test('subscription-tracker page honours currency, sort and weekly default', async ({ page }) => {
  await page.goto('/tools/subscription-tracker/');
  await page.fill('#in-subscriptions', ['Yoga: 12', 'Zine: 40 yearly'].join('\n'));
  await page.selectOption('#in-default_cycle', 'weekly');
  await page.fill('#in-currency', '£');
  await page.selectOption('#in-sort', 'name');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Subscription spend — 2 active plans', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('Yoga          £12.00/wk   £52.00  £624.00  94.0%');
  expect(text).toContain('Zine          £40.00/yr    £3.33   £40.00   6.0%');
  expect(text).toContain('Annual total:   £664.00');
});

test('subscription-tracker page deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/subscription-tracker/?subscriptions=' +
      encodeURIComponent(SAMPLE) +
      '&default_cycle=monthly&currency=%E2%82%AC&sort=input',
  );

  await expect(page.locator('#in-subscriptions')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-default_cycle')).toHaveValue('monthly');
  await expect(page.locator('#in-currency')).toHaveValue('€');
  await expect(page.locator('#in-sort')).toHaveValue('input');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Subscription spend — 4 active plans', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('Netflix       €15.99/mo');
  expect(text).toContain('Biggest spend: Adobe at €239.96/yr (€20.00/mo). Cancelling it saves €239.96/year.');
});
