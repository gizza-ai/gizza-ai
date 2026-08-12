import { test, expect } from './fixtures';

async function setBigTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('rest-file-to-curl expands a .http file with variables into curl', async ({ page }) => {
  await page.goto('/tools/rest-file-to-curl/');
  await setBigTextarea(
    page,
    '#in-data',
    '@host = https://api.example.com\n\nGET {{host}}/v1/users HTTP/1.1\nAccept: application/json'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText("curl 'https://api.example.com/v1/users'", { timeout: 15000 });
  await expect(out).toContainText("-H 'Accept: application/json'");
});

test('rest-file-to-curl deep link selects named environment and long single-line flags', async ({ page }) => {
  await page.goto('/tools/rest-file-to-curl/?environment=prod&shell=bash&flag_style=long&multiline=false&follow_redirects=true&include_comments=false');
  await expect(page.locator('#in-environment')).toHaveValue('prod', { timeout: 15000 });
  await expect(page.locator('#in-flag_style')).toHaveValue('long');
  await expect(page.locator('#in-multiline')).not.toBeChecked();
  await expect(page.locator('#in-follow_redirects')).toBeChecked();
  await expect(page.locator('#in-include_comments')).not.toBeChecked();
  await setBigTextarea(
    page,
    '#in-data',
    'POST {{host}}/v1/items\nContent-Type: application/json\nX-Trace-Id: {{trace_id}}\n\n{"name":"gizza"}'
  );
  await setBigTextarea(
    page,
    '#in-env',
    '{"dev":{"host":"http://localhost:3000","trace_id":"dev-trace"},"prod":{"host":"https://api.example.com","trace_id":"abc123"}}'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText("curl --request POST 'https://api.example.com/v1/items' --header 'Content-Type: application/json' --header 'X-Trace-Id: abc123' --data-raw '{\"name\":\"gizza\"}' --location", { timeout: 15000 });
});

test('rest-file-to-curl supports .ain templates and PowerShell output', async ({ page }) => {
  await page.goto('/tools/rest-file-to-curl/');
  await page.selectOption('#in-format', 'ain');
  await page.selectOption('#in-shell', 'powershell');
  await setBigTextarea(
    page,
    '#in-data',
    '[Method]\nPOST\n\n[Host]\nhttps://api.example.com/v1/orders\n\n[Query]\nlimit=10\n\n[Headers]\nX-Trace-Id: $TRACE_ID\nContent-Type: application/json\n\n[Body]\n{"ok":true}'
  );
  await setBigTextarea(page, '#in-env', 'TRACE_ID=abc123');
  const out = page.locator('#tool-output');
  await expect(out).toContainText("curl.exe -X POST 'https://api.example.com/v1/orders?limit=10'", { timeout: 15000 });
  await expect(out).toContainText("-H 'X-Trace-Id: abc123'");
  await expect(out).toContainText("-d '{\"ok\":true}'");
});
