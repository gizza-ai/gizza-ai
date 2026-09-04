import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  action = 'auto',
  notes = '',
  root = 'any',
  key = 'C',
  scale = 'major',
  fit = 'contains',
  spelling = 'auto',
  includeChords = 'true',
  includeModes = 'true',
  chordType = 'triads',
  maxResults = '12',
  output = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/scale-chord-finder/gizza_ai_scale_chord_finder_web.js');
    await mod.default('/tools/scale-chord-finder/gizza_ai_scale_chord_finder_web_bg.wasm');
    return mod.run(
      args.action,
      args.notes,
      args.root,
      args.key,
      args.scale,
      args.fit,
      args.spelling,
      args.includeChords,
      args.includeModes,
      args.chordType,
      args.maxResults,
      args.output,
    );
  }, { action, notes, root, key, scale, fit, spelling, includeChords, includeModes, chordType, maxResults, output });
}

test('scale-chord-finder page renders exact default list output', async ({ page }) => {
  await page.goto('/tools/scale-chord-finder/');
  await expect(page.locator('#tool-output')).toContainText('Scale: C major (Major (Ionian))', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Triads:     C  Dm Em F  G  Am Bdim');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool scale-chord-finder');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('scale-chord-finder deep-link prefills controls and searches notes', async ({ page }) => {
  const params = new URLSearchParams({
    action: 'find',
    notes: 'C E G B',
    root: 'any',
    fit: 'contains',
    max_results: '5',
    output: 'names',
  });
  await page.goto(`/tools/scale-chord-finder/?${params.toString()}`);
  await expect(page.locator('#in-action')).toHaveValue('find', { timeout: 15_000 });
  await expect(page.locator('#in-notes')).toHaveValue('C E G B');
  await expect(page.locator('#in-fit')).toHaveValue('contains');
  await expect(page.locator('#in-output')).toHaveValue('names');
  await expect(page.locator('#tool-output')).toContainText('E hirajoshi', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).not.toContainText('TODO');
});

test('scale-chord-finder wasm covers enums, booleans, bounds and errors', async ({ page }) => {
  await page.goto('/tools/scale-chord-finder/');
  await page.waitForSelector('#in-action');

  expect(await runWasm(page, 'list', '', 'any', 'G', 'lydian', 'contains', 'auto', 'true', 'true', 'triads', '12', 'names')).toBe('G A B C# D E F#');
  expect(await runWasm(page, 'list', '', 'any', 'Db', 'minor', 'contains', 'flats', 'false', 'false', 'sevenths', '12', 'csv')).toContain('degree,note,semitones');
  const exact = await runWasm(page, 'find', 'C D E G A', 'any', 'C', 'major', 'exact', 'auto', 'true', 'false', 'both', '50', 'json');
  expect(exact).toContain('"fit":"exact"');
  expect(exact).toContain('major-pentatonic');
  expect(exact).toContain('"searched":504');

  const rooted = await runWasm(page, 'find', 'Eb G Bb', 'Eb', 'C', 'major', 'contains', 'auto', 'false', 'false', 'triads', '3', 'names');
  expect(rooted.split('\n').every((line: string) => line.startsWith('Eb '))).toBeTruthy();

  await expect(runWasm(page, 'find', 'H')).rejects.toThrow(/unknown note/);
});
