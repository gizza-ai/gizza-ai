import { test, expect } from './fixtures';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

const BAD = 'void log_user(char *s) {\n  char buf[16];\n  strcpy(buf, s);\n  printf(s);\n  gets(buf);\n}';

test('default text report flags risky C patterns with CWE ids and context', async ({ page }) => {
  await page.goto('/tools/c-vuln-pattern-scanner/');
  await page.fill('#in-code', BAD);
  await expect(page.locator('#tool-output')).toContainText('3 findings', { timeout: 15000 });
  const out = await outText(page);
  expect(out).toContain('1 critical · 2 high · 0 medium · 0 low');
  expect(out).toContain('BANNED-COPY (CWE-120)');
  expect(out).toContain('FORMAT-STRING (CWE-134)');
  expect(out).toContain('GETS (CWE-242)');
  expect(out).toContain('  strcpy(buf, s);');
});

test('deep-link pre-fills and auto-runs the critical-only filter', async ({ page }) => {
  await page.goto(
    `/tools/c-vuln-pattern-scanner/?code=${encodeURIComponent(BAD)}&min_severity=critical&include_context=false`
  );
  await expect(page.locator('#tool-output')).toContainText('1 findings', { timeout: 15000 });
  const out = await outText(page);
  expect(out).toContain('1 critical · 0 high · 0 medium · 0 low');
  expect(out).toContain('GETS (CWE-242)');
  expect(out).not.toContain('BANNED-COPY');
  expect(out).not.toContain('  gets(buf);');
});

test('CSV output is real table data and include_context=false blanks source', async ({ page }) => {
  await page.goto('/tools/c-vuln-pattern-scanner/');
  await page.fill('#in-code', BAD);
  await page.selectOption('#in-format', 'csv');
  await page.uncheck('#in-include_context');
  await expect(page.locator('#tool-output')).toContainText('line,severity,code,cwe,message,source', { timeout: 15000 });
  const out = await outText(page);
  const rows = out.trim().split('\n');
  expect(rows[0]).toBe('line,severity,code,cwe,message,source');
  expect(rows).toHaveLength(4);
  expect(rows.some((row) => row.startsWith('5,critical,GETS,CWE-242,'))).toBe(true);
  expect(rows.every((row, idx) => idx === 0 || row.endsWith(','))).toBe(true);
});

test('JSON output and profile filter expose structured injection findings', async ({ page }) => {
  await page.goto('/tools/c-vuln-pattern-scanner/');
  await page.fill('#in-code', 'void f(char *s) {\n  strcpy(dst, s);\n  system(s);\n  fprintf(stderr, s);\n}');
  await page.selectOption('#in-profile', 'injection');
  await page.selectOption('#in-format', 'json');
  await expect(page.locator('#tool-output')).toContainText('"profile": "injection"', { timeout: 15000 });
  const parsed = JSON.parse(await outText(page));
  expect(parsed.summary.findings).toBe(2);
  expect(parsed.findings.map((f: { code: string }) => f.code).sort()).toEqual(['COMMAND-EXEC', 'FORMAT-STRING']);
  expect(parsed.findings.every((f: { cwe: string }) => f.cwe.startsWith('CWE-'))).toBe(true);
});

test('ignore list and C++ auto-detection exercise advertised enum values', async ({ page }) => {
  await page.goto('/tools/c-vuln-pattern-scanner/');
  await page.fill('#in-code', '#include <iostream>\nint main(){ char name[16]; std::cin >> name; strncpy(name, src, 8); }');
  await page.fill('#in-ignore', 'BOUNDED-COPY');
  await expect(page.locator('#tool-output')).toContainText('C/C++ vulnerability scan (cpp)', { timeout: 15000 });
  const out = await outText(page);
  expect(out).toContain('CPP-STREAM (CWE-120)');
  expect(out).not.toContain('BOUNDED-COPY');
});
