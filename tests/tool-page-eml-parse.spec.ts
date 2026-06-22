import { test, expect } from './fixtures';

const EML =
  'From: Alice <alice@example.com>\n' +
  'To: Bob <bob@example.org>\n' +
  'Subject: Test Email\n' +
  'Date: Mon, 1 Jan 2024 10:00:00 +0000\n' +
  'MIME-Version: 1.0\n' +
  'Content-Type: multipart/mixed; boundary="X"\n' +
  '\n' +
  '--X\n' +
  'Content-Type: text/plain\n' +
  '\n' +
  'Hello world body.\n' +
  '--X\n' +
  'Content-Type: application/pdf; name="doc.pdf"\n' +
  'Content-Disposition: attachment; filename="doc.pdf"\n' +
  '\n' +
  '%PDF-1.4\n' +
  '--X--\n';

// /tools/eml-parse/ parses a raw .eml message in-browser (pure wasm).
test('eml-parse extracts headers, body, and attachment', async ({ page }) => {
  await page.goto('/tools/eml-parse/');
  await page.fill('#in-eml', EML);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Subject: Test Email', { timeout: 15000 });
  await expect(out).toContainText('alice@example.com');
  await expect(out).toContainText('bob@example.org');
  await expect(out).toContainText('doc.pdf');
  await expect(out).toContainText('application/pdf');
  await expect(out).toContainText('Hello world body.');
});
