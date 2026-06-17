import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runFfmpegRequest } from './ffmpeg-engine.js';

test('runFfmpegRequest returns the exec result on success', async () => {
  const fakeExec = async (a, i, o) => {
    assert.equal(a, '["-i","in.png","out.png"]');
    assert.equal(i, '[{"name":"in.png","bytes_b64":"AAA"}]');
    assert.equal(o, 'out.png');
    return { exit_code: 0, output_b64: 'QkJC', log: 'ok' };
  };
  const resp = await runFfmpegRequest(
    {
      args_json: '["-i","in.png","out.png"]',
      inputs_json: '[{"name":"in.png","bytes_b64":"AAA"}]',
      output_name: 'out.png',
    },
    fakeExec,
  );
  assert.deepEqual(resp, { exit_code: 0, output_b64: 'QkJC', log: 'ok' });
});

test('runFfmpegRequest encodes a thrown error as exit_code -1 (never throws)', async () => {
  const boom = async () => { throw new Error('worker died'); };
  const resp = await runFfmpegRequest(
    { args_json: '[]', inputs_json: '[]', output_name: 'out.png' },
    boom,
  );
  assert.equal(resp.exit_code, -1);
  assert.equal(resp.output_b64, '');
  assert.match(resp.log, /worker died/);
});
