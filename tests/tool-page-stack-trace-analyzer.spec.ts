import { test, expect } from './fixtures';

const javaTrace = [
  'Exception in thread "main" com.example.SvcException: could not start',
  '\tat com.example.App.start(App.java:42)',
  '\tat org.springframework.boot.SpringApplication.run(SpringApplication.java:301)',
  'Caused by: java.net.ConnectException: Connection refused',
  '\tat java.base/sun.nio.ch.Net.pollConnect(Native Method)',
  '\tat com.example.Db.connect(Db.java:17)',
  '\t... 12 more',
].join('\n');

const pythonTrace = [
  'Traceback (most recent call last):',
  '  File "/app/main.py", line 10, in <module>',
  '    main()',
  '  File "/app/svc.py", line 4, in divide',
  '    return a / b',
  'ZeroDivisionError: division by zero',
].join('\n');

const jsTrace = [
  "TypeError: Cannot read properties of undefined (reading 'name')",
  '    at getName (/app/src/user.js:12:18)',
  '    at /app/src/index.js:4:3',
  '    at Module._compile (node:internal/modules/cjs/loader:1105:14)',
  '    at Array.forEach (<anonymous>)',
].join('\n');

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  trace: string,
  language = 'auto',
  output = 'report',
  userPackages = '',
  hideFramework = 'false',
  reverse = 'false',
  limit = '100',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/stack-trace-analyzer/gizza_ai_stack_trace_analyzer_web.js');
    await mod.default('/tools/stack-trace-analyzer/gizza_ai_stack_trace_analyzer_web_bg.wasm');
    return mod.run(
      args.trace,
      args.language,
      args.output,
      args.userPackages,
      args.hideFramework,
      args.reverse,
      args.limit,
    );
  }, { trace, language, output, userPackages, hideFramework, reverse, limit });
}

test('stack-trace-analyzer wasm parses Java chains and exact root-cause output', async ({ page }) => {
  await page.goto('/tools/stack-trace-analyzer/');
  await page.waitForSelector('#in-trace');

  const out = await runWasm(page, javaTrace);
  expect(out).toContain('Language: Java / Kotlin / Scala (auto-detected)');
  expect(out).toContain('Reported: com.example.SvcException: could not start');
  expect(out).toContain('Root cause: java.net.ConnectException: Connection refused');
  expect(out).toContain('First user frame: com.example.Db.connect(Db.java:17)');
  expect(out).toContain('12 frame(s) elided by the runtime');
});

test('stack-trace-analyzer wasm covers language/output enum values and boolean controls', async ({ page }) => {
  await page.goto('/tools/stack-trace-analyzer/');
  await page.waitForSelector('#in-trace');

  await expect(runWasm(page, pythonTrace, 'python')).resolves.toContain('Language: Python (as selected)');
  await expect(runWasm(page, jsTrace, 'javascript', 'table', '/app/src', 'true'))
    .resolves.toContain('| 1 | user | getName | /app/src/user.js | 12 | 18 |');

  const json = await runWasm(page, javaTrace, 'java', 'json', 'com.example', 'false', 'true', '2');
  expect(json).toContain('"language": "java"');
  expect(json).toContain('"language_detected": false');
  expect(json).toContain('"root_cause": {');
  expect(json).toContain('"frames_truncated":');
});

test('stack-trace-analyzer page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/stack-trace-analyzer/');
  await setTextarea(page, '#in-trace', javaTrace);
  await page.fill('#in-user_packages', 'com.example');
  await page.check('#in-hide_framework');

  await expect(page.locator('#tool-output')).toContainText('Root cause: java.net.ConnectException: Connection refused', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('First user frame: com.example.Db.connect(Db.java:17)');
  await expect(page.locator('#tool-output')).toContainText('framework frame(s) hidden');
});

test('stack-trace-analyzer deep-link prefills params and generated CLI example is generic', async ({ page }) => {
  const params = new URLSearchParams({
    trace: pythonTrace,
    language: 'python',
    output: 'report',
    user_packages: '/app',
    hide_framework: 'false',
    reverse: 'false',
    limit: '100',
  });

  await page.goto(`/tools/stack-trace-analyzer/?${params.toString()}`);
  await expect(page.locator('#in-trace')).toHaveValue(pythonTrace, { timeout: 15_000 });
  await expect(page.locator('#in-language')).toHaveValue('python');
  await expect(page.locator('#tool-output')).toContainText('Root cause: same as reported (single exception)', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('First user frame: divide(/app/svc.py:4)');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool stack-trace-analyzer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
