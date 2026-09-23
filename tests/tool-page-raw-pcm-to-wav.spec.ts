import { test, expect } from './fixtures';

const tool = '/tools/raw-pcm-to-wav/';

// The page's "CD-style stereo dump" chip: 8 stereo frames of 16-bit signed
// little-endian PCM as hex — 32 headerless bytes, no magic, no header.
const DEMO_HEX = '00000000001000f0002000e0003000d0004000c0005000b0006000a000700090';
const DEMO_B64 = Buffer.from(DEMO_HEX, 'hex').toString('base64');

// The same 16 samples read big-endian: every pair byte-swapped into WAVE order.
const DEMO_SWAPPED = '000000001000f0002000e0003000d0004000c0005000b0006000a00070009000';

async function setInput(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

// Call the wasm export directly for the advertised-values matrix — the same
// module the page driver loads, without driving ten controls per case.
async function runWasm(
  page: import('@playwright/test').Page,
  params: {
    input?: string;
    input_format?: string;
    sample_rate?: string;
    channels?: string;
    bit_depth?: string;
    encoding?: string;
    byte_order?: string;
    skip_bytes?: string;
    max_frames?: string;
    output?: string;
  } = {},
) {
  const p = {
    input: DEMO_HEX,
    input_format: 'auto',
    sample_rate: '44100',
    channels: '2',
    bit_depth: '16',
    encoding: 'signed',
    byte_order: 'little',
    skip_bytes: '0',
    max_frames: '0',
    output: 'hex',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/raw-pcm-to-wav/gizza_ai_raw_pcm_to_wav_web.js');
    await mod.default('/tools/raw-pcm-to-wav/gizza_ai_raw_pcm_to_wav_web_bg.wasm');
    return mod.run(
      args.input,
      args.input_format,
      args.sample_rate,
      args.channels,
      args.bit_depth,
      args.encoding,
      args.byte_order,
      args.skip_bytes,
      args.max_frames,
      args.output,
    );
  }, p);
}

test('raw-pcm-to-wav page wraps the CD-style demo dump in a playable WAV data URL', async ({
  page,
}) => {
  await page.goto(tool);
  await setInput(page, DEMO_HEX);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('data:audio/wav;base64,', { timeout: 15_000 });
  const url = (await out.textContent())!.trim();

  // Decode the data: URL in the browser and read the RIFF/WAVE header back.
  const header = await page.evaluate((u) => {
    const bin = atob(u.slice(u.indexOf(',') + 1));
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
    const view = new DataView(bytes.buffer);
    const ascii = (from: number, to: number) =>
      String.fromCharCode(...Array.from(bytes.slice(from, to)));
    return {
      length: bytes.length,
      riff: ascii(0, 4),
      riffSize: view.getUint32(4, true),
      wave: ascii(8, 12),
      fmt: ascii(12, 16),
      fmtSize: view.getUint32(16, true),
      formatTag: view.getUint16(20, true),
      channels: view.getUint16(22, true),
      sampleRate: view.getUint32(24, true),
      byteRate: view.getUint32(28, true),
      blockAlign: view.getUint16(32, true),
      bitsPerSample: view.getUint16(34, true),
      data: ascii(36, 40),
      dataSize: view.getUint32(40, true),
    };
  }, url);

  expect(header.riff).toBe('RIFF');
  expect(header.wave).toBe('WAVE');
  expect(header.fmt).toBe('fmt ');
  expect(header.data).toBe('data');
  expect(header.fmtSize).toBe(16);
  expect(header.formatTag).toBe(1); // WAVE_FORMAT_PCM
  expect(header.channels).toBe(2);
  expect(header.sampleRate).toBe(44100);
  expect(header.byteRate).toBe(176400);
  expect(header.blockAlign).toBe(4);
  expect(header.bitsPerSample).toBe(16);
  expect(header.dataSize).toBe(32);
  // 44-byte canonical header + the 32 sample bytes, copied verbatim.
  expect(header.length).toBe(76);
  expect(header.riffSize).toBe(68);
});

