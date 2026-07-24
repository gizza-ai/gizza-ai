import { test, expect } from './fixtures';

// Defaults: format=auto, detect_types=on, pretty=on, output=json.

test('text-to-json auto-detects logfmt into an array of objects', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill(
    '#in-text',
    'level=info msg="server started" port=8080\nlevel=error msg="connection refused" code=502',
  );
  const out = page.locator('#tool-output');
  // Pretty-printed by default, and port=8080 is inferred as a number.
  await expect(out).toContainText('"level": "info"', { timeout: 15000 });
  await expect(out).toContainText('"msg": "server started"');
  await expect(out).toContainText('"port": 8080');
  await expect(out).toContainText('"code": 502');
});

test('format=ini pins nested-object parsing', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', 'app = demo\n\n[server]\nhost = localhost\nport = 8080');
  await page.selectOption('#in-format', 'ini');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"app": "demo"', { timeout: 15000 });
  await expect(out).toContainText('"server"');
  await expect(out).toContainText('"host": "localhost"');
  await expect(out).toContainText('"port": 8080');
});

test('format=keyvalue tolerates export prefix and comments', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', '# app config\nexport HOST=localhost\nPORT=8080\nDEBUG=true');
  await page.selectOption('#in-format', 'keyvalue');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"HOST": "localhost"', { timeout: 15000 });
  await expect(out).toContainText('"PORT": 8080');
  await expect(out).toContainText('"DEBUG": true');
});

test('format=csv reads header row into an array with type inference', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', 'name,age,admin\nAlice,30,true\nBob,25,false');
  await page.selectOption('#in-format', 'csv');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "Alice"', { timeout: 15000 });
  await expect(out).toContainText('"age": 30');
  await expect(out).toContainText('"admin": true');
  await expect(out).toContainText('"name": "Bob"');
});

test('format=passwd maps colon fields to named columns', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', 'root:x:0:0:root:/root:/bin/bash');
  await page.selectOption('#in-format', 'passwd');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "root"', { timeout: 15000 });
  await expect(out).toContainText('"uid": 0');
  await expect(out).toContainText('"shell": "/bin/bash"');
});

test('output=report surfaces the auto-detected format and record count', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', 'host=a env=prod\nhost=b env=dev');
  await page.selectOption('#in-output', 'report');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"detected_format": "logfmt"', { timeout: 15000 });
  await expect(out).toContainText('"record_count": 2');
  await expect(out).toContainText('"data"');
});

test('output=ndjson emits one exact compact record per line', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', 'name,x\nA,1\nB,2');
  await page.selectOption('#in-format', 'csv');
  await page.selectOption('#in-output', 'ndjson');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"name":"A","x":1}', { timeout: 15000 });
  await expect(out).toContainText('{"name":"B","x":2}');
});

test('unchecking detect_types keeps every value a lossless string', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', 'name,age\nAlice,007');
  await page.selectOption('#in-format', 'csv');
  await page.uncheck('#in-detect_types'); // non-default: default is checked (on)
  const out = page.locator('#tool-output');
  // Leading zero preserved as a string, not coerced to the number 7.
  await expect(out).toContainText('"age": "007"', { timeout: 15000 });
});

test('unchecking pretty produces exact minified JSON', async ({ page }) => {
  await page.goto('/tools/text-to-json/');
  await page.fill('#in-text', 'k=v');
  await page.selectOption('#in-format', 'keyvalue');
  await page.uncheck('#in-pretty'); // non-default: default is checked (on)
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('{"k":"v"}', { timeout: 15000 });
});

test('query-param deep-link prefills fields and converts', async ({ page }) => {
  const text = 'root:x:0:0:root:/root:/bin/bash\ndaemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin';
  await page.goto(
    '/tools/text-to-json/?text=' + encodeURIComponent(text) + '&format=passwd&output=ndjson',
  );
  await expect(page.locator('#in-text')).toHaveValue(text, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('passwd');
  const out = page.locator('#tool-output');
  // NDJSON deep-link: one compact object per passwd line, uid/gid inferred as numbers.
  await expect(out).toContainText(
    '{"name":"root","password":"x","uid":0,"gid":0,"gecos":"root","home":"/root","shell":"/bin/bash"}',
    { timeout: 15000 },
  );
  await expect(out).toContainText('"name":"daemon"');
});
