import { test, expect } from './fixtures';

const SAMPLE = 'type User = { name: string }\nexport function greet(name: string): string {\n  return `Hello ${name}` as string;\n}';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('typescript-transpiler page strips type syntax', async ({ page }) => {
  await page.goto('/tools/typescript-transpiler/');
  await page.fill('#in-input', SAMPLE);
  await page.selectOption('#in-enum_style', 'compile');
  await expect(page.locator('#tool-output')).toContainText('export function greet(name)', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('export function greet(name) {');
  expect(out).toContain('return `Hello ${name}`;');
  expect(out).not.toContain(': string');
  expect(out).not.toContain('type User');
});

test('typescript-transpiler supports enum strip and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    input: '// color\nenum Color { Red, Green = 4, Blue }\nconst current: Color = Color.Blue;',
    enum_style: 'strip',
    remove_comments: 'true',
  });
  await page.goto(`/tools/typescript-transpiler/?${params.toString()}`);
  await expect(page.locator('#in-remove_comments')).toBeChecked({ timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('const current= Color.Blue;', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).not.toContain('enum Color');
  expect(out).not.toContain('// color');
});

test('typescript-transpiler deep-link pre-fills and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    input: SAMPLE,
    enum_style: 'compile',
    remove_comments: 'true',
  });
  await page.goto(`/tools/typescript-transpiler/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-enum_style')).toHaveValue('compile');
  await expect(page.locator('#in-remove_comments')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('export function greet(name)', { timeout: 15000 });
});
