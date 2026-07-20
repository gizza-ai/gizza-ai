import { test, expect } from './fixtures';

// Two-entry capture: a login POST with a JSON body + websocket frames, and a
// base64-inlined image response. All times are integers or non-integral
// floats so serde's compact serialization matches JSON.stringify exactly.
const harObj = {
  log: {
    version: '1.2',
    creator: { name: 'devtools', version: '1' },
    entries: [
      {
        startedDateTime: '2024-01-01T00:00:00.000Z',
        time: 120,
        request: {
          method: 'POST',
          url: 'https://example.com/api/login',
          postData: {
            mimeType: 'application/json',
            text: '{"user":"alice","password":"hunter2"}',
          },
        },
        response: {
          status: 200,
          statusText: 'OK',
          content: { size: 20, mimeType: 'application/json', text: '{"session":"abc123"}' },
          bodySize: 20,
        },
        _webSocketMessages: [
          { type: 'send', time: 1.5, opcode: 1, data: 'ws-auth-token' },
          { type: 'receive', time: 2.5, opcode: 1, data: 'ws-ack' },
        ],
      },
      {
        startedDateTime: '2024-01-01T00:00:01.000Z',
        time: 80,
        request: { method: 'GET', url: 'https://example.com/logo.png' },
        response: {
          status: 200,
          statusText: 'OK',
          content: {
            size: 51200,
            mimeType: 'image/png',
            encoding: 'base64',
            text: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
          },
          bodySize: 51234,
        },
      },
    ],
  },
};
const har = JSON.stringify(harObj);

type Side = { req: boolean; resp: boolean };

// Build the expected stripped capture: postData.text/params, content.text/
// encoding, and ws frame data removed per side (send=request, receive=response).
function stripped(side: Side, opts?: { skipImage?: boolean; onlyImage?: boolean; keepWs?: boolean }) {
  const c = JSON.parse(JSON.stringify(harObj));
  const [login, img] = c.log.entries;
  if (side.req && !opts?.onlyImage) delete login.request.postData.text;
  if (side.resp && !opts?.onlyImage) delete login.response.content.text;
  if (side.resp && !opts?.skipImage) {
    delete img.response.content.text;
    delete img.response.content.encoding;
  }
  if (!opts?.keepWs && !opts?.onlyImage) {
    if (side.req) delete login._webSocketMessages[0].data;
    if (side.resp) delete login._webSocketMessages[1].data;
  }
  return JSON.stringify(c);
}

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('default run strips both sides, ws payloads included — exact compact HAR', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', har);
  await expect(page.locator('#tool-output')).toContainText('"log"', { timeout: 15000 });
  expect(await output(page)).toBe(stripped({ req: true, resp: true }));
});

test('strip=request keeps response bodies and receive frames', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', har);
  await page.selectOption('#in-strip', 'request');
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    stripped({ req: true, resp: false }),
  );
});

test('strip=response keeps request bodies and send frames', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', har);
  await page.selectOption('#in-strip', 'response');
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    stripped({ req: false, resp: true }),
  );
});

test('only_mime comma list strips just matching bodies, ws frames survive', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', har);
  await page.fill('#in-only_mime', 'image/,font/');
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    stripped({ req: true, resp: true }, { onlyImage: true }),
  );
});

test('min_bytes keeps small bodies, drops the big blob', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', har);
  await page.fill('#in-min_bytes', '1000');
  // Only the 51200-byte (decoded) image body crosses the threshold: the
  // 37-byte postData, 20-byte JSON response, and ws frames all survive.
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    stripped({ req: true, resp: true }, { onlyImage: true }),
  );
});

test('pretty checkbox (non-default) emits indented JSON of the same data', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', har);
  await page.check('#in-pretty');
  await expect(page.locator('#tool-output')).toContainText('"log"', { timeout: 15000 });
  const out = await output(page);
  expect(out.startsWith('{\n  "log"')).toBe(true);
  expect(JSON.parse(out)).toEqual(JSON.parse(stripped({ req: true, resp: true })));
});

