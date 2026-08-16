import { test, expect } from './fixtures';

const tool = '/tools/check-digit-validator/';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  value: string,
  scheme = 'auto',
  mode = 'validate',
  showSteps = 'false',
): Promise<string> {
  return await page.evaluate(
    async ({ value, scheme, mode, showSteps }) => {
      const mod = await import('/tools/check-digit-validator/gizza_ai_check_digit_validator_web.js');
      await mod.default('/tools/check-digit-validator/gizza_ai_check_digit_validator_web_bg.wasm');
      return mod.run(value, scheme, mode, showSteps);
    },
    { value, scheme, mode, showSteps },
  );
}

test('check-digit-validator page reports exact batch verdicts', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(
    page.locator('#in-value'),
    '4539 1488 0343 6467\n978-0-306-40615-7\nGB82 WEST 1234 5698 7654 32\n021000021',
  );
  await page.selectOption('#in-scheme', 'auto');
  await page.selectOption('#in-mode', 'validate');
  await page.uncheck('#in-show_steps');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Checked 4 codes — 4 valid, 0 invalid.', { timeout: 15000 });
  await expect(out).toContainText('VALID — Credit card (Luhn mod-10) · Visa');
  await expect(out).toContainText('VALID — ISBN-13 (GS1 mod-10)');
  await expect(out).toContainText('VALID — IBAN (mod-97-10) · United Kingdom, 22 characters');
  await expect(out).toContainText('VALID — ABA routing number · Federal Reserve routing symbol 0210');
});

test('check-digit-validator deep link pre-fills non-default state and shows arithmetic', async ({ page }) => {
  const qs = new URLSearchParams({
    value: '9780306406158',
    scheme: 'isbn13',
    mode: 'validate',
    show_steps: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-value')).toHaveValue('9780306406158', { timeout: 15000 });
  await expect(page.locator('#in-scheme')).toHaveValue('isbn13');
  await expect(page.locator('#in-mode')).toHaveValue('validate');
  await expect(page.locator('#in-show_steps')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID — ISBN-13 (GS1 mod-10)');
  await expect(out).toContainText('expected check digit 7, got 8');
  await expect(out).toContainText('steps: GS1 weighted sum');
});

test('check-digit-validator wasm covers advertised schemes and compute mode', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-value');

  expect(await runWasm(page, '79927398713', 'luhn')).toContain('VALID — Luhn (mod-10)');
  expect(await runWasm(page, '490154203237518', 'imei')).toContain('VALID — IMEI (15-digit Luhn)');
  expect(await runWasm(page, '1234567893', 'npi')).toContain('VALID — NPI (10-digit Luhn)');
  expect(await runWasm(page, '0-8044-2957-X', 'isbn10')).toContain('VALID — ISBN-10 (mod-11)');
  expect(await runWasm(page, '0378-5955', 'issn')).toContain('VALID — ISSN (mod-11)');
  expect(await runWasm(page, '96385074', 'ean8')).toContain('VALID — EAN-8 / GTIN-8');
  expect(await runWasm(page, '036000291452', 'upc-a')).toContain('VALID — UPC-A / GTIN-12');
  expect(await runWasm(page, '10614141000415', 'gtin14')).toContain('VALID — GTIN-14 / ITF-14');
  expect(await runWasm(page, '340123450000000018', 'sscc')).toContain('VALID — SSCC-18');
  expect(await runWasm(page, 'US0378331005', 'isin')).toContain('VALID — ISIN (Luhn on expanded letters)');
  expect(await runWasm(page, '1HGCM82633A004352', 'vin')).toContain('VALID — VIN (mod-11 transliteration)');

  expect(await runWasm(page, '978030640615', 'isbn13', 'compute', 'true')).toContain('full code: 9780306406157');
  expect(await runWasm(page, 'GBWEST12345698765432', 'iban', 'compute')).toContain('full code: GB82WEST12345698765432');
  await expect(runWasm(page, '978030640615', 'auto', 'compute')).rejects.toThrow(/compute mode needs an explicit scheme/);
});

test('check-digit-validator enforces the advertised 5000-code cap at the boundary', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-value');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/check-digit-validator/gizza_ai_check_digit_validator_web.js');
    await mod.default('/tools/check-digit-validator/gizza_ai_check_digit_validator_web_bg.wasm');
    const one = '4539148803436467';
    const atCap = Array(5000).fill(one).join('\n');
    const overCap = atCap + '\n' + one;
    const call = (value: string) => {
      try {
        return { ok: true, value: mod.run(value, 'auto', 'validate', 'false').slice(0, 50) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toContain('Checked 5000 codes');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/too many codes: 5001/);
});

test('check-digit-validator page ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(6);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Payment card — auto-detect',
    'Book barcode with a typo',
    'Mixed batch — cards, books, IBAN',
    'Compute an ISBN-13 check digit',
    'IBAN from country + account number',
    'VIN check character',
  ]);
});
