import { test, expect } from './fixtures';

const DEAL_2 = `Deal — 2 players × 5 cards · 1 deck (52 cards) · seed "42"

Player 1: AS JH 4H 3S 3D — Pair of 3s
Player 2: QH TH TD TC 5C — Three of a kind, 10s

Remaining in deck: 42`;

async function runWasm(
  page: any,
  mode = 'deal',
  players = '2',
  cardsPerPlayer = '5',
  count = '5',
  decks = '1',
  jokers = '0',
  seed = '42',
  notation = 'short',
  replacement = 'false',
  sortHands = 'true',
  evaluate = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/card-deck-tools/gizza_ai_card_deck_tools_web.js');
    await mod.default('/tools/card-deck-tools/gizza_ai_card_deck_tools_web_bg.wasm');
    return mod.run(
      args.mode,
      args.players,
      args.cardsPerPlayer,
      args.count,
      args.decks,
      args.jokers,
      args.seed,
      args.notation,
      args.replacement,
      args.sortHands,
      args.evaluate,
    );
  }, { mode, players, cardsPerPlayer, count, decks, jokers, seed, notation, replacement, sortHands, evaluate });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('card-deck-tools wasm produces reproducible deals, draws, replacement and notation variants', async ({ page }) => {
  await page.goto('/tools/card-deck-tools/');
  await page.waitForSelector('#in-mode');

  await expect(runWasm(page)).resolves.toBe(DEAL_2);
  await expect(runWasm(page, 'draw', '4', '5', '3', '1', '0', '42', 'long', 'false', 'false', 'false'))
    .resolves.toContain('Four of Hearts, Ten of Hearts, Ace of Spades');
  await expect(runWasm(page, 'draw', '4', '5', '60', '1', '0', '42', 'short', 'true', 'false', 'false'))
    .resolves.toContain('Remaining in deck: 52 (drawn with replacement)');
  await expect(runWasm(page, 'shuffle', '4', '5', '5', '2', '1', 'shoe-1', 'symbol', 'false', 'false', 'false'))
    .resolves.toContain('2 decks, 1 joker each (106 cards)');
});

test('card-deck-tools page renders exact deal and CLI example', async ({ page }) => {
  const params = new URLSearchParams({
    mode: 'deal',
    players: '2',
    cards_per_player: '5',
    count: '5',
    decks: '1',
    jokers: '0',
    seed: '42',
    notation: 'short',
    replacement: 'false',
    sort_hands: 'true',
    evaluate: 'true',
  });
  await page.goto(`/tools/card-deck-tools/?${params.toString()}`);

  await expect(page.locator('#in-mode')).toHaveValue('deal', { timeout: 15_000 });
  await expect(page.locator('#in-players')).toHaveValue('2');
  await expect(page.locator('#in-sort_hands')).toBeChecked();
  await expect(page.locator('#in-evaluate')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(DEAL_2, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool card-deck-tools');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('card-deck-tools deep-link draws long cards without replacement', async ({ page }) => {
  const params = new URLSearchParams({
    mode: 'draw',
    players: '4',
    cards_per_player: '5',
    count: '3',
    decks: '1',
    jokers: '0',
    seed: '42',
    notation: 'long',
    replacement: 'false',
    sort_hands: 'false',
    evaluate: 'false',
  });

  await page.goto(`/tools/card-deck-tools/?${params.toString()}`);
  await expect(page.locator('#in-mode')).toHaveValue('draw', { timeout: 15_000 });
  await expect(page.locator('#in-count')).toHaveValue('3');
  await expect(page.locator('#in-notation')).toHaveValue('long');
  await expect(page.locator('#tool-output')).toContainText('Four of Hearts, Ten of Hearts, Ace of Spades', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Remaining in deck: 49');
  expect(await outputText(page)).toContain('Draw — 3 cards');
});
