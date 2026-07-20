import { test, expect } from './fixtures';

// /tools/sql-injection-scanner/ statically flags SQL built by concatenation /
// interpolation / format calls, and bare variables executed as SQL — pure wasm,
// in-browser. code is a multiline <textarea>; language / min_severity / format
// are <select>s.

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

test('python string concatenation is flagged HIGH (exact output)', async ({ page }) => {
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', 'cursor.execute("SELECT * FROM users WHERE name = \'" + name + "\'")');
  await page.selectOption('#in-language', 'python');
  await expect(page.locator('#tool-output')).toContainText('SQLI-CONCAT', { timeout: 15000 });
  expect(await outText(page)).toBe(
    '1 finding(s) (1 high, 0 medium) in 1 line(s) scanned\n\n' +
      'line 1, col 51  HIGH  SQLI-CONCAT  SQL query built with string concatenation; a variable is joined directly into the SQL text.\n' +
      '  cursor.execute("SELECT * FROM users WHERE name = \'" + name + "\'")\n\n' +
      'Fix: use parameterized queries / prepared statements — pass user input as bound parameters (?, %s, :name, $1), never concatenate it into the SQL string.\n' +
      'Example: cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))'
  );
});

test('deep-link pre-fills and auto-runs (f-string, INTERP)', async ({ page }) => {
  const code = encodeURIComponent('cursor.execute(f"DELETE FROM logs WHERE id = {log_id}")');
  await page.goto(`/tools/sql-injection-scanner/?code=${code}&language=python`);
  await expect(page.locator('#tool-output')).toContainText('SQLI-INTERP', { timeout: 15000 });
  expect(await outText(page)).toContain('1 finding(s) (1 high, 0 medium)');
});

test('PHP "." concatenation: flagged as php, ignored as javascript (language scoping)', async ({ page }) => {
  const code = '$sql = "SELECT * FROM users WHERE id = " . $id;';
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', code);
  await page.selectOption('#in-language', 'php');
  await expect(page.locator('#tool-output')).toContainText('SQLI-CONCAT', { timeout: 15000 });

  await page.selectOption('#in-language', 'javascript');
  await expect(page.locator('#tool-output')).toContainText('No injection-prone SQL construction', {
    timeout: 15000,
  });
});

test('JS template literal is flagged INTERP', async ({ page }) => {
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', 'db.query(`SELECT * FROM users WHERE id = ${id}`)');
  await page.selectOption('#in-language', 'javascript');
  await expect(page.locator('#tool-output')).toContainText('SQLI-INTERP', { timeout: 15000 });
});

test('go sprintf is flagged FORMAT', async ({ page }) => {
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', 'q := fmt.Sprintf("SELECT * FROM users WHERE id = %s", id)');
  await page.selectOption('#in-language', 'go');
  await expect(page.locator('#tool-output')).toContainText('SQLI-FORMAT', { timeout: 15000 });
});

test('percent-format is flagged FORMAT (python)', async ({ page }) => {
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', 'sql = "SELECT * FROM t WHERE a = %s" % val');
  await page.selectOption('#in-language', 'python');
  await expect(page.locator('#tool-output')).toContainText('SQLI-FORMAT', { timeout: 15000 });
});

test('bare variable executed is MEDIUM; High-only filter hides it', async ({ page }) => {
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', 'cursor.execute(sql)');
  // default min_severity = all → medium finding shows
  await expect(page.locator('#tool-output')).toContainText('SQLI-EXEC-VAR', { timeout: 15000 });
  expect(await outText(page)).toContain('(0 high, 1 medium)');

  await page.selectOption('#in-min_severity', 'high');
  await expect(page.locator('#tool-output')).toContainText('No injection-prone SQL construction', {
    timeout: 15000,
  });
});

test('parameterized query produces no findings', async ({ page }) => {
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', 'cursor.execute("SELECT * FROM users WHERE id = %s", (uid,))');
  await page.selectOption('#in-language', 'python');
  await expect(page.locator('#tool-output')).toContainText('No injection-prone SQL construction', {
    timeout: 15000,
  });
});

test('JSON output format returns structured findings', async ({ page }) => {
  await page.goto('/tools/sql-injection-scanner/');
  await page.fill('#in-code', 'query = "SELECT * FROM t WHERE n = \'" + name + "\'"');
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('"rule": "SQLI-CONCAT"', { timeout: 15000 });
  const parsed = JSON.parse(await outText(page));
  expect(parsed.summary.findings).toBe(1);
  expect(parsed.summary.high).toBe(1);
  expect(parsed.findings[0].severity).toBe('high');
  expect(parsed.findings[0].line).toBe(1);
});
