import { test, expect } from './fixtures';

// /tools/secret-scanner/ statically flags hardcoded API keys, tokens, and
// private-key headers — pure wasm, in-browser. text is a multiline <textarea>;
// min_severity / format are <select>s; redact is a checkbox (default checked).

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

test('AWS access key is flagged HIGH (exact output, redacted by default)', async ({ page }) => {
  await page.goto('/tools/secret-scanner/');
  await page.fill('#in-text', 'aws_secret_key = "AKIAIOSFODNN7EXAMPLE"');
  await expect(page.locator('#tool-output')).toContainText('aws-access-key-id', { timeout: 15000 });
  expect(await outText(page)).toBe(
    '1 finding(s) (1 high, 0 medium) in 1 line(s) scanned\n\n' +
      'line 1, col 19  HIGH  aws-access-key-id  AWS Access Key ID\n' +
      '  AKIA…[redacted]\n\n' +
      'Recommendation: remove hardcoded secrets from source, rotate anything real that was exposed, ' +
      'and load credentials from environment variables or a secrets manager. A clean result means ' +
      'nothing matched, not that the code is secret-free.'
  );
  // Redaction hides the full key.
  expect(await outText(page)).not.toContain('AKIAIOSFODNN7EXAMPLE');
});

test('deep-link pre-fills and auto-runs (GitHub token)', async ({ page }) => {
  const text = encodeURIComponent('GITHUB_TOKEN=ghp_1234567890abcdefghijklmnopqrstuvwxyz');
  await page.goto(`/tools/secret-scanner/?text=${text}`);
  await expect(page.locator('#tool-output')).toContainText('github-token', { timeout: 15000 });
  expect(await outText(page)).toContain('1 finding(s) (1 high, 0 medium)');
  expect(await outText(page)).toContain('ghp_…[redacted]');
});

test('redact OFF (non-default checkbox) reveals the full value', async ({ page }) => {
  await page.goto('/tools/secret-scanner/');
  await page.fill('#in-text', 'key = AKIAIOSFODNN7EXAMPLE');
  await page.uncheck('#in-redact');
  await expect(page.locator('#tool-output')).toContainText('AKIAIOSFODNN7EXAMPLE', { timeout: 15000 });
  expect(await outText(page)).not.toContain('[redacted]');
});

test('generic keyword+entropy is MEDIUM; High-only filter hides it', async ({ page }) => {
  await page.goto('/tools/secret-scanner/');
  await page.fill('#in-text', 'api_key = "f4Kd9xQ2pLm7Zt1Rv8Nw3Bc6Yh0Ge5J"');
  // default min_severity = all → medium finding shows
  await expect(page.locator('#tool-output')).toContainText('generic-secret-assignment', { timeout: 15000 });
  expect(await outText(page)).toContain('(0 high, 1 medium)');

  await page.selectOption('#in-min_severity', 'high');
  await expect(page.locator('#tool-output')).toContainText('No hardcoded secrets found', { timeout: 15000 });
});

test('JSON output format returns structured findings', async ({ page }) => {
  await page.goto('/tools/secret-scanner/');
  await page.fill('#in-text', 'key = AKIAIOSFODNN7EXAMPLE');
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('"rule": "aws-access-key-id"', { timeout: 15000 });
  const parsed = JSON.parse(await outText(page));
  expect(parsed.summary.findings).toBe(1);
  expect(parsed.summary.high).toBe(1);
  expect(parsed.findings[0].severity).toBe('high');
  expect(parsed.findings[0].provider).toBe('AWS Access Key ID');
  expect(parsed.findings[0].line).toBe(1);
});

test('multiline: private-key header (HIGH) + generic assignment (MEDIUM), mixed counts', async ({ page }) => {
  await page.goto('/tools/secret-scanner/');
  await page.fill(
    '#in-text',
    'cert = "-----BEGIN EC PRIVATE KEY-----"\ndb_secret = "f4Kd9xQ2pLm7Zt1Rv8Nw3Bc6Yh0Ge5J"'
  );
  await expect(page.locator('#tool-output')).toContainText('private-key', { timeout: 15000 });
  const out = await outText(page);
  expect(out).toContain('generic-secret-assignment');
  expect(out).toContain('(1 high, 1 medium)');
  expect(out).toContain('in 2 line(s) scanned');
});

test('clean config produces no findings', async ({ page }) => {
  await page.goto('/tools/secret-scanner/');
  await page.fill('#in-text', 'port = 8080\nhost = "localhost"\ndebug = true');
  await expect(page.locator('#tool-output')).toContainText('No hardcoded secrets found', { timeout: 15000 });
});
