import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-watermark-tile/ page stamps a REPEATING watermark
// over the whole uploaded image in-browser via ffmpeg (@ffmpeg/core from a CDN —
// needs network). The wasm `build_argv` is pure and shared with the chat block
// through core, so the pattern maths (tile count, rotation, opacity scaling,
// output container) is asserted directly on the plan the way the nearby ffmpeg
// page specs do — no CDN round trip needed for the advertised-values matrix.
//
// Fixture white-64x64.png is solid white (every pixel 255,255,255), so a BLACK
// watermark at full opacity is unambiguous: any dark pixel in the output proves
// the tiles were really drawn, and the untouched corner proves the rest of the
// image survived.
const WHITE = path.resolve(__dirname, 'fixtures/white-64x64.png');

type Plan = { argv: string[]; out_name: string; inputs: [string, string][] };

async function buildArgv(
  page,
  text: string,
  fontSize: number,
  color: string,
  opacity: number,
  angle: number,
  columns: number,
  rows: number,
  pattern: string,
  outline: string,
  format: string,
  inName: string,
): Promise<Plan> {
  return await page.evaluate(
    async (a) => {
      const mod = await import('/tools/image-watermark-tile/gizza_ai_image_watermark_tile_web.js');
      await mod.default('/tools/image-watermark-tile/gizza_ai_image_watermark_tile_web_bg.wasm');
      return mod.build_argv(
        a.text, a.fontSize, a.color, a.opacity, a.angle, a.columns, a.rows,
        a.pattern, a.outline, a.format, a.inName,
      );
    },
    { text, fontSize, color, opacity, angle, columns, rows, pattern, outline, format, inName },
  );
}

// build_argv rejects invalid input; capture the message instead of failing the
// evaluate so the error path can be asserted like any other value.
async function buildArgvError(page, text: string, fontSize: number, opacity: number, columns: number) {
  return await page.evaluate(
    async (a) => {
      const mod = await import('/tools/image-watermark-tile/gizza_ai_image_watermark_tile_web.js');
      await mod.default('/tools/image-watermark-tile/gizza_ai_image_watermark_tile_web_bg.wasm');
      try {
        mod.build_argv(a.text, a.fontSize, '#ffffff', a.opacity, 0, a.columns, 4, 'grid', 'false', 'keep', 'in.png');
        return 'NO ERROR';
      } catch (e) {
        return String(e);
      }
    },
    { text, fontSize, opacity, columns },
  );
}

const filterOf = (plan: Plan) => plan.argv[plan.argv.indexOf('-filter_complex') + 1];
const countDrawtext = (filter: string) => filter.split('drawtext=').length - 1;

// 1) Real end-to-end run: the tiled watermark actually reaches the pixels.
//    Black text at full opacity on a solid-white 64×64 PNG — dimensions are
//    preserved, dark glyph pixels exist, and the corner between tiles stays white.
test('image-watermark-tile page tiles a real watermark onto the image', async ({ page }) => {
  await page.goto('/tools/image-watermark-tile/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-text', 'X');
  await page.fill('#in-font_size', '14');
  await page.fill('#in-color', '#000000');
  await page.fill('#in-opacity', '1');
  await page.fill('#in-angle', '0');
  await page.fill('#in-columns', '2');
  await page.fill('#in-rows', '2');
  await page.selectOption('#in-pattern', 'grid');
  await page.setInputFiles('#in-file', WHITE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);

  const stats = await page.evaluate(async (dataUrl) => {
    const img = new Image();
    await new Promise((res, rej) => {
      img.onload = res;
      img.onerror = rej;
      img.src = dataUrl;
    });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    let dark = 0;
    let min = 255;
    for (let i = 0; i < d.length; i += 4) {
      if (d[i] < 100) dark += 1;
      if (d[i] < min) min = d[i];
    }
    const corner = ctx.getImageData(0, 0, 1, 1).data;
    return { w: img.naturalWidth, h: img.naturalHeight, dark, min, corner: corner[0] };
  }, src!);

  expect(stats.w).toBe(64);
  expect(stats.h).toBe(64);
  // Four tiles of black text at opacity 1 → plenty of near-black pixels.
  expect(stats.dark).toBeGreaterThan(20);
  expect(stats.min).toBeLessThan(40);
  // Between the tiles the source image is untouched (overlay's 8-bit alpha
  // rounding leaves 254 rather than a clean 255).
  expect(stats.corner).toBeGreaterThanOrEqual(250);
});

