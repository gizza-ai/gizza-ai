import { test, expect } from './fixtures';

// A trailing newline from the CSV writer is trimmed before comparing so the
// assertions pin the exact row/column structure a user would see.
async function outputText(page): Promise<string> {
  const t = await page.locator('#tool-output').textContent();
  return (t ?? '').replace(/\s+$/, '');
}

const QIF = [
  '!Type:Bank',
  'D03/15/2010',
  'T-50.00',
  'PTarget Store',
  'MWeekly run',
  'LFood:Groceries',
  'N1002',
  '^',
].join('\n');

const OFX =
  '<OFX><STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20231026<TRNAMT>-75.50<FITID>abc123<NAME>Corner Grocery<MEMO>food & drink</STMTTRN></OFX>';

test('qif-ofx-convert page: QIF → normalized CSV, empty columns dropped (ISO dates)', async ({
  page,
}) => {
  await page.goto('/tools/qif-ofx-convert/');
  await page.fill('#in-data', QIF);
  await page.selectOption('#in-format', 'qif');
  await page.check('#in-drop_empty_columns');
  // Wait for a first render, then assert the exact CSV (Type/FITID dropped).
  await expect(page.locator('#tool-output')).toContainText('Target Store', {
    timeout: 15000,
  });
  expect(await outputText(page)).toBe(
    'Date,Amount,Payee,Memo,Category,Check Number\n' +
      '2010-03-15,-50.00,Target Store,Weekly run,Food:Groceries,1002',
  );
});

test('qif-ofx-convert page: full fixed 8-column set when not dropping (boundary)', async ({
  page,
}) => {
  await page.goto('/tools/qif-ofx-convert/');
  await page.fill('#in-data', QIF);
  await page.selectOption('#in-format', 'qif');
  // drop_empty_columns default OFF → every column present.
  await expect(page.locator('#tool-output')).toContainText('Target Store', {
    timeout: 15000,
  });
  const out = await outputText(page);
  expect(out.split('\n')[0]).toBe(
    'Date,Amount,Payee,Memo,Category,Check Number,Type,FITID',
  );
});

test('qif-ofx-convert page: OFX SGML input, auto-detected, entities decoded', async ({
  page,
}) => {
  await page.goto('/tools/qif-ofx-convert/');
  await page.fill('#in-data', OFX);
  // format left on default "auto" — <OFX> marker selects the OFX parser.
  await page.check('#in-drop_empty_columns');
  await expect(page.locator('#tool-output')).toContainText('Corner Grocery', {
    timeout: 15000,
  });
  // This single txn has no CHECKNUM, so drop-empty removes the Check Number column.
  expect(await outputText(page)).toBe(
    'Date,Amount,Payee,Memo,Type,FITID\n' +
      '2023-10-26,-75.50,Corner Grocery,food & drink,DEBIT,abc123',
  );
});

test('qif-ofx-convert page: US and EU date formats reorder month/day', async ({
  page,
}) => {
  await page.goto('/tools/qif-ofx-convert/');
  await page.fill('#in-data', QIF);
  await page.selectOption('#in-format', 'qif');
  await page.check('#in-drop_empty_columns');
  await page.selectOption('#in-date_format', 'us');
  await expect(page.locator('#tool-output')).toContainText('03/15/2010', {
    timeout: 15000,
  });
  await page.selectOption('#in-date_format', 'eu');
  await expect(page.locator('#tool-output')).toContainText('15/03/2010', {
    timeout: 15000,
  });
});

test('qif-ofx-convert page: semicolon delimiter', async ({ page }) => {
  await page.goto('/tools/qif-ofx-convert/');
  await page.fill('#in-data', QIF);
  await page.selectOption('#in-format', 'qif');
  await page.check('#in-drop_empty_columns');
  await page.selectOption('#in-delimiter', 'semicolon');
  await expect(page.locator('#tool-output')).toContainText(
    'Date;Amount;Payee',
    { timeout: 15000 },
  );
});

test('qif-ofx-convert page: invert amounts checkbox flips the sign (non-default)', async ({
  page,
}) => {
  await page.goto('/tools/qif-ofx-convert/');
  await page.fill('#in-data', QIF);
  await page.selectOption('#in-format', 'qif');
  await page.check('#in-drop_empty_columns');
  await page.check('#in-invert_amounts');
  // -50.00 debit becomes 50.00.
  await expect(page.locator('#tool-output')).toContainText(
    '2010-03-15,50.00,Target Store',
    { timeout: 15000 },
  );
});

test('qif-ofx-convert page: query-param deep-link prefills + computes', async ({
  page,
}) => {
  const qif = '!Type:Bank\nD01/02/2020\nT-9.99\nPCoffee\n^';
  await page.goto(
    '/tools/qif-ofx-convert/?data=' +
      encodeURIComponent(qif) +
      '&format=qif&drop_empty_columns=true',
  );
  await expect(page.locator('#in-data')).toHaveValue(qif, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('qif');
  await expect(page.locator('#tool-output')).toContainText(
    '2020-01-02,-9.99,Coffee',
    { timeout: 15000 },
  );
});