test('raw-pcm-to-wav deep link pre-fills an 8-bit mono description and reports the header', async ({
  page,
}) => {
  const qs = new URLSearchParams({
    input: '010203',
    output: 'info',
    sample_rate: '16000',
    channels: '1',
    bit_depth: '8',
    encoding: 'unsigned',
    byte_order: 'little',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('info', { timeout: 15_000 });
  await expect(page.locator('#in-sample_rate')).toHaveValue('16000');
  await expect(page.locator('#in-channels')).toHaveValue('1');
  await expect(page.locator('#in-bit_depth')).toHaveValue('8');
  await expect(page.locator('#in-encoding')).toHaveValue('unsigned');
  await expect(page.locator('#in-byte_order')).toHaveValue('little');
  await expect(page.locator('#in-input')).toHaveValue('010203');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('encoding         unsigned integer, 8-bit', { timeout: 15_000 });
  await expect(out).toContainText('frame size       1 bytes (1 ch x 1 byte)');
  await expect(out).toContainText('usable           3 bytes = 3 sample frames');
  await expect(out).toContainText('leftover         0 bytes (the data divides evenly into frames)');
  await expect(out).toContainText('note             byte_order is irrelevant for single-byte samples');
  await expect(out).toContainText('format tag       1 (WAVE_FORMAT_PCM)');
  await expect(out).toContainText('stored samples   unsigned 8-bit PCM (WAVE stores 8-bit as unsigned)');
  await expect(out).toContainText('data chunk       3 bytes');
  await expect(out).toContainText('ffmpeg           ffmpeg -f u8 -ar 16000 -ac 1 -i in.pcm out.wav');
  await expect(out).toContainText(
    'sox              sox -t raw -r 16000 -b 8 -e unsigned-integer -L -c 1 in.pcm out.wav',
  );
});

test('raw-pcm-to-wav wasm covers every advertised value, boundary and error', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  // Input forms: auto-detected hex, explicit base64 and a data: URI agree.
  const hexWav = await runWasm(page);
  expect(hexWav).toHaveLength((44 + 32) * 2);
  expect(hexWav.startsWith('52494646')).toBe(true); // "RIFF"
  expect(hexWav.slice(16, 24)).toBe('57415645'); // "WAVE"
  expect(hexWav.slice(24, 32)).toBe('666d7420'); // "fmt "
  expect(hexWav.slice(72, 80)).toBe('64617461'); // "data"
  expect(hexWav.slice(80, 88)).toBe('20000000'); // data chunk 32 bytes
  expect(hexWav.slice(88)).toBe(DEMO_HEX); // samples copied verbatim
  expect(await runWasm(page, { input: DEMO_B64, input_format: 'base64' })).toBe(hexWav);
  expect(
    await runWasm(page, { input: `data:application/octet-stream;base64,${DEMO_B64}` }),
  ).toBe(hexWav);

  // output=base64 and output=data_url render the very same file.
  const asB64 = await runWasm(page, { output: 'base64' });
  expect(Buffer.from(asB64, 'base64').toString('hex')).toBe(hexWav);
  expect(await runWasm(page, { output: 'data_url' })).toBe(`data:audio/wav;base64,${asB64}`);

  // byte_order=big byte-swaps every sample into the order WAVE requires.
  expect((await runWasm(page, { byte_order: 'big' })).slice(88)).toBe(DEMO_SWAPPED);

  // float and G.711 take the 18-byte fmt chunk plus a fact chunk.
  const f32 = await runWasm(page, {
    input: '0000803f',
    sample_rate: '48000',
    channels: '1',
    bit_depth: '32',
    encoding: 'float',
  });
  expect(f32.slice(32, 40)).toBe('12000000'); // fmt size 18
  expect(f32.slice(40, 44)).toBe('0300'); // WAVE_FORMAT_IEEE_FLOAT
  expect(f32.slice(76, 84)).toBe('66616374'); // "fact"
  expect(f32.slice(116)).toBe('0000803f');
  const ulaw = await runWasm(page, {
    input: 'ff7f0080',
    sample_rate: '8000',
    channels: '1',
    bit_depth: '8',
    encoding: 'mulaw',
  });
  expect(ulaw.slice(40, 44)).toBe('0700'); // WAVE_FORMAT_MULAW
  expect(ulaw.slice(116)).toBe('ff7f0080'); // companded bytes kept as-is
  const alaw = await runWasm(page, {
    input: 'ff7f0080',
    sample_rate: '8000',
    channels: '1',
    bit_depth: '8',
    encoding: 'alaw',
  });
  expect(alaw.slice(40, 44)).toBe('0600'); // WAVE_FORMAT_ALAW

  // Boundaries: the advertised maximum rate and channel count both wrap.
  const fast = await runWasm(page, { sample_rate: '768000', channels: '1' });
  expect(fast.slice(48, 56)).toBe('00b80b00'); // 768000 Hz
  const wide = await runWasm(page, { channels: '16' });
  expect(wide.slice(44, 48)).toBe('1000'); // 16 channels
  const info = await runWasm(page, { sample_rate: '768000', channels: '16', output: 'info' });
  expect(info).toContain('sample rate      768000 Hz');
  expect(info).toContain('channels         16');

  // The one pairing WAVE cannot express: a 16-bit IEEE float.
  await expect(runWasm(page, { bit_depth: '16', encoding: 'float' })).rejects.toThrow(
    /encoding=float needs bit_depth 32 or 64, got 16/,
  );
});

test('raw-pcm-to-wav page ships runnable CLI and preset chips', async ({ page }) => {
  await page.goto(tool);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool raw-pcm-to-wav');
  expect(cli).not.toContain('TODO');

  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'CD-style stereo dump',
    '8 kHz mu-law telephony',
    'Big-endian capture',
    '32-bit float dump',
    'Explain the header',
  ]);
});
