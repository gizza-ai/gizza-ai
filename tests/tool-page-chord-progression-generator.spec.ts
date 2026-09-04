import { test, expect } from './fixtures';
import { Buffer } from 'node:buffer';

async function runWasm(
  page: any,
  key = 'C',
  mode = 'major',
  style = 'pop',
  variation = '1',
  sevenths = 'auto',
  borrowed = 'none',
  chords = '0',
  tempo = '100',
  instrument = 'acoustic-grand-piano',
  pattern = 'block',
  voiceLeading = 'true',
  repeats = '1',
  octave = '4',
  output = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/chord-progression-generator/gizza_ai_chord_progression_generator_web.js');
    await mod.default('/tools/chord-progression-generator/gizza_ai_chord_progression_generator_web_bg.wasm');
    return mod.run(
      args.key,
      args.mode,
      args.style,
      args.variation,
      args.sevenths,
      args.borrowed,
      args.chords,
      args.tempo,
      args.instrument,
      args.pattern,
      args.voiceLeading,
      args.repeats,
      args.octave,
      args.output,
    );
  }, { key, mode, style, variation, sevenths, borrowed, chords, tempo, instrument, pattern, voiceLeading, repeats, octave, output });
}

test('chord-progression-generator page renders exact default progression', async ({ page }) => {
  await page.goto('/tools/chord-progression-generator/');
  await page.locator('#in-output').selectOption('chords');
  await expect(page.locator('#tool-output')).toContainText('C G Am F', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool chord-progression-generator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('chord-progression-generator deep-link prefills controls', async ({ page }) => {
  const params = new URLSearchParams({
    key: 'Eb',
    mode: 'minor',
    style: 'rock',
    variation: '3',
    borrowed: 'rich',
    chords: '6',
    output: 'chords',
  });
  await page.goto(`/tools/chord-progression-generator/?${params.toString()}`);
  await expect(page.locator('#in-key')).toHaveValue('Eb', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('minor');
  await expect(page.locator('#in-style')).toHaveValue('rock');
  await expect(page.locator('#in-borrowed')).toHaveValue('rich');
  await expect(page.locator('#in-output')).toHaveValue('chords');
  await expect(page.locator('#tool-output')).not.toContainText('TODO', { timeout: 15_000 });
});

test('chord-progression-generator wasm covers enums, bounds, checkbox and midi bytes', async ({ page }) => {
  await page.goto('/tools/chord-progression-generator/');
  await page.waitForSelector('#in-key');

  expect(await runWasm(page, 'C', 'major', 'pop', '1', 'auto', 'none', '0', '100', 'acoustic-grand-piano', 'block', 'true', '1', '4', 'roman')).toBe('I V vi IV');
  expect(await runWasm(page, 'Eb', 'minor', 'jazz', '2', 'sevenths', 'none', '4', '100', 'electric-piano', 'strum', 'false', '1', '4', 'chords')).toBe('Ebm7 Cbmaj7 Fm7b5 Bbm7');
  expect(await runWasm(page, 'F', 'lydian', 'cinematic', '99', 'extended', 'rich', '32', '300', 'synth-pad-warm', 'arpeggio-updown', 'true', '8', '7', 'csv')).toContain('bar,roman,chord,notes');

  const midi = await runWasm(page, 'F', 'major', 'jazz', '1', 'extended', 'none', '4', '92', 'electric-piano', 'arpeggio-updown', 'true', '2', '4', 'midi-base64');
  const bytes = Buffer.from(midi, 'base64');
  expect(bytes.subarray(0, 4).toString('ascii')).toBe('MThd');
  expect(bytes.length).toBeGreaterThan(100);

  await expect(runWasm(page, 'Q')).rejects.toThrow(/unknown key/);
});
