import { test, expect } from './fixtures';

const STATEMENT = [
  '<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02">',
  ' <BkToCstmrStmt>',
  '  <Stmt>',
  '   <Id>STMT-2024-001</Id>',
  '   <ElctrncSeqNb>1</ElctrncSeqNb>',
  '   <CreDtTm>2024-02-01T06:00:00</CreDtTm>',
  '   <Acct><Id><IBAN>NL91ABNA0417164300</IBAN></Id><Ccy>EUR</Ccy><Ownr><Nm>Acme BV</Nm></Ownr></Acct>',
  '   <Bal><Tp><CdOrPrtry><Cd>OPBD</Cd></CdOrPrtry></Tp><Amt Ccy="EUR">1000.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2024-01-01</Dt></Dt></Bal>',
  '   <Bal><Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp><Amt Ccy="EUR">2849.50</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2024-01-31</Dt></Dt></Bal>',
  '   <Ntry><Amt Ccy="EUR">150.50</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2024-01-02</Dt></BookgDt><ValDt><Dt>2024-01-02</Dt></ValDt><AcctSvcrRef>BANKREF1</AcctSvcrRef><BkTxCd><Domn><Cd>PMNT</Cd><Fmly><Cd>ICDT</Cd><SubFmlyCd>ESCT</SubFmlyCd></Fmly></Domn></BkTxCd><NtryDtls><TxDtls><Refs><EndToEndId>E2E-42</EndToEndId></Refs><RltdPties><Cdtr><Nm>Acme Corp</Nm></Cdtr><CdtrAcct><Id><IBAN>DE89370400440532013000</IBAN></Id></CdtrAcct></RltdPties><RmtInf><Ustrd>Payment invoice 42</Ustrd></RmtInf></TxDtls></NtryDtls></Ntry>',
  '   <Ntry><Amt Ccy="EUR">2000.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts>BOOK</Sts><BookgDt><Dt>2024-01-03</Dt></BookgDt><ValDt><Dt>2024-01-03</Dt></ValDt><AcctSvcrRef>BANKREF2</AcctSvcrRef><NtryDtls><TxDtls><Refs><EndToEndId>PAYROLL-MAR</EndToEndId></Refs><RltdPties><Dbtr><Nm>Payroll Ltd</Nm></Dbtr><DbtrAcct><Id><IBAN>GB29NWBK60161331926819</IBAN></Id></DbtrAcct></RltdPties><RmtInf><Ustrd>Salary March</Ustrd></RmtInf></TxDtls></NtryDtls></Ntry>',
  '  </Stmt>',
  ' </BkToCstmrStmt>',
  '</Document>',
].join('\n');

// A single Ntry rolling up two TxDtls payments (v8-style Sts/Cd + Pty/Nm nesting).
const BATCH = [
  '<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.08">',
  ' <BkToCstmrStmt>',
  '  <Stmt>',
  '   <Id>B1</Id>',
  '   <Acct><Id><IBAN>NL91ABNA0417164300</IBAN></Id><Ccy>EUR</Ccy></Acct>',
  '   <Ntry><Amt Ccy="EUR">300.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts><Cd>BOOK</Cd></Sts><BookgDt><Dt>2024-03-01</Dt></BookgDt><ValDt><Dt>2024-03-01</Dt></ValDt><NtryDtls><TxDtls><Refs><EndToEndId>E2E-A</EndToEndId></Refs><Amt Ccy="EUR">100.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><RltdPties><Cdtr><Pty><Nm>Alpha GmbH</Nm></Pty></Cdtr></RltdPties><RmtInf><Ustrd>Rent</Ustrd></RmtInf></TxDtls><TxDtls><Refs><EndToEndId>E2E-B</EndToEndId></Refs><Amt Ccy="EUR">200.00</Amt><CdtDbtInd>DBIT</CdtDbtInd><RltdPties><Cdtr><Pty><Nm>Beta SARL</Nm></Pty></Cdtr></RltdPties><RmtInf><Ustrd>Utilities</Ustrd></RmtInf></TxDtls></NtryDtls></Ntry>',
  '  </Stmt>',
  ' </BkToCstmrStmt>',
  '</Document>',
].join('\n');

const CSV =
  'Statement,Booking Date,Value Date,D/C,Amount,Currency,Status,Bank Transaction Code,End To End Id,Bank Reference,Counterparty,Counterparty IBAN,Description\n' +
  '1,2024-01-02,2024-01-02,DBIT,-150.50,EUR,BOOK,PMNT.ICDT.ESCT,E2E-42,BANKREF1,Acme Corp,DE89370400440532013000,Payment invoice 42\n' +
  '1,2024-01-03,2024-01-03,CRDT,2000.00,EUR,BOOK,,PAYROLL-MAR,BANKREF2,Payroll Ltd,GB29NWBK60161331926819,Salary March\n';

