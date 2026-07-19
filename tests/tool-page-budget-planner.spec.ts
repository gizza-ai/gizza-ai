import { test, expect } from './fixtures';

// budget-planner page: pure text tool. Exact-output assertions use
// textContent (toHaveText normalizes whitespace and can't pin multi-line
// alignment). Every mode/value-form advertised on the page gets a real run.

const RULE_DEFAULT = `50/30/20 budget · take-home income $4,500.00/month

Bucket   Share     Target
Needs      50%  $2,250.00
Wants      30%  $1,350.00
Savings    20%    $900.00
`;

const RULE_60_30_10_DEEPLINK = `60/30/10 budget · take-home income $4,500.00/month

Bucket   Share     Target
Needs      60%  $2,700.00
Wants      30%  $1,350.00
Savings    10%    $450.00
`;

const RULE_WITH_EXPENSES = `60/30/10 budget · take-home income $5,200.00/month

Bucket   Share     Target    Planned       Left
Needs      60%  $3,120.00  $1,800.00  $1,320.00
Wants      30%  $1,560.00    $260.00  $1,300.00
Savings    10%    $520.00    $600.00    -$80.00  (over)

Planned $2,660.00 of $5,200.00 · left to allocate $2,540.00
`;

const ZERO_BASED = `Zero-based budget · take-home income $2,500.00/month

Category            Planned  Share
Rent              $1,400.00  56.0%
Groceries           $450.00  18.0%
Utilities           $180.00   7.2%
Fun money           $200.00   8.0%
Total planned     $2,230.00  89.2%
Left to allocate    $270.00  10.8%

Assign the remaining $270.00 — a zero-based budget gives every dollar a job.
`;

test('budget-planner default 50/30/20 exact report', async ({ page }) => {
  await page.goto('/tools/budget-planner/');
  await page.fill('#in-income', '4500');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('$2,250.00', { timeout: 15000 });
  expect(await out.textContent()).toBe(RULE_DEFAULT);
});

test('budget-planner custom split with $/comma amounts and tagged expenses', async ({ page }) => {
  await page.goto('/tools/budget-planner/');
  await page.fill('#in-income', '5200');
  await page.fill('#in-split', '60/30/10');
  // "$1,800" exercises the dollar-sign + thousands-separator amount form
  // end-to-end; "(over)" must appear on the over-target savings bucket.
  await page.fill(
    '#in-expenses',
    'Rent: $1,800 (needs)\nDining out: 260 (wants)\nBrokerage: 600 (savings)'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('left to allocate $2,540.00', { timeout: 15000 });
  expect(await out.textContent()).toBe(RULE_WITH_EXPENSES);
});

test('budget-planner zero-based mode exact report', async ({ page }) => {
  await page.goto('/tools/budget-planner/');
  await page.selectOption('#in-mode', 'zero-based'); // non-default enum choice
  await page.fill('#in-income', '2500');
  await page.fill('#in-expenses', 'Rent: 1400\nGroceries: 450\nUtilities: 180\nFun money: 200');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Left to allocate', { timeout: 15000 });
  expect(await out.textContent()).toBe(ZERO_BASED);
});

test('budget-planner zero-based deficit and balanced statuses', async ({ page }) => {
  await page.goto('/tools/budget-planner/');
  await page.selectOption('#in-mode', 'zero-based');
  await page.fill('#in-income', '1000');
  await page.fill('#in-expenses', 'Rent: 900\nFood: 300');
  const out = page.locator('#tool-output');
  await expect(out).toContainText(
    'Over budget by $200.00 — trim planned spending to get income minus expenses back to zero.',
    { timeout: 15000 }
  );
  // Same plan balances exactly at $1,200 income.
  await page.fill('#in-income', '1200');
  await expect(out).toContainText('Every dollar is assigned — your budget zeroes out.', {
    timeout: 15000,
  });
});

test('budget-planner custom currency symbol', async ({ page }) => {
  await page.goto('/tools/budget-planner/');
  await page.fill('#in-income', '4500');
  await page.fill('#in-currency', '€');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('€4,500.00/month', { timeout: 15000 });
  await expect(out).toContainText('€2,250.00');
});

test('budget-planner untagged expense in rule mode shows a naming error', async ({ page }) => {
  await page.goto('/tools/budget-planner/');
  await page.fill('#in-income', '4500');
  await page.fill('#in-expenses', 'Streaming: 30');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('has no bucket tag', { timeout: 15000 });
  await expect(out).toContainText('Streaming');
  await expect(out).toHaveClass(/error/);
});

test('budget-planner expense line cap: 100 ok, 101 rejected', async ({ page }) => {
  await page.goto('/tools/budget-planner/');
  await page.selectOption('#in-mode', 'zero-based');
  await page.fill('#in-income', '4500');
  const setExpenses = async (n: number) => {
    const lines = Array.from({ length: n }, (_, i) => `Cat${i}: 1`).join('\n');
    await page.locator('#in-expenses').evaluate((el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, lines);
  };
  await setExpenses(100); // exactly at the cap
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Total planned', { timeout: 15000 });
  await expect(out).toContainText('$100.00');
  await setExpenses(101); // one over
  await expect(out).toContainText('too many expense lines (max 100)', { timeout: 15000 });
});

test('budget-planner deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto('/tools/budget-planner/?income=4500&split=60%2F30%2F10');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('$2,700.00', { timeout: 15000 });
  expect(await out.textContent()).toBe(RULE_60_30_10_DEEPLINK);
});
