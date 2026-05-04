import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseHTML } from 'linkedom';
import { renderToolAttachment } from '../site/render.js';

function makeRow() {
    const { document } = parseHTML('<!doctype html><div class="tool-call"></div>');
    globalThis.document = document; // renderToolAttachment uses document.createElement
    return document.querySelector('.tool-call');
}

test('renders <img> for image mime', () => {
    const row = makeRow();
    renderToolAttachment(row, {
        data_url: 'data:image/png;base64,AAA',
        mime: 'image/png',
        filename: 'cat.png',
    });
    const img = row.querySelector('img');
    assert.ok(img, 'img should be appended');
    assert.equal(img.getAttribute('src'), 'data:image/png;base64,AAA');
    assert.equal(img.getAttribute('alt'), 'cat.png');
    assert.equal(img.getAttribute('class'), 'tool-attachment');
});

test('renders <video controls> for video mime', () => {
    const row = makeRow();
    renderToolAttachment(row, {
        data_url: 'data:video/mp4;base64,BBB',
        mime: 'video/mp4',
        filename: 'clip.mp4',
    });
    const video = row.querySelector('video');
    assert.ok(video, 'video should be appended');
    assert.ok(video.hasAttribute('controls'));
    assert.equal(video.getAttribute('aria-label'), 'clip.mp4');
});

test('renders nothing for unknown mime', () => {
    const row = makeRow();
    renderToolAttachment(row, {
        data_url: 'data:application/pdf;base64,CCC',
        mime: 'application/pdf',
    });
    assert.equal(row.children.length, 0);
});

test('renders nothing when data_url is not a data: URL', () => {
    const row = makeRow();
    renderToolAttachment(row, {
        data_url: 'https://evil.example/x.png',
        mime: 'image/png',
    });
    assert.equal(row.children.length, 0);
});

test('escapes filename in alt attribute (no <script> in DOM)', () => {
    const row = makeRow();
    renderToolAttachment(row, {
        data_url: 'data:image/png;base64,AAA',
        mime: 'image/png',
        filename: '<script>alert(1)</script>',
    });
    assert.equal(row.querySelector('script'), null, 'no live <script> element');
    const img = row.querySelector('img');
    assert.equal(
        img.getAttribute('alt'),
        '<script>alert(1)</script>',
        'literal string in alt — DOM API handles escaping',
    );
});

test('returns null and renders nothing when for_ui is null', () => {
    const row = makeRow();
    const result = renderToolAttachment(row, null);
    assert.equal(result, null);
    assert.equal(row.children.length, 0);
});
