import { test, expect } from './fixtures';

const cipher = 'Wkh txlfn eurzq ira mxpsv ryhu wkh odcb grj.';
const plain = 'The quick brown fox jumps over the lazy dog.';

test('caesar-cipher-breaker page finds the best shift', async ({ page }) => {
  await page.goto('/tools/caesar-cipher-breaker/');
  await page.fill('#in-input', cipher);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Best shift: 3', { timeout: 20_000 });
  await expect(output).toContainText(plain);
});

test('caesar-cipher-breaker deep link shows ranked candidates', async ({ page }) => {
  const params = new URLSearchParams({
    input: cipher,
    output: 'ranked',
    language: 'english',
    top: '3',
    shift_digits: 'false',
  });

  await page.goto(`/tools/caesar-cipher-breaker/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('ranked', { timeout: 15_000 });
  await expect(page.locator('#in-top')).toHaveValue('3');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Top 3 of 26 shifts by score:', { timeout: 20_000 });
  await expect(output).toContainText('shift  3');
  await expect(output).toContainText(plain);
});

test('caesar-cipher-breaker rotates digits when requested', async ({ page }) => {
  await page.goto('/tools/caesar-cipher-breaker/');
  await page.fill('#in-input', 'Phhw dw 45 vkdus eb jdwh 2, eulqj wkh eoxh iroghu');
  await page.check('#in-shift_digits');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Best shift: 3', { timeout: 20_000 });
  await expect(output).toContainText('Meet at 12 sharp by gate 9, bring the blue folder');
});
