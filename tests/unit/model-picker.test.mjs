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

test('groupModels: maps q4f16 → Balanced, q4f32 → Standard, q0f16 → High quality', () => {
    const groups = groupModels(fixture);
    const llama = groups.find((g) => g.base_id === 'Llama-3.2-1B-Instruct');
    const labels = Object.fromEntries(llama.variants.map((v) => [v.quant, v.label]));
    // The Llama-3.2-1B-Instruct fixture entries include q4f16_1, q4f32_1, q0f16, q0f32 — all four must map.
    assert.equal(labels['q4f16_1'], 'Balanced');
    assert.equal(labels['q4f32_1'], 'Standard');
    assert.equal(labels['q0f16'], 'High quality');
    assert.equal(labels['q0f32'], 'High quality');
});

test('groupModels: tool-capable detection on Qwen2.5/Hermes families', () => {
    const groups = groupModels(fixture);
    const qwen = groups.find((g) => g.base_id === 'Qwen2.5-1.5B-Instruct');
    const hermes = groups.find((g) => g.base_id === 'Hermes-2-Pro-Llama-3-8B');
    const phi = groups.find((g) => g.base_id === 'Phi-3.5-mini-instruct');
    assert.ok(qwen, 'fixture should contain Qwen2.5-1.5B-Instruct');
    assert.ok(hermes, 'fixture should contain Hermes-2-Pro-Llama-3-8B');
    assert.ok(phi, 'fixture should contain Phi-3.5-mini-instruct');
    assert.equal(qwen.has_tools, true, 'Qwen2.5 should be tool-capable');
    assert.equal(hermes.has_tools, true, 'Hermes-2-Pro should be tool-capable');
    assert.equal(phi.has_tools, false, 'Phi-3.5 should NOT be tool-capable');
});

test('groupModels: vision flag on Phi-3.5-vision', () => {
    const groups = groupModels(fixture);
    const vision = groups.find((g) => g.base_id === 'Phi-3.5-vision-instruct');
    assert.ok(vision, 'fixture should contain Phi-3.5-vision-instruct');
    assert.equal(vision.has_vision, true);
});

test('groupModels: variants sorted by quality tier (q0 before q4f32 before q4f16)', () => {
    const groups = groupModels(fixture);
    for (const g of groups) {
        const quants = g.variants.map((v) => v.quant);
        const order = ['q0f32', 'q0f16', 'q4f32_1', 'q4f16_1', 'q3f16_1'];
        let lastIdx = -1;
        for (const q of quants) {
            const idx = order.indexOf(q);
            if (idx === -1) continue; // unknown quants ride at the end, skip
            assert.ok(idx >= lastIdx, `quant out of order in ${g.base_id}: ${quants.join(',')}`);
            lastIdx = idx;
        }
    }
});

test('groupModels: empty input returns empty array', () => {
    assert.deepEqual(groupModels([]), []);
});

test('groupModels: skips entries with no model_id', () => {
    const result = groupModels([{ model_id: null }, { foo: 'bar' }, ...fixture.slice(0, 2)]);
    assert.equal(result.length >= 1, true);
});
