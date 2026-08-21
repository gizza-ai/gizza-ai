import { test, expect } from './fixtures';

const TWO = '>seq1 first sequence\nACGTACGTNN\n>seq2\nacgt';

const DEFAULT_CSV =
  'id,description,sequence,length\n' +
  'seq1,first sequence,ACGTACGTNN,10\n' +
  'seq2,,acgt,4\n';

/**
 * Call the page's wasm export directly. Argument order mirrors
 * blocks/fasta-to-csv/web/src/lib.rs `run(...)`; every field is a string, as the
 * page driver sends them.
 */
async function runWasm(
  page: any,
  fasta: string,
  delimiter = 'comma',
  headerMode = 'split',
  headerRow = 'true',
  includeSequence = 'true',
  includeLength = 'true',
  includeGc = 'false',
  includeBaseCounts = 'false',
  uppercase = 'false',
  dedupe = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/fasta-to-csv/gizza_ai_fasta_to_csv_web.js');
    await mod.default('/tools/fasta-to-csv/gizza_ai_fasta_to_csv_web_bg.wasm');
    return mod.run(
      args.fasta,
      args.delimiter,
      args.headerMode,
      args.headerRow,
      args.includeSequence,
      args.includeLength,
      args.includeGc,
      args.includeBaseCounts,
      args.uppercase,
      args.dedupe,
    );
  }, {
    fasta, delimiter, headerMode, headerRow, includeSequence,
    includeLength, includeGc, includeBaseCounts, uppercase, dedupe,
  });
}

test('fasta-to-csv wasm emits the default id/description/sequence/length table', async ({ page }) => {
  await page.goto('/tools/fasta-to-csv/');
  await page.waitForSelector('#in-fasta');

  await expect(runWasm(page, TWO)).resolves.toBe(DEFAULT_CSV);

  // Wrapped sequence lines are joined; blank lines and CRLF are tolerated.
  await expect(runWasm(page, '>s desc\r\nACGT\r\n\r\nACGT\r\nAC\r\n'))
    .resolves.toBe('id,description,sequence,length\ns,desc,ACGTACGTAC,10\n');
});

test('fasta-to-csv wasm covers every advertised delimiter and header mode', async ({ page }) => {
  await page.goto('/tools/fasta-to-csv/');
  await page.waitForSelector('#in-fasta');

  await expect(runWasm(page, TWO, 'tab')).resolves.toBe(
    'id\tdescription\tsequence\tlength\nseq1\tfirst sequence\tACGTACGTNN\t10\nseq2\t\tacgt\t4\n',
  );
  await expect(runWasm(page, TWO, 'semicolon')).resolves.toBe(
    'id;description;sequence;length\nseq1;first sequence;ACGTACGTNN;10\nseq2;;acgt;4\n',
  );
  await expect(runWasm(page, TWO, 'pipe')).resolves.toBe(
    'id|description|sequence|length\nseq1|first sequence|ACGTACGTNN|10\nseq2||acgt|4\n',
  );

  await expect(runWasm(page, TWO, 'comma', 'id_only')).resolves.toBe(
    'id,sequence,length\nseq1,ACGTACGTNN,10\nseq2,acgt,4\n',
  );
  await expect(runWasm(page, TWO, 'comma', 'full_header')).resolves.toBe(
    'id,sequence,length\nseq1 first sequence,ACGTACGTNN,10\nseq2,acgt,4\n',
  );
});