// 2) Deep link: ?query pre-fills every control, including the non-default enum
//    values, the NON-default checkbox state, and angle=0 (a meaningful value that
//    must not be treated as "unset").
test('image-watermark-tile page honors a ?text&angle=0&pattern=grid&outline=true&format=png deep link', async ({ page }) => {
  await page.goto(
    '/tools/image-watermark-tile/?text=DRAFT&font_size=24&color=%23c00000&opacity=0.5' +
      '&angle=0&columns=3&rows=6&pattern=grid&outline=true&format=png',
  );
  await page.waitForSelector('#in-file');

  await expect(page.locator('#in-text')).toHaveValue('DRAFT');
  await expect(page.locator('#in-font_size')).toHaveValue('24');
  await expect(page.locator('#in-color')).toHaveValue('#c00000');
  await expect(page.locator('#in-opacity')).toHaveValue('0.5');
  await expect(page.locator('#in-angle')).toHaveValue('0');
  await expect(page.locator('#in-columns')).toHaveValue('3');
  await expect(page.locator('#in-rows')).toHaveValue('6');
  await expect(page.locator('#in-pattern')).toHaveValue('grid');
  await expect(page.locator('#in-outline')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('png');
});

// 3) The default-ish plan, asserted exactly: transparent layer → opaque glyphs →
//    one alpha scale → re-centred overlay, with the text and font supplied as
//    virtual-FS files rather than interpolated into the filtergraph.
test('image-watermark-tile wasm build_argv plans the default tiled watermark', async ({ page }) => {
  await page.goto('/tools/image-watermark-tile/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 'SAMPLE', 32, '#ffffff', 0.3, 30, 4, 5, 'brick', 'false', 'keep', 'in.png');

  expect(plan.out_name).toBe('out.png');
  expect(plan.argv.slice(0, 2)).toEqual(['-i', 'in.png']);
  expect(plan.argv[plan.argv.length - 2]).toBe('-y');
  expect(plan.argv[plan.argv.length - 1]).toBe('out.png');

  const f = filterOf(plan);
  // Text/font come from files — the user's string never enters the graph, and
  // the whole graph stays one space-free argv token.
  expect(f).toContain('textfile=watermark.txt');
  expect(f).toContain('fontfile=font.ttf');
  expect(f).not.toContain('SAMPLE');
  expect(f).not.toContain(' ');
  // Transparent same-size layer, opaque white glyphs, single alpha scale.
  expect(f).toContain('[0:v]format=rgba,split=2[wmbase][wmlay]');
  expect(f).toContain('colorchannelmixer=rr=0:gg=0:bb=0:aa=0');
  expect(f).toContain('fontcolor=0xFFFFFF:x=');
  expect(f).toContain('colorchannelmixer=aa=0.3[wmtile]');
  // 30° → pad to 1.5× (≥ √2 so rotated corners stay covered), rotate in radians,
  // then re-centre the oversized layer with negative offsets.
  expect(f).toContain('pad=w=iw*1.5:h=ih*1.5:x=(ow-iw)/2:y=(oh-ih)/2:color=black@0');
  expect(f).toContain('rotate=0.5236:c=black@0');
  expect(f).toContain('overlay=x=(W-w)/2:y=(H-h)/2:format=rgb');
  // 4×5 brick on the 1.5× canvas = 6 cols × 8 rows; the 4 offset rows draw one
  // extra tile each so both edges stay covered → 6*8 + 4 tiles.
  expect(countDrawtext(f)).toBe(52);

  // The bundled font and the watermark text ride along as virtual-FS inputs.
  const names = plan.inputs.map(([name]) => name);
  expect(names).toEqual(['font.ttf', 'watermark.txt']);
  expect(plan.inputs[1][1]).toBe('U0FNUExF'); // base64("SAMPLE")
  expect(plan.inputs[0][1].length).toBeGreaterThan(10_000); // the real TTF
});

// 4) Advertised-values matrix — every enum choice and both accepted hex forms,
//    exercised through the same entry point the page calls.
test('image-watermark-tile wasm build_argv covers every pattern, format and hex form', async ({ page }) => {
  await page.goto('/tools/image-watermark-tile/');
  await page.waitForSelector('#in-file');

  // pattern: grid aligns every row (3×3 = 9 tiles); brick offsets alternate rows
  // and draws one extra tile on each of them (3+4+3 = 10).
  const grid = await buildArgv(page, 'X', 20, '#ffffff', 0.3, 0, 3, 3, 'grid', 'false', 'keep', 'in.png');
  expect(countDrawtext(filterOf(grid))).toBe(9);
  const brick = await buildArgv(page, 'X', 20, '#ffffff', 0.3, 0, 3, 3, 'brick', 'false', 'keep', 'in.png');
  expect(countDrawtext(filterOf(brick))).toBe(10);
  // angle 0 skips the pad/rotate hop entirely and overlays 1:1.
  expect(filterOf(grid)).not.toContain('pad=');
  expect(filterOf(grid)).not.toContain('rotate=');
  expect(filterOf(grid)).toContain('overlay=x=0:y=0:format=rgb');

  // format: keep reuses the input container; the rest convert (and a converted
  // still image is capped at one frame).
  const keep = await buildArgv(page, 'X', 20, '#ffffff', 0.3, 0, 2, 2, 'grid', 'false', 'keep', 'photo.webp');
  expect(keep.out_name).toBe('out.webp');
  expect(keep.argv).not.toContain('-frames:v'); // keep never drops GIF animation
  expect(keep.argv).not.toContain('-q:v');

  const png = await buildArgv(page, 'X', 20, '#ffffff', 0.3, 0, 2, 2, 'grid', 'false', 'png', 'photo.jpg');
  expect(png.out_name).toBe('out.png');
  expect(png.argv).toContain('-frames:v');
  expect(png.argv).toContain('-update');
  expect(png.argv).not.toContain('-q:v'); // PNG is lossless — no quality flag

  const jpg = await buildArgv(page, 'X', 20, '#ffffff', 0.3, 0, 2, 2, 'grid', 'false', 'jpg', 'photo.png');
  expect(jpg.out_name).toBe('out.jpg');
  expect(jpg.argv.slice(jpg.argv.indexOf('-q:v'), jpg.argv.indexOf('-q:v') + 2)).toEqual(['-q:v', '2']);
  expect(jpg.argv).toContain('-frames:v');

  const webp = await buildArgv(page, 'X', 20, '#ffffff', 0.3, 0, 2, 2, 'grid', 'false', 'webp', 'photo.png');
  expect(webp.out_name).toBe('out.webp');
  expect(webp.argv).toContain('-frames:v');

  // Both advertised hex forms normalize to the same ffmpeg color, and a CSS name
  // is passed through — a bare hex string must not be numeric-coerced on the way.
  const short = await buildArgv(page, 'X', 20, '#fff', 0.3, 0, 2, 2, 'grid', 'false', 'keep', 'in.png');
  const long = await buildArgv(page, 'X', 20, '#ffffff', 0.3, 0, 2, 2, 'grid', 'false', 'keep', 'in.png');
  expect(filterOf(short)).toContain('fontcolor=0xFFFFFF:');
  expect(filterOf(short)).toBe(filterOf(long));
  const named = await buildArgv(page, 'X', 20, 'black', 0.3, 0, 2, 2, 'grid', 'false', 'keep', 'in.png');
  expect(filterOf(named)).toContain('fontcolor=black:');

  // The NON-default checkbox state must marshal as a real boolean, not "on".
  const outlined = await buildArgv(page, 'X', 56, '#ffffff', 0.3, 0, 2, 2, 'grid', 'true', 'keep', 'in.png');
  expect(filterOf(outlined)).toContain('borderw=4:bordercolor=black');
  expect(filterOf(long)).not.toContain('borderw');
});

// 5) Error path + cap boundaries: the limits the page advertises are enforced,
//    with messages that say what was expected.
test('image-watermark-tile wasm build_argv rejects empty text and out-of-range settings', async ({ page }) => {
  await page.goto('/tools/image-watermark-tile/');
  await page.waitForSelector('#in-file');

  expect(await buildArgvError(page, '', 32, 0.3, 4)).toContain('text must not be empty');
  expect(await buildArgvError(page, 'x'.repeat(121), 32, 0.3, 4)).toContain('too long');
  expect(await buildArgvError(page, 'X', 401, 0.3, 4)).toContain('font_size must be between 6 and 400');
  expect(await buildArgvError(page, 'X', 32, 1.5, 4)).toContain('opacity must be between');
  expect(await buildArgvError(page, 'X', 32, 0.3, 13)).toContain('columns must be between 1 and 12');

  // Exactly at each cap still plans.
  const atCap = await buildArgv(page, 'x'.repeat(120), 400, '#ffffff', 1, 90, 12, 12, 'brick', 'false', 'keep', 'in.png');
  expect(atCap.out_name).toBe('out.png');
  expect(countDrawtext(filterOf(atCap))).toBeGreaterThan(0);
});
