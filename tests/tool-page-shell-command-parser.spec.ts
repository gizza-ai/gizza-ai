import { test, expect } from './fixtures';

// /tools/shell-command-parser/ parses a shell command in-browser (pure wasm).
// input is a multiline textarea; format is a select; pretty is a checkbox.

test('shell-command-parser renders a command table with redirects', async ({ page }) => {
  await page.goto('/tools/shell-command-parser/');
  await page.fill('#in-input', 'a -1 | b -2 > o.txt');
  await page.selectOption('#in-format', 'commands');
  const pretty = page.locator('#in-pretty');
  if (!(await pretty.isChecked())) await pretty.check();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('#  COMMAND', { timeout: 15000 });
  await expect(out).toContainText('1  a');
  await expect(out).toContainText('2  b');
  await expect(out).toContainText('> o.txt');
});

test('shell-command-parser explains quoting and expansions with non-default pretty', async ({ page }) => {
  await page.goto('/tools/shell-command-parser/');
  await page.fill('#in-input', 'echo "$HOME/$(date +%F)" \'raw $x\' *.txt');
  await page.selectOption('#in-format', 'explain');
  await page.uncheck('#in-pretty');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('runs `echo', { timeout: 15000 });
  await expect(out).toContainText('parameter expansion $HOME');
  await expect(out).toContainText('command substitution $(date +%F)');
  await expect(out).toContainText('contains a glob');
});

test('shell-command-parser supports deep-linked parameters', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'LC_ALL=C grep -rn TODO src/ 2>/dev/null | sort -u > todos.txt',
    format: 'tree',
    pretty: 'true',
  });
  await page.goto(`/tools/shell-command-parser/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue('LC_ALL=C grep -rn TODO src/ 2>/dev/null | sort -u > todos.txt');
  await expect(page.locator('#in-format')).toHaveValue('tree');
  await expect(page.locator('#tool-output')).toContainText('pipeline (2 commands)', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('stderr (fd 2) is written to /dev/null');
});
