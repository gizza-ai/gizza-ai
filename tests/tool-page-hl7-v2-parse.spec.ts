import { test, expect } from './fixtures';

const ADT = 'MSH|^~\\&|SENDINGAPP|SENDINGFAC|RECVAPP|RECVFAC|20240101120000||ADT^A01|MSG00001|P|2.5\nEVN|A01|20240101120000\nPID|1||123456^^^HOSPITAL^MR||DOE^JOHN^Q||19800101|M|||123 MAIN ST^^ANYTOWN^CA^90210';
const ORU = 'MSH|^~\\&|LAB|LABFAC|EHR|HOSP|20240102083000||ORU^R01|MSG42|P|2.5\nPID|1||987654^^^HOSP^MR||SMITH^JANE||19751212|F\nOBR|1||ORD123|CBC^Complete Blood Count\nOBX|1|NM|WBC^White Blood Cell Count||7.2|10*3/uL|4.0-11.0|N|||F';

async function setData(page: import('@playwright/test').Page, data: string) {
  await page.locator('#in-data').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, data);
}

test('hl7 page parses ADT message to described nested JSON', async ({ page }) => {
  await page.goto('/tools/hl7-v2-parse/');
  await setData(page, ADT);
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"segment": "MSH"', { timeout: 15_000 });
  await expect(out).toContainText('"description": "Message Header"');
  await expect(out).toContainText('"id": "MSH.9"');
  await expect(out).toContainText('"Message Type"');
  await expect(out).toContainText('"ADT"');
  await expect(out).toContainText('"A01"');
  await expect(out).toContainText('"id": "PID.5"');
  await expect(out).toContainText('"Patient Name"');
  await expect(out).toContainText('"DOE"');
  await expect(out).toContainText('"JOHN"');
});

test('hl7 page deep-links CSV output and preserves raw escapes when unescape is false', async ({ page }) => {
  const msg = 'MSH|^~\\&|A\nNTE|1||Line one\\.br\\Line two \\T\\ more';
  const qs = new URLSearchParams({
    data: msg,
    output: 'csv',
    include_descriptions: 'true',
    unescape: 'false',
  });
  await page.goto(`/tools/hl7-v2-parse/?${qs.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#in-unescape')).not.toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Segment,Location,Value,Description', { timeout: 15_000 });
  await expect(out).toContainText('NTE,NTE.3,Line one\\.br\\Line two \\T\\ more,');
});

test('hl7 page covers CSV enum and no-description checkbox state', async ({ page }) => {
  await page.goto('/tools/hl7-v2-parse/');
  await setData(page, ORU);
  await page.selectOption('#in-output', 'csv');
  await page.uncheck('#in-include_descriptions');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Segment,Location,Value', { timeout: 15_000 });
  await expect(out).not.toContainText('Description');
  await expect(out).toContainText('OBX,OBX.3.1,WBC');
  await expect(out).toContainText('OBX,OBX.5,7.2');
});

test('hl7 page reports empty-input and unknown-output errors', async ({ page }) => {
  await page.goto('/tools/hl7-v2-parse/');
  await setData(page, '');
  await expect(page.locator('#tool-output')).toContainText('no HL7 segments found', { timeout: 15_000 });

  await setData(page, ADT);
  await page.locator('#in-output').evaluate((el) => {
    const select = el as HTMLSelectElement;
    const opt = document.createElement('option');
    opt.value = 'xml';
    opt.text = 'xml';
    select.appendChild(opt);
    select.value = 'xml';
    select.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await expect(page.locator('#tool-output')).toContainText("unknown output 'xml'", { timeout: 15_000 });
});
