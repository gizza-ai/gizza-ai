import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { parseHTML } from 'linkedom';
import {
    addPending,
    removePending,
    clearPending,
    getPending,
    renderChips,
    _resetForTests,
} from '../site/pending.js';

function setupDom() {
    const { document } = parseHTML(
        '<!doctype html><div id="upload-chips" class="upload-chips empty"></div>',
    );
    globalThis.document = document;
    return document.querySelector('#upload-chips');
}

beforeEach(() => {
    _resetForTests();
});

function fakeFile(name, type, size = 100) {
    return {
        name,
        type,
        size,
    };
}

test('addPending with image creates chip with image-type label', () => {
    const strip = setupDom();
    const r = addPending(fakeFile('cat.png', 'image/png'));
    assert.equal(r.ok, true);
    assert.equal(r.entry.id, 'upload_1');
    renderChips(strip);
    assert.equal(strip.classList.contains('empty'), false);
    const chip = strip.querySelector('.chip');
    assert.ok(chip, 'chip rendered');
    assert.ok(chip.textContent.includes('cat.png'));
    assert.ok(chip.querySelector('button.remove'));
});

test('addPending with video creates chip with filename text', () => {
    const strip = setupDom();
    addPending(fakeFile('clip.mp4', 'video/mp4'));
    renderChips(strip);
    const chip = strip.querySelector('.chip');
    assert.ok(chip.textContent.includes('clip.mp4'));
});

test('addPending rejects non-image non-video', () => {
    const strip = setupDom();
    const r = addPending(fakeFile('a.pdf', 'application/pdf'));
    assert.equal(r.ok, false);
    assert.match(r.error, /image|video/i);
    renderChips(strip);
    assert.equal(getPending().length, 0);
});

test('addPending rejects oversize file (>10 MiB)', () => {
    const strip = setupDom();
    const r = addPending(fakeFile('big.png', 'image/png', 10 * 1024 * 1024 + 1));
    assert.equal(r.ok, false);
    assert.match(r.error, /too large|10 MiB/i);
    renderChips(strip);
    assert.equal(getPending().length, 0);
});

test('removePending clears chip and pending entry; nextUploadId is monotonic', () => {
    const strip = setupDom();
    const a = addPending(fakeFile('a.png', 'image/png')).entry;
    const b = addPending(fakeFile('b.png', 'image/png')).entry;
    assert.equal(a.id, 'upload_1');
    assert.equal(b.id, 'upload_2');

    removePending(a.id);
    renderChips(strip);
    const ids = [...strip.querySelectorAll('.chip')].map((c) =>
        c.getAttribute('data-id'),
    );
    assert.deepEqual(ids, ['upload_2']);

    // After remove, the next id is upload_3 (monotonic — does NOT reuse upload_1).
    const c = addPending(fakeFile('c.png', 'image/png')).entry;
    assert.equal(c.id, 'upload_3');
});

test('clearPending empties the strip and pending list', () => {
    const strip = setupDom();
    addPending(fakeFile('a.png', 'image/png'));
    addPending(fakeFile('b.png', 'image/png'));
    assert.equal(getPending().length, 2);
    clearPending();
    renderChips(strip);
    assert.equal(getPending().length, 0);
    assert.equal(strip.classList.contains('empty'), true);
    assert.equal(strip.children.length, 0);
});
