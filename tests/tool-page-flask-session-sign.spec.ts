import { test, expect } from './fixtures';

test('flask-session-sign produces a Flask default signed cookie', async ({ page }) => {
  await page.goto('/tools/flask-session-sign/');
  await page.fill('#in-payload', '{"logged_in":true}');
  await page.fill('#in-secret', 'CHANGEME');
  await page.fill('#in-timestamp', '1547409146');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"timestamp_iso": "2019-01-13T19:52:26Z"', { timeout: 15000 });
  await expect(output).toContainText('"signature_segment": "cPCkFmmeB7qNIcN-ReiN72r0hvU"');
  await expect(output).toContainText('"set_cookie_header"');
});

test('flask-session-sign deep link applies enum and checkbox params', async ({ page }) => {
  const qs =
    '?payload=' + encodeURIComponent('{"a":1}') +
    '&secret=' + encodeURIComponent('4142') +
    '&secret_encoding=hex' +
    '&digest=sha256' +
    '&key_derivation=none' +
    '&timestamp=1700000000' +
    '&legacy_epoch=true' +
    '&compress=never' +
    '&cookie_name=admin_session';

  await page.goto('/tools/flask-session-sign/' + qs);
  await expect(page.locator('#in-payload')).toHaveValue('{"a":1}', { timeout: 15000 });
  await expect(page.locator('#in-secret_encoding')).toHaveValue('hex');
  await expect(page.locator('#in-digest')).toHaveValue('sha256');
  await expect(page.locator('#in-key_derivation')).toHaveValue('none');
  await expect(page.locator('#in-legacy_epoch')).toBeChecked();
  await expect(page.locator('#in-compress')).toHaveValue('never');
  await expect(page.locator('#in-cookie_name')).toHaveValue('admin_session');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"timestamp": 1700000000');
  await expect(output).toContainText('"derived_key_hex": "4142"');
  await expect(output).toContainText('"compressed": false');
  await expect(output).toContainText('admin_session=');
});
