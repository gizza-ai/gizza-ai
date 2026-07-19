import { test, expect } from './fixtures';

// Deterministic expectations use the "your own dice rolls" path — the wordlist
// lookup is pure, so the full report text is exact. Random-path tests assert
// structure (word count, separator, charset) instead of specific words.

const ROLLS = '62315 14534 23633 31662 35553 44151';
const ROLLS_TEXT =
  'tiger-canal-dolphin-garlic-lantern-pebble\n\nEntropy: 77.5 bits — strength: strong\nRecipe: 6 words from the EFF long list (7,776 words, 12.9 bits/word)\nCrack time: about 339 thousand years (offline, 10 billion guesses/sec)';
const ROLLS_TEXT_WITH_DICE =
  ROLLS_TEXT +
  '\n\nDice rolls:\n62315  tiger\n14534  canal\n23633  dolphin\n31662  garlic\n35553  lantern\n44151  pebble';

test('diceware-passphrase page generates 6 hyphenated EFF-long words by default', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Entropy: 77.5 bits — strength: strong', { timeout: 15000 });
  await expect(out).toContainText('Recipe: 6 words from the EFF long list (7,776 words, 12.9 bits/word)');
  const text = (await out.textContent()) ?? '';
  const phrase = text.split('\n')[0];
  expect(phrase.split('-').length).toBeGreaterThanOrEqual(6); // EFF words may themselves contain a hyphen (yo-yo)
  expect(phrase).toMatch(/^[a-z]+(-[a-z]+)+$/);
  // the words slider mirror renders with the descriptor bounds
  const slider = page.locator('input[type="range"][data-for="in-words"]');
  await expect(slider).toHaveAttribute('min', '2');
  await expect(slider).toHaveAttribute('max', '20');
});

test('diceware-passphrase page maps typed dice rolls deterministically (exact output)', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  await page.fill('#in-rolls', ROLLS);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('tiger-canal-dolphin-garlic-lantern-pebble', { timeout: 15000 });
  expect(await out.textContent()).toBe(ROLLS_TEXT);
});

test('diceware-passphrase deep-link prefills rolls and shows dice rolls', async ({ page }) => {
  await page.goto(
    '/tools/diceware-passphrase/?rolls=' + encodeURIComponent(ROLLS) + '&show_rolls=true',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Dice rolls:', { timeout: 15000 });
  expect(await out.textContent()).toBe(ROLLS_TEXT_WITH_DICE);
});

test('diceware-passphrase page supports the EFF short list (exact output)', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  await page.selectOption('#in-wordlist', 'eff-short');
  await page.fill('#in-rolls', '1111 6666');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('acid-zoom', { timeout: 15000 });
  expect(await out.textContent()).toBe(
    'acid-zoom\n\nEntropy: 20.7 bits — strength: weak\nRecipe: 2 words from the EFF short list (1,296 words, 10.3 bits/word)\nCrack time: under a second (offline, 10 billion guesses/sec)',
  );
});

test('diceware-passphrase page covers every separator choice', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  await page.fill('#in-rolls', '62315 14534');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('tiger-canal', { timeout: 15000 });

  await page.selectOption('#in-separator', 'space');
  await expect(out).toContainText('tiger canal');
  await page.selectOption('#in-separator', 'underscore');
  await expect(out).toContainText('tiger_canal');
  await page.selectOption('#in-separator', 'dot');
  await expect(out).toContainText('tiger.canal');
  await page.selectOption('#in-separator', 'none');
  await expect(out).toContainText('tigercanal');
  await page.selectOption('#in-separator', 'random-symbol');
  await expect(out).toContainText('Entropy: 29.4 bits');
  const text = (await out.textContent()) ?? '';
  expect(text.split('\n')[0]).toMatch(/^tiger[!@#$%^&*\-+=?]canal$/);
});

test('diceware-passphrase page non-default checkboxes: capitalize + digit + symbol', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  await page.fill('#in-rolls', '62315 14534');
  await page.check('#in-capitalize');
  await page.check('#in-add_number');
  await page.check('#in-add_symbol');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Entropy: 32.8 bits', { timeout: 15000 });
  const text = (await out.textContent()) ?? '';
  expect(text.split('\n')[0]).toMatch(/^Tiger-Canal\d[!@#$%^&*\-+=?]$/);
  await expect(out).toContainText('trailing digit (+3.3 bits)');
  await expect(out).toContainText('trailing symbol (+3.6 bits)');
});

test('diceware-passphrase page generates a batch (count=3)', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  await page.fill('#in-count', '3');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Entropy: 77.5 bits', { timeout: 15000 });
  const text = (await out.textContent()) ?? '';
  const phrases = text.split('\n\n')[0].split('\n');
  expect(phrases.length).toBe(3);
  expect(new Set(phrases).size).toBe(3);
});

test('diceware-passphrase page cap boundary: 20 words ok, 21 errors', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  await page.fill('#in-words', '20');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Recipe: 20 words from the EFF long list', { timeout: 15000 });
  await expect(out).toContainText('Entropy: 258.5 bits — strength: very strong');
  await page.fill('#in-words', '21');
  await expect(out).toHaveText('words must be between 2 and 20');
  await expect(out).toHaveClass(/error/);
});

test('diceware-passphrase example chip runs the physical-dice example', async ({ page }) => {
  await page.goto('/tools/diceware-passphrase/');
  await page.getByRole('button', { name: 'From physical dice rolls' }).click();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Dice rolls:', { timeout: 15000 });
  expect(await out.textContent()).toBe(ROLLS_TEXT_WITH_DICE);
});
