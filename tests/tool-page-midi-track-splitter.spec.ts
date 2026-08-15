import { test, expect, type Page } from './fixtures';

const tool = '/tools/midi-track-splitter/';

const THREE_TRACK_HEX =
  '4d546864000000060001000300604d54726b0000002000ff0309436f6e647563746f7200ff510307a12000ff58040402180800ff2f004d54726b0000002000ff03055069616e6f00c00000903c4060903c00009040466090400000ff2f004d54726b0000001800ff03044261737300c12100912464814091240000ff2f00';
const THREE_TRACK_B64 =
  'TVRoZAAAAAYAAQADAGBNVHJrAAAAIAD/AwlDb25kdWN0b3IA/1EDB6EgAP9YBAQCGAgA/y8ATVRyawAAACAA/wMFUGlhbm8AwAAAkDxAYJA8AACQQEZgkEAAAP8vAE1UcmsAAAAYAP8DBEJhc3MAwSEAkSRkgUCRJAAA/y8A';
const FORMAT0_HEX =
  '4d546864000000060000000100604d54726b0000002600ff0304536f6e6700ff5103061a8000c0040090435a0099246e609043000099240000ff2f00';

async function outputText(page: Page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

async function firstDownloadMidi(page: Page) {
  return page.evaluate(async () => {
    const a = document.querySelector('#tool-output a.split-download') as HTMLAnchorElement | null;
    const href = a?.href || '';
    if (!href.startsWith('data:audio/midi;base64,')) return { error: 'no MIDI data URL: ' + href };
    const buf = new Uint8Array(await (await fetch(href)).arrayBuffer());
    const text = new TextDecoder('latin1').decode(buf);
    const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    const tempos: number[] = [];
    for (let i = 0; i + 5 < buf.length; i += 1) {
      if (buf[i] === 0xff && buf[i + 1] === 0x51 && buf[i + 2] === 0x03) {
        tempos.push((buf[i + 3] << 16) | (buf[i + 4] << 8) | buf[i + 5]);
      }
    }
    return {
      download: a?.getAttribute('download'),
      magic: text.slice(0, 4),
      tracks: dv.getUint16(10, false),
      ppq: dv.getUint16(12, false) & 0x7fff,
      tempos,
      hasPiano: text.includes('Piano'),
    };
  });
}

test('midi-track-splitter splits a format-1 file by track into real MIDI downloads', async ({
  page,
}) => {
  await page.goto(tool);
  await page.fill('#in-input', THREE_TRACK_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-split_by', 'track');

  await expect(page.locator('#tool-output')).toContainText('2 single-part file(s)', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('Piano');
  await expect(page.locator('#tool-output')).toContainText('Bass');
  await expect(page.locator('#tool-output a.split-download')).toHaveCount(2);

  const midi = await firstDownloadMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.download).toBe('part-02-piano.mid');
  expect(midi.magic).toBe('MThd');
  expect(midi.tracks).toBe(1);
  expect(midi.ppq).toBe(96);
  expect(midi.tempos).toContain(500000);
  expect(midi.hasPiano).toBe(true);
});

test('midi-track-splitter supports base64 auto-detect, list output, and an unchecked checkbox', async ({
  page,
}) => {
  await page.goto(tool);
  await page.fill('#in-input', THREE_TRACK_B64);
  await page.selectOption('#in-encoding', 'auto');
  await expect(page.locator('#in-skip_empty')).toBeChecked();
  await page.uncheck('#in-skip_empty');
  await page.selectOption('#in-output', 'list');

  await expect(page.locator('#tool-output')).toContainText('3 single-part file(s)', {
    timeout: 15_000,
  });
  expect(await outputText(page)).toContain('Listing only — no file bytes were produced.');
  await expect(page.locator('#tool-output a.split-download')).toHaveCount(0);
  await expect(page.locator('#tool-output')).toContainText('Conductor');
});

test('midi-track-splitter can split by MIDI channel and write format-1 parts', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', THREE_TRACK_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-split_by', 'channel');
  await page.selectOption('#in-output_format', 'format-1');

  await expect(page.locator('#tool-output')).toContainText('one per channel', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('channel 1');
  await expect(page.locator('#tool-output')).toContainText('channel 2');
  const midi = await firstDownloadMidi(page);
  expect(midi.error).toBeUndefined();
  expect(midi.tracks).toBe(2);
  expect(midi.tempos).toContain(500000);
});

test('midi-track-splitter deep link prefills, auto-runs, and selects one part', async ({ page }) => {
  await page.goto(
    tool +
      '?input=' +
      encodeURIComponent(THREE_TRACK_HEX) +
      '&encoding=hex&split_by=track&include_conductor=true&output_format=format-0&skip_empty=true&select=3&filename_prefix=stem&output=files',
  );

  await expect(page.locator('#in-input')).toHaveValue(THREE_TRACK_HEX, { timeout: 15_000 });
  await expect(page.locator('#in-encoding')).toHaveValue('hex');
  await expect(page.locator('#in-split_by')).toHaveValue('track');
  await expect(page.locator('#in-select')).toHaveValue('3');
  await expect(page.locator('#in-filename_prefix')).toHaveValue('stem');
  await expect(page.locator('#tool-output')).toContainText('1 single-part file(s)', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('Bass');
  await expect(page.locator('#tool-output a.split-download')).toHaveAttribute(
    'download',
    'stem-03-bass.mid',
  );
});

test('midi-track-splitter automatically cuts format-0 input by channel', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', FORMAT0_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-split_by', 'track');

  await expect(page.locator('#tool-output')).toContainText('Format 0 file with a single track', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('channel 1');
  await expect(page.locator('#tool-output')).toContainText('channel 10');
});

test('midi-track-splitter reports an out-of-range selection as a useful error', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', THREE_TRACK_HEX);
  await page.selectOption('#in-encoding', 'hex');
  await page.fill('#in-select', '64');

  await expect(page.locator('#tool-output')).toContainText('selection', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('matched none');
  await expect(page.locator('#tool-output a.split-download')).toHaveCount(0);
});
