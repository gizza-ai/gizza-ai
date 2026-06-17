import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  makeId,
  registerPending,
  completeBridgeMessage,
  ffmpegExec,
} from './ffmpeg-bridge.js';

test('makeId returns unique ffmpeg-prefixed ids', () => {
  const a = makeId();
  const b = makeId();
  assert.match(a, /^ffmpeg-\d+$/);
  assert.notEqual(a, b);
});

test('completeBridgeMessage resolves the matching pending request', async () => {
  const p = registerPending('ffmpeg-test-1');
  const consumed = completeBridgeMessage({
    type: 'ffmpeg-exec-response',
    id: 'ffmpeg-test-1',
    exit_code: 0,
    output_b64: 'ZZZ',
    log: 'done',
  });
  assert.equal(consumed, true);
  assert.deepEqual(await p, { exit_code: 0, output_b64: 'ZZZ', log: 'done' });
});

test('completeBridgeMessage ignores foreign types and unknown ids', () => {
  assert.equal(completeBridgeMessage({ type: 'llm-stream-frame', id: 'x' }), false);
  assert.equal(completeBridgeMessage({ type: 'ffmpeg-exec-response', id: 'no-such-id' }), false);
});

test('ffmpegExec posts a request and resolves when the response arrives', async () => {
  let posted = null;
  const fakePost = async (msg) => { posted = msg; };
  const resultPromise = ffmpegExec('["-i","in","out"]', '[]', 'out', fakePost);
  assert.equal(posted.type, 'ffmpeg-exec-request');
  assert.equal(posted.args_json, '["-i","in","out"]');
  assert.equal(posted.output_name, 'out');
  completeBridgeMessage({
    type: 'ffmpeg-exec-response',
    id: posted.id,
    exit_code: 0,
    output_b64: 'QQ',
    log: '',
  });
  assert.deepEqual(await resultPromise, { exit_code: 0, output_b64: 'QQ', log: '' });
});
