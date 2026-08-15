import { test, expect } from './fixtures';

const PS = `CONTAINER ID   IMAGE          COMMAND                  CREATED         STATUS         PORTS                    NAMES
9f21a1b2c3d4   nginx:1.25     "/docker-entrypoint.…"   3 minutes ago   Up 3 minutes   0.0.0.0:8080->80/tcp     web
0011deadbeef   postgres:16    "docker-entrypoint.s…"   2 hours ago     Up 2 hours                              db`;

const STATS = `CONTAINER ID   NAME   CPU %   MEM USAGE / LIMIT     MEM %     NET I/O           BLOCK I/O     PIDS
9f21a1b2c3d4   web    0.07%   12.05MiB / 7.667GiB   0.15%     1.31kB / 0B       0B / 8.19kB   5
0011deadbeef   db     1.25%   64.5MiB / 7.667GiB    0.82%     18.4kB / 12.2kB   4.1MB / 0B    12`;

const IMAGES = `REPOSITORY   TAG       IMAGE ID       CREATED        SIZE
nginx        1.25      a1b2c3d4e5f6   2 weeks ago    187MB
postgres     16        112233445566   3 months ago   432MB`;

const PS_JSON = `[
  {
    "container_id": "9f21a1b2c3d4",
    "image": "nginx:1.25",
    "command": "/docker-entrypoint.…",
    "created": "3 minutes ago",
    "status": "Up 3 minutes",
    "ports": [
      "0.0.0.0:8080->80/tcp"
    ],
    "names": [
      "web"
    ]
  },
  {
    "container_id": "0011deadbeef",
    "image": "postgres:16",
    "command": "docker-entrypoint.s…",
    "created": "2 hours ago",
    "status": "Up 2 hours",
    "ports": [],
    "names": [
      "db"
    ]
  }
]`;

const STATS_CSV = `container_id,name,cpu_percent,mem_usage,mem_usage_bytes,mem_limit,mem_limit_bytes,mem_percent,net_input,net_input_bytes,net_output,net_output_bytes,block_input,block_input_bytes,block_output,block_output_bytes,pids
9f21a1b2c3d4,web,0.07,12.05MiB,12635341,7.667GiB,8232378565,0.15,1.31kB,1310,0B,0,0B,0,8.19kB,8190,5
0011deadbeef,db,1.25,64.5MiB,67633152,7.667GiB,8232378565,0.82,18.4kB,18400,12.2kB,12200,4.1MB,4100000,0B,0,12`;

async function runWasm(page: any, input: string, kind = 'auto', output = 'json', keys = 'snake', parseValues = 'true', columns = '', header = 'true', strict = 'false', limit = '500') {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/docker-cli-output-parser/gizza_ai_docker_cli_output_parser_web.js');
    await mod.default('/tools/docker-cli-output-parser/gizza_ai_docker_cli_output_parser_web_bg.wasm');
    return mod.run(args.input, args.kind, args.output, args.keys, args.parseValues, args.columns, args.header, args.strict, args.limit);
  }, { input, kind, output, keys, parseValues, columns, header, strict, limit });
}

test('docker-cli-output-parser page parses docker ps to exact JSON', async ({ page }) => {
  await page.goto('/tools/docker-cli-output-parser/');
  await page.fill('#in-input', PS);
  await expect(page.locator('#tool-output')).toContainText('"container_id": "9f21a1b2c3d4"', { timeout: 15_000 });
  expect(await page.locator('#tool-output').textContent()).toBe(PS_JSON);
});

test('docker-cli-output-parser deep link pre-fills stats CSV', async ({ page }) => {
  const params = new URLSearchParams({ input: STATS, kind: 'stats', output: 'csv' });
  await page.goto(`/tools/docker-cli-output-parser/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(STATS, { timeout: 15_000 });
  await expect(page.locator('#in-kind')).toHaveValue('stats');
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('mem_usage_bytes', { timeout: 15_000 });
  expect(await page.locator('#tool-output').textContent()).toBe(STATS_CSV);
});

test('docker-cli-output-parser example chip fills images markdown', async ({ page }) => {
  await page.goto('/tools/docker-cli-output-parser/');
  const chips = page.locator('.tool-example-chip');
  await expect(chips).toHaveCount(4);
  await chips.nth(2).click();
  await expect(page.locator('#in-input')).toHaveValue(IMAGES);
  await expect(page.locator('#in-output')).toHaveValue('markdown');
  await expect(page.locator('#tool-output')).toContainText('| repository | tag  | image_id', { timeout: 15_000 });
});

test('docker-cli-output-parser wasm covers advertised options', async ({ page }) => {
  await page.goto('/tools/docker-cli-output-parser/');
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, PS, 'ps', 'json')).toBe(PS_JSON);
  expect(await runWasm(page, STATS, 'stats', 'csv')).toBe(STATS_CSV);
  expect(await runWasm(page, STATS, 'stats', 'tsv')).toContain('container_id\tname\tcpu_percent');
  expect(await runWasm(page, IMAGES, 'images', 'markdown')).toContain('| nginx      | 1.25 |');
  expect(await runWasm(page, STATS, 'stats', 'table', 'snake', 'true', 'name,cpu_percent,mem_usage_bytes,pids')).toBe(
    ['name   cpu_percent   mem_usage_bytes   pids', 'web    0.07          12635341          5', 'db     1.25          67633152          12'].join('\n'),
  );

  const raw = JSON.parse(await runWasm(page, STATS, 'stats', 'json', 'header', 'false'));
  expect(raw[0]['CPU %']).toBe('0.07%');
  const dockerKeys = JSON.parse(await runWasm(page, STATS, 'stats', 'json', 'docker'));
  expect(dockerKeys[0].CPUPerc).toBe(0.07);
  expect(await runWasm(page, IMAGES, 'images', 'csv', 'snake', 'true', 'repository', 'false', 'false', '1')).toBe('nginx');
  await expect(runWasm(page, IMAGES, 'stats', 'json', 'snake', 'true', '', 'true', 'true')).rejects.toThrow(/looks like docker images output, not docker stats/);
  await expect(runWasm(page, IMAGES, 'images', 'json', 'snake', 'true', '', 'true', 'false', '5001')).rejects.toThrow(/limit must be a whole number/);
});

test('docker-cli-output-parser generated CLI example is generic and runnable-looking', async ({ page }) => {
  await page.goto('/tools/docker-cli-output-parser/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool docker-cli-output-parser');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
