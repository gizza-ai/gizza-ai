import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = JSON.parse(fs.readFileSync(path.join(here, 'fixtures/webllm-model-list.json'), 'utf8'));

const moduleUrl = path.resolve(here, '../../site/model-picker.js');
const { groupModels } = await import(moduleUrl);

test('groupModels: collapses Llama-3.2-1B variants into one base entry', () => {
    const groups = groupModels(fixture);
    const llama1b = groups.find((g) => g.base_id === 'Llama-3.2-1B-Instruct');
    assert.ok(llama1b, 'expected a Llama-3.2-1B-Instruct group');
    // Fixture should contain at least q4f32_1, q4f16_1, q0f32, q0f16 = 4 variants
    assert.ok(llama1b.variants.length >= 2, `expected >=2 variants, got ${llama1b.variants.length}`);
    // Each variant must keep its full WebLLM model_id for engine handoff
    for (const v of llama1b.variants) {
        assert.ok(v.id.startsWith('Llama-3.2-1B-Instruct-q'), `unexpected variant id ${v.id}`);
    }
});

test('groupModels: family detection', () => {
    const groups = groupModels(fixture);
    const llama = groups.find((g) => g.base_id === 'Llama-3.2-1B-Instruct');
    const qwen = groups.find((g) => g.base_id.startsWith('Qwen2.5-1.5B-Instruct'));
    const phi = groups.find((g) => g.base_id.startsWith('Phi-3.5-mini-instruct'));
    assert.equal(llama.family, 'Meta');
    assert.equal(qwen.family, 'Alibaba');
    assert.equal(phi.family, 'Microsoft');
});

test('groupModels: no group contains a quantization suffix in its base_id', () => {
    const groups = groupModels(fixture);
    for (const g of groups) {
        assert.doesNotMatch(g.base_id, /-q\d+f\d+(_\d+)?-MLC$/, `base_id leaked variant suffix: ${g.base_id}`);
        assert.doesNotMatch(g.base_id, /-MLC(-\d+k)?$/, `base_id leaked -MLC suffix: ${g.base_id}`);
    }
});
