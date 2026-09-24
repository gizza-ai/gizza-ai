import { test, expect } from './fixtures';

const FOUR_TEAM_TEXT = `Spring Cup
Single elimination · 4 participants · 4-slot bracket · 0 byes · 2 rounds · 4 matches
Seeds: 1 Lions · 2 Tigers · 3 Bears · 4 Sharks

Round 1 — Semifinals
  M1  Lions (1) vs Sharks (4)
  M2  Tigers (2) vs Bears (3)

Round 2 — Final
  M3  Winner of M1 vs Winner of M2

THIRD-PLACE MATCH

  M4  Loser of M1 vs Loser of M2`;

async function runWasm(
  page: any,
  participants = 'Lions\nTigers\nBears\nSharks',
  bracketType = 'single',
  seeding = 'standard',
  outputFormat = 'text',
  thirdPlaceMatch = 'true',
  grandFinalReset = 'true',
  tournamentName = 'Spring Cup',
  includeSummary = 'true',
  seed = '0',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/tournament-bracket-generator/gizza_ai_tournament_bracket_generator_web.js');
    await mod.default('/tools/tournament-bracket-generator/gizza_ai_tournament_bracket_generator_web_bg.wasm');
    return mod.run(
      args.participants,
      args.bracketType,
      args.seeding,
      args.outputFormat,
      args.thirdPlaceMatch,
      args.grandFinalReset,
      args.tournamentName,
      args.includeSummary,
      args.seed,
    );
  }, { participants, bracketType, seeding, outputFormat, thirdPlaceMatch, grandFinalReset, tournamentName, includeSummary, seed });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('tournament-bracket-generator wasm returns exact single-elimination text', async ({ page }) => {
  await page.goto('/tools/tournament-bracket-generator/');
  await page.waitForSelector('#in-participants');

  await expect(runWasm(page)).resolves.toBe(FOUR_TEAM_TEXT);
});

test('tournament-bracket-generator wasm covers double elimination and count input', async ({ page }) => {
  await page.goto('/tools/tournament-bracket-generator/');
  await page.waitForSelector('#in-participants');

  const out = await runWasm(page, '8', 'double', 'random', 'text', 'false', 'false', '', 'true', '7');
  expect(out).toContain('Double elimination · 8 participants · 8-slot bracket · 0 byes');
  expect(out).toContain('WINNERS BRACKET');
  expect(out).toContain('LOSERS BRACKET');
  expect(out).toContain('GRAND FINAL');
  expect(out).not.toContain('Grand Final Reset');
});

test('tournament-bracket-generator page renders exact output and honors non-default checkbox', async ({ page }) => {
  await page.goto('/tools/tournament-bracket-generator/');
  await page.fill('#in-participants', 'Lions\nTigers\nBears\nSharks');
  await page.selectOption('#in-bracket_type', 'single');
  await page.selectOption('#in-seeding', 'standard');
  await page.selectOption('#in-output_format', 'text');
  await page.check('#in-third_place_match');
  await page.fill('#in-tournament_name', 'Spring Cup');

  await expect(page.locator('#tool-output')).toHaveText(FOUR_TEAM_TEXT, { timeout: 15_000 });
});

test('tournament-bracket-generator deep-link prefills and runs CSV output', async ({ page }) => {
  const params = new URLSearchParams({
    participants: 'Lions\nTigers\nBears\nSharks',
    bracket_type: 'single',
    seeding: 'standard',
    output_format: 'csv',
    third_place_match: 'false',
    grand_final_reset: 'true',
    tournament_name: '',
    include_summary: 'true',
    seed: '0',
  });

  await page.goto(`/tools/tournament-bracket-generator/?${params.toString()}`);
  await expect(page.locator('#in-participants')).toHaveValue('Lions\nTigers\nBears\nSharks', { timeout: 15_000 });
  await expect(page.locator('#in-output_format')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('match,bracket,round,round_name,side_a,side_a_seed,side_b,side_b_seed,status', { timeout: 15_000 });
  expect(await outputText(page)).toContain('M1,main,1,Semifinals,Lions,1,Sharks,4,match');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool tournament-bracket-generator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
