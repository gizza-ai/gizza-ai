import { test, expect } from './fixtures';

const ENTRIES = '2026-08-03 | Landing page copy | 3.5\n2026-08-04 | Bug fixes | 2h 30m\n2026-08-05 | Client call | 09:00-10:15';

async function runWasm(
  page: any,
  entries: string = ENTRIES,
  rate = '120',
  currency = '$',
  business = 'Ada Consulting',
  client = 'Globex Ltd',
  invoiceNumber = 'INV-001',
  issueDate = '2026-08-14',
  dueDate = '',
  paymentTerms = '30',
  taxLabel = 'Tax',
  taxRate = '0',
  discountPercent = '0',
  round = '0',
  groupBy = 'entry',
  notes = '',
  format = 'markdown',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/timesheet-to-invoice/gizza_ai_timesheet_to_invoice_web.js');
    await mod.default('/tools/timesheet-to-invoice/gizza_ai_timesheet_to_invoice_web_bg.wasm');
    return mod.run(
      args.entries,
      args.rate,
      args.currency,
      args.business,
      args.client,
      args.invoiceNumber,
      args.issueDate,
      args.dueDate,
      args.paymentTerms,
      args.taxLabel,
      args.taxRate,
      args.discountPercent,
      args.round,
      args.groupBy,
      args.notes,
      args.format,
    );
  }, { entries, rate, currency, business, client, invoiceNumber, issueDate, dueDate, paymentTerms, taxLabel, taxRate, discountPercent, round, groupBy, notes, format });
}

test('timesheet-to-invoice page computes a real invoice from the form', async ({ page }) => {
  await page.goto('/tools/timesheet-to-invoice/');
  await page.fill('#in-entries', ENTRIES);
  await page.fill('#in-rate', '120');
  await page.fill('#in-business', 'Ada Consulting');
  await page.fill('#in-client', 'Globex Ltd');
  await page.fill('#in-issue_date', '2026-08-14');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('# Invoice INV-001', { timeout: 15_000 });
  await expect(out).toContainText('Total hours:** 7.25');
  await expect(out).toContainText('Subtotal:** $870.00');
  await expect(out).toContainText('Due date:** 2026-09-13');
});

test('timesheet-to-invoice deep link covers CSV, terms, tax, discount, rounding and grouping', async ({ page }) => {
  const params = new URLSearchParams({
    entries: '2026-08-03 | Draft spec | 1.5\n2026-08-03 | Review | 2\n2026-08-04 | Ship release | 3h 15m',
    rate: '95',
    currency: '$',
    business: 'Ada Consulting',
    client: 'Globex Ltd',
    invoice_number: 'INV-002',
    issue_date: '2026-08-14',
    payment_terms: '7',
    tax_label: 'Sales tax',
    tax_rate: '8.5',
    discount_percent: '0',
    round: '6',
    group_by: 'date',
    format: 'csv',
  });
  await page.goto(`/tools/timesheet-to-invoice/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('csv', { timeout: 15_000 });
  await expect(page.locator('#in-group_by')).toHaveValue('date');
  await expect(page.locator('#tool-output')).toContainText('date,description,hours,rate,amount', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('2026-08-03,Draft spec; Review,3.50,95.00,332.50');
  await expect(page.locator('#tool-output')).toContainText('Sales tax (8.5%)');
});

test('timesheet-to-invoice wasm covers enum values, caps, per-row rates and CLI example', async ({ page }) => {
  await page.goto('/tools/timesheet-to-invoice/');

  const markdown = await runWasm(page);
  expect(markdown).toContain('**Total due: $870.00**');

  const text = await runWasm(page, ENTRIES, '120', '$', '', '', 'INV-001', '2026-08-14', '', '30', 'Tax', '0', '0', '0', 'entry', '', 'text');
  expect(text).toContain('INVOICE INV-001');
  expect(text).toContain('TOTAL DUE');

  const csv = await runWasm(page, 'Design | 0.4 | 90\nDesign | 0.3 | 90', '120', '£', '', '', '2026-014', '2026-08-14', '2026-08-28', '14', 'VAT', '20', '10', '15', 'description', 'VAT reverse charge does not apply.', 'csv');
  expect(csv).toContain('Design,0.75,90.00,67.50');
  expect(csv).toContain('Discount (10%)');
  expect(csv).toContain('VAT (20%)');
  expect(csv).toContain('Total due,,,72.90');

  const json = await runWasm(page, '2026-08-03 | Draft spec | 1.5\n2026-08-03 | Review | 2', '95', '$', '', '', 'INV-002', '2026-08-14', '', '7', 'Tax', '0', '0', '6', 'date', '', 'json');
  expect(json).toContain('"total_hours": 3.5');
  expect(json).toContain('"description": "Draft spec; Review"');

  await expect(runWasm(page, '', '120')).rejects.toThrow(/no entry lines found/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool timesheet-to-invoice');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
