import { test, expect } from './fixtures';

const LOGS = '{"time":"2026-08-08T12:00:00Z","level":"info","msg":"server started","port":8080}\n{"time":"2026-08-08T12:00:09Z","level":"error","msg":"db timeout","req":{"method":"GET","url":"/api"}}';

async function runWasm(
  page,
  input: string,
  level = 'all',
  field = '',
  filter = '',
  match = 'contains',
  fields = '',
  levelField = '',
  timeField = '',
  messageField = '',
  flatten = 'true',
  onInvalid = 'skip',
  limit = '200',
  output = 'pretty',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/json-log-formatter/gizza_ai_json_log_formatter_web.js');
    await mod.default('/tools/json-log-formatter/gizza_ai_json_log_formatter_web_bg.wasm');
    return mod.run(args.input, args.level, args.field, args.filter, args.match, args.fields, args.levelField, args.timeField, args.messageField, args.flatten, args.onInvalid, args.limit, args.output);
  }, { input, level, field, filter, match, fields, levelField, timeField, messageField, flatten, onInvalid, limit, output });
}

const PRETTY = `2 records

[2026-08-08T12:00:00Z] INFO  server started port=8080
[2026-08-08T12:00:09Z] ERROR db timeout     req.method=GET req.url=/api`;

const ERROR_ONLY = `2 records · 1 shown

[2026-08-08T12:00:09Z] ERROR db timeout req.method=GET req.url=/api`;

const TABLE = `2 records · 1 shown

| time                 | level | msg        | req.method |
| -------------------- | ----- | ---------- | ---------- |
| 2026-08-08T12:00:09Z | error | db timeout | GET        |`;

const CSV = `time,level,msg,req.method
2026-08-08T12:00:00Z,info,server started,
2026-08-08T12:00:09Z,error,db timeout,GET`;

test('json-log-formatter wasm renders aligned pretty logs exactly', async ({ page }) => {
  await page.goto('/tools/json-log-formatter/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, LOGS)).resolves.toBe(PRETTY);
});

test('json-log-formatter wasm covers level filters, field filters, formats, and cap errors', async ({ page }) => {
  await page.goto('/tools/json-log-formatter/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, LOGS, 'error')).resolves.toBe(ERROR_ONLY);
  await expect(runWasm(page, LOGS, 'all', 'req.method', 'GET', 'exact', 'time,level,msg,req.method', '', '', '', 'true', 'skip', '200', 'table')).resolves.toBe(TABLE);
  await expect(runWasm(page, LOGS, 'all', '', '', 'contains', 'time,level,msg,req.method', '', '', '', 'true', 'skip', '200', 'csv')).resolves.toBe(CSV);
  await expect(runWasm(page, LOGS, 'all', '', '', 'contains', '', '', '', '', 'true', 'skip', '5000')).resolves.toBe(PRETTY);
});

test('json-log-formatter page renders exact output and honors non-default checkbox', async ({ page }) => {
  await page.goto('/tools/json-log-formatter/');
  await page.fill('#in-input', '{"time":"2026-08-08T12:00:00Z","level":"info","msg":"server started","req":{"method":"GET"}}');
  await page.uncheck('#in-flatten');
  await page.fill('#in-limit', '1');

  await expect(page.locator('#tool-output')).toHaveText('1 record\n\n[2026-08-08T12:00:00Z] INFO server started req="{\\\"method\\\":\\\"GET\\\"}"', { timeout: 15_000 });
});

test('json-log-formatter deep-link prefills controls and filters a table', async ({ page }) => {
  const params = new URLSearchParams({
    input: LOGS,
    level: 'all',
    field: 'req.method',
    filter: 'GET',
    match: 'exact',
    fields: 'time,level,msg,req.method',
    level_field: '',
    time_field: '',
    message_field: '',
    flatten: 'true',
    on_invalid: 'skip',
    limit: '200',
    output: 'table',
  });

  await page.goto(`/tools/json-log-formatter/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(LOGS, { timeout: 15_000 });
  await expect(page.locator('#in-field')).toHaveValue('req.method');
  await expect(page.locator('#in-match')).toHaveValue('exact');
  await expect(page.locator('#in-fields')).toHaveValue('time,level,msg,req.method');
  await expect(page.locator('#in-flatten')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('table');
  await expect(page.locator('#tool-output')).toHaveText(`2 records · 1 shown

| time                 | level | msg        | req.method |
| -------------------- | ----- | ---------- | ---------- |
| 2026-08-08T12:00:09Z | error | db timeout | GET        |`, { timeout: 15_000 });
});