test('summary dry-run reports exact counts, bytes, and shrink', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', har);
  await page.selectOption('#in-output', 'summary');
  const reqBytes = harObj.log.entries[0].request.postData!.text.length; // 37
  const respBytes =
    harObj.log.entries[0].response.content.text!.length +
    harObj.log.entries[1].response.content.text!.length;
  const wsBytes = 'ws-auth-token'.length + 'ws-ack'.length; // 19
  const outLen = stripped({ req: true, resp: true }).length;
  const human = (b: number) => (b < 1024 ? `${b} B` : `${(b / 1024).toFixed(1)} KB`);
  const pct = (((har.length - outLen) / har.length) * 100).toFixed(1);
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    [
      'HAR body strip summary',
      'entries scanned: 2',
      `request bodies stripped: 1 (${human(reqBytes)})`,
      `response bodies stripped: 2 (${human(respBytes)})`,
      `websocket payloads stripped: 2 (${human(wsBytes)})`,
      `size: ${human(har.length)} → ${human(outLen)} (${pct}% smaller)`,
      'Run with output=har to get the stripped capture.',
    ].join('\n'),
  );
});

test('deep-link prefills and auto-runs', async ({ page }) => {
  const small =
    '{"log":{"entries":[{"request":{"method":"GET","url":"https://example.com/"},"response":{"status":200,"content":{"size":2,"mimeType":"text/plain","text":"hi"}}}]}}';
  const strippedSmall =
    '{"log":{"entries":[{"request":{"method":"GET","url":"https://example.com/"},"response":{"status":200,"content":{"size":2,"mimeType":"text/plain"}}}]}}';
  await page.goto(
    `/tools/har-body-stripper/?har=${encodeURIComponent(small)}&output=summary`,
  );
  const pct = (((small.length - strippedSmall.length) / small.length) * 100).toFixed(1);
  await expect.poll(async () => output(page), { timeout: 15000 }).toBe(
    [
      'HAR body strip summary',
      'entries scanned: 1',
      'request bodies stripped: 0',
      'response bodies stripped: 1 (2 B)',
      'websocket payloads stripped: 0',
      `size: ${small.length} B → ${strippedSmall.length} B (${pct}% smaller)`,
      'Run with output=har to get the stripped capture.',
    ].join('\n'),
  );
});

test('entry cap: 10000 entries run, 10001 error with the exact message', async ({ page }) => {
  const entry =
    '{"request":{"method":"GET","url":"https://x.test/"},"response":{"status":200,"content":{"size":1,"mimeType":"text/plain","text":"x"}}}';
  const bigHar = (n: number) => `{"log":{"entries":[${Array(n).fill(entry).join(',')}]}}`;
  const setHar = async (v: string) => {
    // page.fill on a multi-MB textarea routes through insertText and takes
    // minutes; set the value directly and fire the same event the driver
    // listens to (see create-next-tool references/page-patterns.md).
    await page.locator('#in-har').evaluate((el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, v);
  };
  await page.goto('/tools/har-body-stripper/');
  await page.selectOption('#in-output', 'summary');
  await setHar(bigHar(10000));
  await expect(page.locator('#tool-output')).toContainText('entries scanned: 10000', {
    timeout: 30000,
  });
  await expect(page.locator('#tool-output')).toContainText(
    'response bodies stripped: 10000',
  );
  await setHar(bigHar(10001));
  await expect(page.locator('#tool-output')).toContainText(
    'too many entries: 10001 (max 10000 entries per run)',
    { timeout: 30000 },
  );
});

test('non-JSON and non-HAR inputs error clearly', async ({ page }) => {
  await page.goto('/tools/har-body-stripper/');
  await page.fill('#in-har', 'not json at all');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
  await page.fill('#in-har', '{"foo":1}');
  await expect(page.locator('#tool-output')).toContainText('not a HAR capture', {
    timeout: 15000,
  });
});
