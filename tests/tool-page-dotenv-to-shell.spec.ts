import { test, expect } from './fixtures';

test('dotenv-to-shell page emits exact POSIX exports', async ({ page }) => {
  await page.goto('/tools/dotenv-to-shell/');
  await page.fill('#in-input', 'FOO=bar\nGREETING=hello world\nDOLLAR=$HOME');
  await page.selectOption('#in-direction', 'to-shell');
  await page.selectOption('#in-shell', 'posix');
  await page.selectOption('#in-quote', 'auto');

  const out = page.locator('#tool-output');
  await expect(out).toContainText("export GREETING='hello world'", { timeout: 15_000 });
  expect(await out.textContent()).toBe("export FOO=bar\nexport GREETING='hello world'\nexport DOLLAR='$HOME'");
});

test('dotenv-to-shell deep link supports reverse conversion and fish/single controls', async ({ page }) => {
  const reverseInput = "export FOO=bar\nexport GREETING='hello world'";
  const qs =
    '?input=' + encodeURIComponent(reverseInput) +
    '&direction=to-env' +
    '&shell=fish' +
    '&quote=single';
  await page.goto('/tools/dotenv-to-shell/' + qs);

  await expect(page.locator('#in-direction')).toHaveValue('to-env', { timeout: 15_000 });
  await expect(page.locator('#in-shell')).toHaveValue('fish');
  await expect(page.locator('#in-quote')).toHaveValue('single');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('FOO=bar', { timeout: 15_000 });
  expect(await out.textContent()).toBe('FOO=bar\nGREETING="hello world"');
});
