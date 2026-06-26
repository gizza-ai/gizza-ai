import { test, expect } from './fixtures';

test('list-converter page functionalities', async ({ page }) => {
  await page.goto('/tools/list-converter/');

  const inputTextarea = page.locator('#in-input');
  const outputTextarea = page.locator('#list-conv-output-area');
  const outFormatSelect = page.locator('#in-output_format');
  const sortModeSelect = page.locator('#in-sort_mode');
  const caseSelect = page.locator('#in-case_transform');
  const prefixInput = page.locator('#in-prefix');
  const suffixInput = page.locator('#in-suffix');
  const dedupeCheckbox = page.locator('#in-dedupe');
  const resetBtn = page.locator('.list-conv-btn-clear');
  const swapBtn = page.locator('.list-conv-btn-swap');

  // 1. Basic auto separator to newline conversion
  await inputTextarea.fill('apple, banana, cherry');
  await expect(outputTextarea).toHaveValue('apple\nbanana\ncherry', { timeout: 15000 });

  // 2. Change output layout to CSV/comma
  await outFormatSelect.selectOption('comma');
  await expect(outputTextarea).toHaveValue('apple, banana, cherry', { timeout: 10000 });

  // 3. Test sorting and deduplication
  await dedupeCheckbox.check();
  await inputTextarea.fill('banana, apple, cherry, banana');
  // Deduplicated output: banana, apple, cherry
  await expect(outputTextarea).toHaveValue('banana, apple, cherry', { timeout: 10000 });

  await sortModeSelect.selectOption('asc');
  // Sorted & Deduplicated: apple, banana, cherry
  await expect(outputTextarea).toHaveValue('apple, banana, cherry', { timeout: 10000 });

  // 4. Test casing, prefix and suffix
  await caseSelect.selectOption('uppercase');
  await prefixInput.fill('FRUIT:');
  await suffixInput.fill('!');
  // Transformed output: FRUIT:APPLE!, FRUIT:BANANA!, FRUIT:CHERRY!
  await expect(outputTextarea).toHaveValue('FRUIT:APPLE!, FRUIT:BANANA!, FRUIT:CHERRY!', { timeout: 10000 });

  // 5. Test Swap button
  await swapBtn.click();
  // Input should now contain the transformed output text
  await expect(inputTextarea).toHaveValue('FRUIT:APPLE!, FRUIT:BANANA!, FRUIT:CHERRY!', { timeout: 10000 });

  // 6. Test Reset button
  await resetBtn.click();
  await expect(inputTextarea).toHaveValue('');
  await expect(outputTextarea).toHaveValue('');
});
