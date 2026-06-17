import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ffmpegExec, completeBridgeMessage } from './ffmpeg-bridge.js';
import { runFfmpegRequest } from './ffmpeg-engine.js';

// Wire the SW-side bridge to the page-side engine with fakes for the two
// postMessage hops and a fake ffmpeg, proving request→response correlation end
// to end without a browser or real ffmpeg.
test('bridge request round-trips through the engine and resolves', async () => {
  const fakeFfmpeg = async (argsJson, inputsJson, outputName) => {
    assert.equal(outputName, 'out.png');
    return { exit_code: 0, output_b64: 'R1JBWQ', log: 'frames' };
  };

  // SW→page hop: when the bridge "posts" a request, run the engine on it…
  const post = async (request) => {
    const resp = await runFfmpegRequest(request, fakeFfmpeg);
    // …page→SW hop: feed the engine's response back into the bridge.
    completeBridgeMessage({ type: 'ffmpeg-exec-response', id: request.id, ...resp });
  };

  const result = await ffmpegExec(
    '["-i","in.png","-vf","format=gray","out.png"]',
    '[{"name":"in.png","bytes_b64":"AAAA"}]',
    'out.png',
    post,
  );
  assert.deepEqual(result, { exit_code: 0, output_b64: 'R1JBWQ', log: 'frames' });
});
