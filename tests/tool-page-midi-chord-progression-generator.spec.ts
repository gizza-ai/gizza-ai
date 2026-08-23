import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  progression: string,
  tempo = '120',
  beatsPerChord = '4',
  beatsPerBar = '4',
  octave = '4',
  voicing = 'close',
  inversion = 'root',
  pattern = 'block',
  arpNote = 'eighth',
  noteLength = '95',
  addBass = 'false',
  transpose = '0',
  velocity = '96',
  instrument = 'acoustic-grand-piano',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/midi-chord-progression-generator/gizza_ai_midi_chord_progression_generator_web.js');
    await mod.default('/tools/midi-chord-progression-generator/gizza_ai_midi_chord_progression_generator_web_bg.wasm');
    return mod.run(
      args.progression,
      args.tempo,
      args.beatsPerChord,
      args.beatsPerBar,
      args.octave,
      args.voicing,
      args.inversion,
      args.pattern,
      args.arpNote,
      args.noteLength,
      args.addBass,
      args.transpose,
      args.velocity,
      args.instrument,
    );
  }, {
    progression,
    tempo,
    beatsPerChord,
    beatsPerBar,
    octave,
    voicing,
    inversion,
    pattern,
    arpNote,
    noteLength,
    addBass,
    transpose,
    velocity,
    instrument,
  });
}

function parsePayload(raw: string) {
  const payload = JSON.parse(raw);
  expect(payload.data_url).toContain('data:audio/midi;base64,');
  const b64 = payload.data_url.split(',')[1];
  const bytes = Buffer.from(b64, 'base64');
  expect(bytes.subarray(0, 4).toString('ascii')).toBe('MThd');
  return { payload, bytes };
}

test('midi-chord-progression-generator page renders a downloadable MIDI summary', async ({ page }) => {
  await page.goto('/tools/midi-chord-progression-generator/');
  await setTextarea(page, '#in-progression', 'C G Am F');

  await expect(page.locator('#tool-output')).toContainText('4 chord(s)', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('C — C4 E4 G4');
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'chord-progression.mid');
});

test('midi-chord-progression-generator deep-link prefills controls and strums slash chords', async ({ page }) => {
  const params = new URLSearchParams({
    progression: 'C/E:2 F:2 Gsus4:1 G:1 C:4',
    tempo: '110',
    beats_per_chord: '4',
    beats_per_bar: '4',
    octave: '4',
    voicing: 'spread',
    inversion: 'smooth',
    pattern: 'strum',
    arp_note: 'eighth',
    note_length: '80',
    add_bass: 'true',
    transpose: '0',
    velocity: '96',
    instrument: 'acoustic-guitar-steel',
  });

  await page.goto(`/tools/midi-chord-progression-generator/?${params.toString()}`);
  await expect(page.locator('#in-progression')).toHaveValue('C/E:2 F:2 Gsus4:1 G:1 C:4', { timeout: 15_000 });
  await expect(page.locator('#in-voicing')).toHaveValue('spread');
  await expect(page.locator('#in-inversion')).toHaveValue('smooth');
  await expect(page.locator('#in-pattern')).toHaveValue('strum');
  await expect(page.locator('#in-add_bass')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('5 chord(s)', { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool midi-chord-progression-generator');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('midi-chord-progression-generator wasm covers enum choices and output bytes', async ({ page }) => {
  await page.goto('/tools/midi-chord-progression-generator/');
  await page.waitForSelector('#in-progression');

  for (const voicing of ['close', 'drop-2', 'drop-3', 'spread']) {
    const { payload, bytes } = parsePayload(await runWasm(page, 'Cmaj7 Dm7 G7 Cmaj7', '120', '2', '4', '4', voicing));
    expect(payload.summary).toContain('4 chord(s)');
    expect(bytes.length).toBeGreaterThan(120);
  }
  for (const inversion of ['root', 'first', 'second', 'third', 'smooth']) {
    const { payload } = parsePayload(await runWasm(page, 'Cmaj7 Fmaj7 G7 Cmaj7', '120', '2', '4', '4', 'close', inversion));
    expect(payload.detail).toContain('Cmaj7');
  }
  for (const pattern of ['block', 'arpeggio-up', 'arpeggio-down', 'arpeggio-updown', 'strum']) {
    const { payload } = parsePayload(await runWasm(page, 'C G', '100', '2', '4', '4', 'close', 'root', pattern));
    expect(payload.notes).toBeGreaterThanOrEqual(6);
  }
  for (const arpNote of ['whole', 'half', 'quarter', 'eighth', 'sixteenth', 'thirty-second']) {
    const { payload } = parsePayload(await runWasm(page, 'Am F', '90', '4', '4', '4', 'close', 'root', 'arpeggio-up', arpNote));
    expect(payload.filename).toBe('chord-progression.mid');
  }
});

test('midi-chord-progression-generator covers bounds, checkbox and errors', async ({ page }) => {
  await page.goto('/tools/midi-chord-progression-generator/');
  await page.waitForSelector('#in-progression');

  const { payload } = parsePayload(await runWasm(page, 'C', '20', '0.25', '1', '4', 'spread', 'root', 'strum', 'eighth', '5', 'true', '-24', '1', 'synth-pad-warm'));
  expect(payload.summary).toContain('1 chord(s)');
  expect(payload.lowest).toBeTruthy();

  await expect(runWasm(page, 'H', '120')).rejects.toThrow(/not a note name/);
  await expect(runWasm(page, 'C', '401')).rejects.toThrow(/tempo must be between/);
});