test('fasta-to-csv wasm honors every boolean option, including the default-on ones off', async ({ page }) => {
  await page.goto('/tools/fasta-to-csv/');
  await page.waitForSelector('#in-fasta');

  // header_row off — bare data rows.
  await expect(runWasm(page, TWO, 'comma', 'split', 'false')).resolves.toBe(
    'seq1,first sequence,ACGTACGTNN,10\nseq2,,acgt,4\n',
  );
  // include_sequence off — names + metrics only.
  await expect(runWasm(page, TWO, 'comma', 'split', 'true', 'false')).resolves.toBe(
    'id,description,length\nseq1,first sequence,10\nseq2,,4\n',
  );
  // include_length off.
  await expect(runWasm(page, TWO, 'comma', 'split', 'true', 'true', 'false')).resolves.toBe(
    'id,description,sequence\nseq1,first sequence,ACGTACGTNN\nseq2,,acgt\n',
  );
  // GC + base counts on: seq1's two Ns land in other_count and are excluded from GC.
  await expect(runWasm(page, TWO, 'comma', 'split', 'true', 'true', 'true', 'true', 'true')).resolves.toBe(
    'id,description,sequence,length,gc_percent,a_count,c_count,g_count,t_count,other_count\n' +
      'seq1,first sequence,ACGTACGTNN,10,50.00,2,2,2,2,2\n' +
      'seq2,,acgt,4,50.00,1,1,1,1,0\n',
  );
  // uppercase touches the sequence column only.
  await expect(runWasm(page, TWO, 'comma', 'split', 'true', 'true', 'true', 'false', 'false', 'true'))
    .resolves.toBe(
      'id,description,sequence,length\nseq1,first sequence,ACGTACGTNN,10\nseq2,,ACGT,4\n',
    );
  // dedupe is case-insensitive and keeps the first record.
  await expect(
    runWasm(page, '>a\nACGT\n>b\nacgt\n>c\nTTTT', 'comma', 'split', 'true', 'true', 'true', 'false', 'false', 'false', 'true'),
  ).resolves.toBe('id,description,sequence,length\na,,ACGT,4\nc,,TTTT,4\n');
});

test('fasta-to-csv wasm quotes RFC-4180 fields and reports malformed input', async ({ page }) => {
  await page.goto('/tools/fasta-to-csv/');
  await page.waitForSelector('#in-fasta');

  await expect(runWasm(page, '>s alpha, beta\nAC')).resolves.toBe(
    'id,description,sequence,length\ns,"alpha, beta",AC,2\n',
  );
  await expect(runWasm(page, '>s say "hi", ok\nAC')).resolves.toBe(
    'id,description,sequence,length\ns,"say ""hi"", ok",AC,2\n',
  );
  // The same description needs no quoting once the delimiter is a tab.
  await expect(runWasm(page, '>s alpha, beta\nAC', 'tab')).resolves.toBe(
    'id\tdescription\tsequence\tlength\ns\talpha, beta\tAC\t2\n',
  );

  await expect(runWasm(page, 'ACGT\n')).rejects.toThrow(/no '>' header line came before it/);
  await expect(runWasm(page, '   \n\n')).rejects.toThrow(/no FASTA records found/);
});

test('fasta-to-csv page renders exact CSV and honors the checkboxes', async ({ page }) => {
  await page.goto('/tools/fasta-to-csv/');
  await page.fill('#in-fasta', TWO);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('seq1', { timeout: 15_000 });
  expect(await out.textContent()).toBe(DEFAULT_CSV);

  // A default-ON checkbox switched off, plus a default-OFF one switched on.
  await page.uncheck('#in-include_sequence');
  await page.check('#in-include_gc');
  await expect(out).toContainText('gc_percent', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'id,description,length,gc_percent\nseq1,first sequence,10,50.00\nseq2,,4,50.00\n',
  );
});

test('fasta-to-csv page reports FASTA with no header line', async ({ page }) => {
  await page.goto('/tools/fasta-to-csv/');
  await page.fill('#in-fasta', 'ACGTACGT');
  await expect(page.locator('#tool-output')).toContainText("no '>' header line came before it", {
    timeout: 15_000,
  });
});

test('fasta-to-csv deep link prefills the selects and computes exact TSV', async ({ page }) => {
  const params = new URLSearchParams({
    fasta: TWO,
    delimiter: 'tab',
    header_mode: 'id_only',
    header_row: 'true',
    include_sequence: 'true',
    include_length: 'true',
    include_gc: 'false',
    include_base_counts: 'false',
    uppercase: 'true',
    dedupe: 'false',
  });
  await page.goto(`/tools/fasta-to-csv/?${params.toString()}`);

  await expect(page.locator('#in-fasta')).toHaveValue(TWO, { timeout: 15_000 });
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');
  await expect(page.locator('#in-header_mode')).toHaveValue('id_only');
  await expect(page.locator('#in-uppercase')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('seq1', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'id\tsequence\tlength\nseq1\tACGTACGTNN\t10\nseq2\tACGT\t4\n',
  );
});

test('fasta-to-csv page ships a runnable, brand-free CLI example', async ({ page }) => {
  await page.goto('/tools/fasta-to-csv/');
  await page.waitForSelector('#in-fasta');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool fasta-to-csv');
  expect(cli).toContain('>seq1 first sequence');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