test('camt053-parse page emits structured JSON with balances and transactions', async ({ page }) => {
  await page.goto('/tools/camt053-parse/');
  await page.fill('#in-data', STATEMENT);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Payment invoice 42', { timeout: 15000 });
  const parsed = JSON.parse((await out.textContent())!);
  expect(parsed[0].message_type).toBe('camt.053');
  expect(parsed[0].statement_id).toBe('STMT-2024-001');
  expect(parsed[0].account_iban).toBe('NL91ABNA0417164300');
  expect(parsed[0].account_owner).toBe('Acme BV');
  expect(parsed[0].opening_balance.amount).toBe(1000);
  expect(parsed[0].opening_balance.description).toBe('Opening booked');
  expect(parsed[0].closing_balance.amount).toBe(2849.5);
  expect(parsed[0].transactions[0].amount).toBe(-150.5);
  expect(parsed[0].transactions[0].counterparty).toBe('Acme Corp');
  expect(parsed[0].transactions[0].bank_transaction_code).toBe('PMNT.ICDT.ESCT');
  expect(parsed[0].transactions[1].counterparty).toBe('Payroll Ltd');
  expect(parsed[0].transactions[1].end_to_end_id).toBe('PAYROLL-MAR');
});

test('camt053-parse page renders exact CSV output', async ({ page }) => {
  await page.goto('/tools/camt053-parse/');
  await page.fill('#in-data', STATEMENT);
  await page.selectOption('#in-output', 'csv');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Salary March', { timeout: 15000 });
  expect(await out.textContent()).toBe(CSV);
});

test('camt053-parse page honors enum choices and non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/camt053-parse/');
  await page.fill('#in-data', STATEMENT);
  await page.selectOption('#in-output', 'csv');
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.selectOption('#in-date_format', 'eu');
  await page.uncheck('#in-signed_amounts');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1;02/01/2024;02/01/2024;DBIT;150.50;EUR', { timeout: 15000 });
  await expect(out).not.toContainText('-150.50');
});

test('camt053-parse page expands batch entries and rolls up when unchecked', async ({ page }) => {
  await page.goto('/tools/camt053-parse/');
  await page.fill('#in-data', BATCH);
  await page.selectOption('#in-output', 'csv');
  const out = page.locator('#tool-output');
  // Expanded (default): one row per TxDtls payment, v8+ Pty/Nm counterparties.
  await expect(out).toContainText('Beta SARL', { timeout: 15000 });
  let text = (await out.textContent())!;
  expect(text).toContain('1,2024-03-01,2024-03-01,DBIT,-100.00,EUR,BOOK,,E2E-A,,Alpha GmbH,,Rent');
  expect(text).toContain('1,2024-03-01,2024-03-01,DBIT,-200.00,EUR,BOOK,,E2E-B,,Beta SARL,,Utilities');
  // Rolled up: one row with the batch total.
  await page.uncheck('#in-expand_details');
  await expect(out).toContainText('-300.00', { timeout: 15000 });
  text = (await out.textContent())!;
  expect(text).toContain('1,2024-03-01,2024-03-01,DBIT,-300.00,EUR,BOOK,,E2E-A,,Alpha GmbH,,Rent');
  expect(text).not.toContain('-200.00');
});

test('camt053-parse page reports a non-CAMT document clearly', async ({ page }) => {
  await page.goto('/tools/camt053-parse/');
  await page.fill('#in-data', '<note><to>You</to></note>');
  await expect(page.locator('#tool-output')).toContainText('not a CAMT bank statement', { timeout: 15000 });
});

test('camt053-parse page honors query-param deep link', async ({ page }) => {
  const data = encodeURIComponent(STATEMENT);
  await page.goto(`/tools/camt053-parse/?data=${data}&output=csv&date_format=iso&delimiter=comma&signed_amounts=true&expand_details=true`);
  await expect(page.locator('#in-data')).toHaveValue(STATEMENT);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Salary March', { timeout: 15000 });
  expect(await out.textContent()).toBe(CSV);
});

test('camt053-parse page download link serves exactly the visible CSV', async ({ page }) => {
  await page.goto('/tools/camt053-parse/');
  await page.fill('#in-data', STATEMENT);
  await page.selectOption('#in-output', 'csv');
  const out = page.locator('#tool-output');
  const dl = page.locator('#tool-output-download');
  await expect(out).toContainText('Salary March', { timeout: 15000 });
  await expect(dl).toBeVisible();
  expect(await dl.getAttribute('download')).toBe('camt053-parse-output.txt');
  const blobText = await page.evaluate(async () => {
    const a = document.getElementById('tool-output-download') as HTMLAnchorElement;
    return (await fetch(a.href)).text();
  });
  expect(blobText).toBe(CSV);
});
