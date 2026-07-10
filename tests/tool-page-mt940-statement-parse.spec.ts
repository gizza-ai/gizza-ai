import { test, expect } from './fixtures';

const STATEMENT = [
  ':20:REF12345',
  ':25:NL91ABNA0417164300',
  ':28C:00123/001',
  ':60F:C240101EUR1000,00',
  ':61:2401020102D150,50NTRFNONREF//BANKREF1',
  ':86:Payment to Acme Corp invoice 42',
  ':61:2401030103C2000,00NTRFPAYROLL//BANKREF2',
  ':86:Salary March',
  ':62F:C240131EUR2849,50',
].join('\n');

const CSV =
  'Statement,Value Date,Entry Date,D/C,Amount,Currency,Transaction Type,Customer Reference,Bank Reference,Description\n' +
  '1,2024-01-02,2024-01-02,D,-150.50,EUR,NTRF,NONREF,BANKREF1,Payment to Acme Corp invoice 42\n' +
  '1,2024-01-03,2024-01-03,C,2000.00,EUR,NTRF,PAYROLL,BANKREF2,Salary March\n';

test('mt940-statement-parse page emits structured JSON with balances and transactions', async ({ page }) => {
  await page.goto('/tools/mt940-statement-parse/');
  await page.fill('#in-data', STATEMENT);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Payment to Acme Corp invoice 42', { timeout: 15000 });
  const parsed = JSON.parse((await out.textContent())!);
  expect(parsed[0].reference).toBe('REF12345');
  expect(parsed[0].account).toBe('NL91ABNA0417164300');
  expect(parsed[0].opening_balance.amount).toBe(1000);
  expect(parsed[0].closing_balance.amount).toBe(2849.5);
  expect(parsed[0].transactions[0].amount).toBe(-150.5);
  expect(parsed[0].transactions[1].customer_reference).toBe('PAYROLL');
});

test('mt940-statement-parse page renders exact CSV output', async ({ page }) => {
  await page.goto('/tools/mt940-statement-parse/');
  await page.fill('#in-data', STATEMENT);
  await page.selectOption('#in-output', 'csv');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Salary March', { timeout: 15000 });
  expect(await out.textContent()).toBe(CSV);
});

test('mt940-statement-parse page honors enum choices and non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/mt940-statement-parse/');
  await page.fill('#in-data', STATEMENT);
  await page.selectOption('#in-output', 'csv');
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.selectOption('#in-date_format', 'eu');
  await page.uncheck('#in-signed_amounts');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1;02/01/2024;02/01/2024;D;150.50;EUR', { timeout: 15000 });
  await expect(out).not.toContainText('-150.50');
});

test('mt940-statement-parse page reports malformed input clearly', async ({ page }) => {
  await page.goto('/tools/mt940-statement-parse/');
  await page.fill('#in-data', ':25:ONLYACCOUNT');
  await expect(page.locator('#tool-output')).toContainText("expected a ':20:' transaction-reference tag", { timeout: 15000 });
});

test('mt940-statement-parse page honors query-param deep link', async ({ page }) => {
  const data = encodeURIComponent(STATEMENT);
  await page.goto(`/tools/mt940-statement-parse/?data=${data}&output=csv&date_format=iso&delimiter=comma&signed_amounts=true`);
  await expect(page.locator('#in-data')).toHaveValue(STATEMENT);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Salary March', { timeout: 15000 });
  expect(await out.textContent()).toBe(CSV);
});

test('mt940-statement-parse page download link serves exactly the visible CSV', async ({ page }) => {
  await page.goto('/tools/mt940-statement-parse/');
  await page.fill('#in-data', STATEMENT);
  await page.selectOption('#in-output', 'csv');
  const out = page.locator('#tool-output');
  const dl = page.locator('#tool-output-download');
  await expect(out).toContainText('Salary March', { timeout: 15000 });
  await expect(dl).toBeVisible();
  expect(await dl.getAttribute('download')).toBe('mt940-statement-parse-output.txt');
  const blobText = await page.evaluate(async () => {
    const a = document.getElementById('tool-output-download') as HTMLAnchorElement;
    return (await fetch(a.href)).text();
  });
  expect(blobText).toBe(CSV);
});
