import { test, expect } from './fixtures';

const BAD_BASH = `#!/usr/bin/env bash
name=$1
echo Hello $name
for f in $(ls *.txt); do
  cat $f | while read line; do
    echo $line
  done
done`;

const POSIX_BASHISMS = `#!/bin/sh
items=(one two)
if [[ -n "$1" ]]; then
  echo \${items[0]}
fi`;

async function runWasm(
  page: any,
  script: string = BAD_BASH,
  shell = 'bash',
  minSeverity = 'all',
  ignore = '',
  format = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/shell-script-linter/gizza_ai_shell_script_linter_web.js');
    await mod.default('/tools/shell-script-linter/gizza_ai_shell_script_linter_web_bg.wasm');
    return mod.run(args.script, args.shell, args.minSeverity, args.ignore, args.format);
  }, { script, shell, minSeverity, ignore, format });
}

test('shell-script-linter page reports real shell pitfalls', async ({ page }) => {
  await page.goto('/tools/shell-script-linter/');
  await page.fill('#in-script', BAD_BASH);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Shell lint (bash) · 7 findings', { timeout: 20_000 });
  await expect(output).toContainText('STRICT-MODE');
  await expect(output).toContainText('UNQUOTED-VAR');
  await expect(output).toContainText('SUBSHELL-SCOPE');
  await expect(output).toContainText('USELESS-CAT');
});

test('shell-script-linter deep link covers POSIX shell, JSON output and severity filter', async ({ page }) => {
  const params = new URLSearchParams({
    script: POSIX_BASHISMS,
    shell: 'sh',
    min_severity: 'warning',
    ignore: 'STRICT-MODE',
    format: 'json',
  });
  await page.goto(`/tools/shell-script-linter/?${params.toString()}`);

  await expect(page.locator('#in-shell')).toHaveValue('sh', { timeout: 15_000 });
  await expect(page.locator('#in-min_severity')).toHaveValue('warning');
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"shell": "sh"', { timeout: 20_000 });
  await expect(page.locator('#tool-output')).toContainText('"code": "SH-BASHISM"');
  await expect(page.locator('#tool-output')).not.toContainText('STRICT-MODE');
});

test('shell-script-linter wasm covers enums, ignore, cap boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/shell-script-linter/');

  const bash = await runWasm(page, BAD_BASH, 'bash', 'all', '', 'text');
  expect(bash).toContain('Shell lint (bash) · 7 findings');
  expect(bash).toContain('L5 [warning] SUBSHELL-SCOPE');

  const zsh = await runWasm(page, '#!/usr/bin/env zsh\necho $name', 'zsh', 'warning', '', 'json');
  expect(zsh).toContain('"shell": "zsh"');
  expect(zsh).toContain('"code": "UNQUOTED-VAR"');

  const sh = await runWasm(page, POSIX_BASHISMS, 'dash', 'warning', 'STRICT-MODE UNQUOTED-VAR', 'text');
  expect(sh).toContain('Shell lint (dash)');
  expect(sh).toContain('SH-BASHISM');
  expect(sh).not.toContain('STRICT-MODE');
  expect(sh).not.toContain('UNQUOTED-VAR');

  const clean = await runWasm(page, '#!/usr/bin/env bash\nset -euo pipefail\nprintf "%s\\n" "$1"', 'auto', 'all', '', 'text');
  expect(clean).toContain('Shell lint (bash) · 0 findings');
  expect(clean).toContain('No issues found.');

  const boundary = `#!/usr/bin/env bash\nset -euo pipefail\n# ${'a'.repeat(199_960)}`;
  const boundaryOut = await runWasm(page, boundary, 'auto', 'all', '', 'text');
  expect(boundaryOut).toContain('Shell lint (bash) · 0 findings');
  await expect(runWasm(page, `${boundary}x`, 'auto', 'all', '', 'text')).rejects.toThrow(/too large/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool shell-script-linter');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
