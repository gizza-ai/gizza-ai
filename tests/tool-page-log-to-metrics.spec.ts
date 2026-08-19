import { test, expect } from './fixtures';

const JSON_LOG = [
  '{"ts":"2024-05-06T07:00:00Z","route":"/api/users","status":200,"duration_ms":10}',
  '{"ts":"2024-05-06T07:00:20Z","route":"/api/users","status":500,"duration_ms":30}',
  '{"ts":"2024-05-06T07:00:40Z","route":"/api/users","status":200,"duration_ms":20}',
  '{"ts":"2024-05-06T07:01:00Z","route":"/health","status":200,"duration_ms":2}',
].join('\n');

async function runWasm(page: any, args: Partial<Record<string, string | boolean | number>> = {}) {
  const defaults = {
    data: JSON_LOG,
    format: 'json',
    group_by: 'route',
    value_field: 'duration_ms',
    percentiles: '50,95,99',
    percentile_method: 'linear',
    time_field: 'ts',
    rate_unit: 'minute',
    error_field: 'status',
    error_values: '5*',
    limit: '20',
    other: 'true',
    sort: 'count',
    output: 'table',
    metric_prefix: 'log',
    ...args,
  };
  return await page.evaluate(async (a) => {
    const mod = await import('/tools/log-to-metrics/gizza_ai_log_to_metrics_web.js');
    await mod.default('/tools/log-to-metrics/gizza_ai_log_to_metrics_web_bg.wasm');
    return mod.run(
      a.data, a.format, a.group_by, a.value_field, a.percentiles, a.percentile_method,
      a.time_field, a.rate_unit, a.error_field, a.error_values, a.limit, a.other,
      a.sort, a.output, a.metric_prefix,
    );
  }, defaults);
}

test('log-to-metrics wasm aggregates JSON logs into exact table output', async ({ page }) => {
  await page.goto('/tools/log-to-metrics/');
  const out = await runWasm(page);
  expect(out).toContain('lines=4 parsed=4 unparsed=0 format=json groups=2 span=60s');
  expect(out).toContain('| /api/users |     3 |      75 |        3 |      1 |  33.33 |  10 |  20 |  20 |  29 | 29.8 |  30 |  60 |');
  expect(out).toContain('| /health    |     1 |      25 |        1 |      0 |      0 |   2 |   2 |   2 |   2 |    2 |   2 |   2 |');
});

test('log-to-metrics page computes exact JSON report from the form', async ({ page }) => {
  await page.goto('/tools/log-to-metrics/');
  await page.fill('#in-data', JSON_LOG);
  await page.selectOption('#in-format', 'json');
  await page.fill('#in-group_by', 'route');
  await page.fill('#in-value_field', 'duration_ms');
  await page.fill('#in-time_field', 'ts');
  await page.selectOption('#in-rate_unit', 'second');
  await page.fill('#in-error_field', 'status');
  await page.fill('#in-error_values', '5*');
  await page.selectOption('#in-output', 'json');

  const raw = await page.locator('#tool-output').textContent({ timeout: 15_000 });
  const report = JSON.parse(raw ?? '');
  expect(report.parsed).toBe(4);
  expect(report.span_seconds).toBe(60);
  expect(report.groups[0].group.route).toBe('/api/users');
  expect(report.groups[0].count).toBe(3);
  expect(report.groups[0].errors).toBe(1);
  expect(report.groups[0].value.percentiles.p95).toBe(29);
});

test('log-to-metrics deep link covers logfmt, nearest percentile and unchecked other', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'route=/a ms=10\nroute=/a ms=30\nroute=/b ms=5\nroute=/c ms=1',
    format: 'logfmt',
    group_by: 'route',
    value_field: 'ms',
    percentiles: '50',
    percentile_method: 'nearest',
    time_field: 'none',
    rate_unit: 'auto',
    error_field: '',
    error_values: '',
    limit: '2',
    other: 'false',
    sort: 'p_top',
    output: 'csv',
    metric_prefix: 'log',
  });
  await page.goto(`/tools/log-to-metrics/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('logfmt', { timeout: 15_000 });
  await expect(page.locator('#in-percentile_method')).toHaveValue('nearest');
  await expect(page.locator('#in-other')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('route,count,percent,min,avg,p50,max,sum\n/a,2,50,10,20,10,30,40\n/b,1,25,5,5,5,5,5', { timeout: 15_000 });
});

test('log-to-metrics covers CSV, Prometheus output, cap boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/log-to-metrics/');

  const csv = await runWasm(page, { data: 'route,ms,level\n/a,10,info\n/a,20,error\n/b,5,info', format: 'csv', group_by: 'route', value_field: 'ms', time_field: 'none', error_field: 'level', error_values: 'error', output: 'csv' });
  expect(csv).toContain('route,count,percent,errors,error%,min,avg,p50,p95,p99,max,sum');
  expect(csv).toContain('/a,2,66.67,1,50,10,15,15,19.5,19.9,20,30');

  const prom = await runWasm(page, { output: 'prometheus', percentiles: '95', metric_prefix: 'http' });
  expect(prom).toContain('# TYPE http_lines_total counter');
  expect(prom).toContain('http_duration_ms{route="/api/users",quantile="0.95"} 29');

  const atCap = 'route=/a\n'.repeat(200_000);
  await expect(runWasm(page, { data: atCap, format: 'logfmt', group_by: 'route', value_field: '', time_field: 'none', output: 'csv' })).resolves.toContain('/a,200000,100');
  await expect(runWasm(page, { data: `${atCap}route=/a\n`, format: 'logfmt' })).rejects.toThrow(/too many lines/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())?.trim() ?? '';
  expect(cli).toContain('gizza tool log-to-metrics');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
