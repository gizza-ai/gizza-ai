import { test, expect } from './fixtures';

const weatherArff = `@relation weather
@attribute outlook {sunny,overcast,rainy}
@attribute temperature numeric
@data
sunny,85
rainy,70
`;

test('arff-converter page converts ARFF to CSV', async ({ page }) => {
  await page.goto('/tools/arff-converter/');
  await page.fill('#in-data', weatherArff);
  await page.selectOption('#in-direction', 'arff-to-csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('outlook,temperature', { timeout: 15_000 });
  expect(await out.textContent()).toBe('outlook,temperature\nsunny,85\nrainy,70');
});

test('arff-converter page deep-link converts CSV to typed ARFF', async ({ page }) => {
  const csv = 'outlook,temperature\nsunny,85\nrainy,70\n';
  const qs =
    '?data=' + encodeURIComponent(csv) +
    '&direction=csv-to-arff' +
    '&relation=weather' +
    '&nominal_threshold=10' +
    '&header=true';
  await page.goto('/tools/arff-converter/' + qs);

  await expect(page.locator('#in-direction')).toHaveValue('csv-to-arff', { timeout: 15_000 });
  await expect(page.locator('#in-relation')).toHaveValue('weather');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('@relation weather', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`@relation weather

@attribute outlook {sunny,rainy}
@attribute temperature numeric

@data
sunny,85
rainy,70
`.trimEnd());
});

test('arff-converter page writes and reads a type row', async ({ page }) => {
  await page.goto('/tools/arff-converter/');
  await page.fill('#in-data', weatherArff);
  await page.selectOption('#in-direction', 'arff-to-csv');
  await page.check('#in-type_row');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"{sunny,overcast,rainy}",numeric', { timeout: 15_000 });
  expect(await out.textContent()).toBe('outlook,temperature\n"{sunny,overcast,rainy}",numeric\nsunny,85\nrainy,70');
});

test('arff-converter page can emit sparse ARFF rows', async ({ page }) => {
  await page.goto('/tools/arff-converter/');
  await page.fill('#in-data', 'label,count\na,0\nb,2\n');
  await page.selectOption('#in-direction', 'csv-to-arff');
  await page.selectOption('#in-arff_format', 'sparse');
  await page.fill('#in-relation', 'counts');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('@relation counts', { timeout: 15_000 });
  await expect(out).toContainText('{0 b,1 2}');
});
