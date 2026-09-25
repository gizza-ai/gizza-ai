import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  player_a_rating = 1600,
  player_b_rating = 1500,
  score_a = 1,
  k_factor = 32,
  games = 1,
  k_factor_b = 0,
  max_rating_difference = 0,
  decimals = 0,
  output_format = 'summary',
  player_a_name = 'Player A',
  player_b_name = 'Player B',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/elo-rating-calculator/gizza_ai_elo_rating_calculator_web.js');
    await mod.default('/tools/elo-rating-calculator/gizza_ai_elo_rating_calculator_web_bg.wasm');
    return mod.run(
      args.player_a_rating,
      args.player_b_rating,
      args.score_a,
      args.k_factor,
      BigInt(args.games),
      args.k_factor_b,
      args.max_rating_difference,
      BigInt(args.decimals),
      args.output_format,
      args.player_a_name,
      args.player_b_name,
    );
  }, { player_a_rating, player_b_rating, score_a, k_factor, games, k_factor_b, max_rating_difference, decimals, output_format, player_a_name, player_b_name });
}

test('elo-rating-calculator page renders the default favourite win', async ({ page }) => {
  await page.goto('/tools/elo-rating-calculator/');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Player A: 1600 -> 1612 (+12)', { timeout: 15_000 });
  await expect(out).toContainText('Player B: 1500 -> 1488 (-12)');
  await expect(out).toContainText('Expected: Player A 0.640065 (64.01%), Player B 0.359935 (35.99%)');
  await expect(out).toContainText('Score 0.5 (draw): 1600 -> 1596 (-4)');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool elo-rating-calculator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('elo-rating-calculator deep-link returns delta output for a draw', async ({ page }) => {
  const params = new URLSearchParams({
    player_a_rating: '1600',
    player_b_rating: '1500',
    score_a: '0.5',
    k_factor: '32',
    output_format: 'delta',
  });
  await page.goto(`/tools/elo-rating-calculator/?${params.toString()}`);
  await expect(page.locator('#in-score_a')).toHaveValue('0.5', { timeout: 15_000 });
  await expect(page.locator('#in-output_format')).toHaveValue('delta');
  await expect(page.locator('#tool-output')).toHaveText('-4', { timeout: 15_000 });
});

test('elo-rating-calculator wasm covers formats, caps, series and errors', async ({ page }) => {
  await page.goto('/tools/elo-rating-calculator/');
  await page.waitForSelector('#in-player_a_rating');

  await expect(runWasm(page, 1500, 1600, 1, 32, 1, 0, 0, 0, 'delta')).resolves.toBe('+20');
  await expect(runWasm(page, 1600, 1500, 1, 10, 1, 40, 0, 0, 'csv')).resolves.toContain('Player B,1500,0.359935,0,40,-14,1486');
  await expect(runWasm(page, 2400, 1400, 1, 20, 1, 0, 400, 0, 'json')).resolves.toContain('"rating_difference_capped": true');
  await expect(runWasm(page, 1600, 1500, 3.5, 32, 5, 0, 0, 0, 'summary')).resolves.toContain('Match: 5 games, Player A scored 3.5, Player B scored 1.5');

  await expect(runWasm(page, 9000, 1500, 1)).rejects.toThrow(/between 0 and 5000/);
  await expect(runWasm(page, 1500, 1500, 2)).rejects.toThrow(/score_a must be between 0 and 1/);
});
