import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  genre = 'rock',
  timeSignature = '4/4',
  bars = '2',
  tempo = '0',
  complexity = 'standard',
  hatSubdivision = 'auto',
  swing = '0',
  humanize = '0',
  fillEvery = '0',
  velocity = '100',
  kit = 'standard',
  seed = '1',
  preview = 'drums',
  output = 'report',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/drum-pattern-generator/gizza_ai_drum_pattern_generator_web.js');
    await mod.default('/tools/drum-pattern-generator/gizza_ai_drum_pattern_generator_web_bg.wasm');
    return mod.run(
      args.genre,
      args.timeSignature,
      args.bars,
      args.tempo,
      args.complexity,
      args.hatSubdivision,
      args.swing,
      args.humanize,
      args.fillEvery,
      args.velocity,
      args.kit,
      args.seed,
      args.preview,
      args.output,
    );
  }, { genre, timeSignature, bars, tempo, complexity, hatSubdivision, swing, humanize, fillEvery, velocity, kit, seed, preview, output });
}

test('drum-pattern-generator page renders default rock report with real grid output', async ({ page }) => {
  await page.goto('/tools/drum-pattern-generator/');
  await expect(page.locator('#tool-output')).toContainText('Rock pattern in 4/4', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Kick');
  await expect(page.locator('#tool-output')).toContainText('Snare');
  await expect(page.locator('#tool-output')).toContainText('Preview:');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool drum-pattern-generator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('drum-pattern-generator deep-link prefills controls and renders a trap MIDI artifact', async ({ page }) => {
  const params = new URLSearchParams({
    genre: 'trap',
    complexity: 'busy',
    humanize: '15',
    seed: '42',
    preview: 'off',
    output: 'midi-base64',
  });
  await page.goto(`/tools/drum-pattern-generator/?${params.toString()}`);
  await expect(page.locator('#in-genre')).toHaveValue('trap', { timeout: 15_000 });
  await expect(page.locator('#in-complexity')).toHaveValue('busy');
  await expect(page.locator('#in-humanize')).toHaveValue('15');
  await expect(page.locator('#in-seed')).toHaveValue('42');
  await expect(page.locator('#in-preview')).toHaveValue('off');
  await expect(page.locator('#in-output')).toHaveValue('midi-base64');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('TVRoZAAAAAYAAAABAe', { timeout: 15_000 });
  await expect(out).not.toContainText('TODO');
});

test('drum-pattern-generator wasm covers enums, bounds, base64 headers and errors', async ({ page }) => {
  await page.goto('/tools/drum-pattern-generator/');
  await page.waitForSelector('#in-genre');

  expect(await runWasm(page, 'jazz-swing', '4/4', '1', '140', 'standard', 'auto', '55', '0', '0', '100', 'jazz', '1', 'off', 'grid')).toContain('Ride');
  expect(await runWasm(page, 'waltz', '3/4', '1', '90', 'standard', 'eighth', '0', '0', '0', '100', 'brush', '1', 'off', 'json')).toContain('"time_signature":"3/4"');
  expect(await runWasm(page, 'trap', '4/4', '1', '140', 'busy', 'sixteenth', '0', '10', '0', '90', 'tr808', '7', 'off', 'midi-base64')).toContain('TVRoZAAAAAYAAAABAe');
  expect(await runWasm(page, 'rock', '4/4', '1', '100', 'standard', 'auto', '0', '0', '0', '100', 'standard', '1', 'click', 'wav-base64')).toContain('UklGR');

  await expect(runWasm(page, 'rock', '7/8', '1', '100', 'standard', 'quarter')).rejects.toThrow(/cannot be divided evenly/);
});
