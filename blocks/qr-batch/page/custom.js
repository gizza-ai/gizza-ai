// qr-batch returns a binary ZIP as a data: URL. Render a download button and
// inspect index.csv in-browser so the page can show real contents, not raw base64.

function approxBytes(dataUrl) {
  const i = dataUrl.indexOf('base64,');
  if (i < 0) return 0;
  const b64 = dataUrl.slice(i + 7);
  const padding = b64.endsWith('==') ? 2 : b64.endsWith('=') ? 1 : 0;
  return Math.max(0, Math.floor((b64.length * 3) / 4) - padding);
}

function humanSize(n) {
  if (n < 1024) return n + ' B';
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB';
  return (n / (1024 * 1024)).toFixed(1) + ' MB';
}

async function readIndex(dataUrl) {
  try {
    const buf = new Uint8Array(await (await fetch(dataUrl)).arrayBuffer());
    const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    const latin1 = new TextDecoder('latin1');
    let i = 0;
    while (i + 30 <= buf.length && dv.getUint32(i, true) === 0x04034b50) {
      const method = dv.getUint16(i + 8, true);
      const compSize = dv.getUint32(i + 18, true);
      const nameLen = dv.getUint16(i + 26, true);
      const extraLen = dv.getUint16(i + 28, true);
      const nameStart = i + 30;
      const name = latin1.decode(buf.subarray(nameStart, nameStart + nameLen));
      const dataStart = nameStart + nameLen + extraLen;
      const comp = buf.subarray(dataStart, dataStart + compSize);
      if (name === 'index.csv') {
        if (method === 0) return new TextDecoder().decode(comp);
        const stream = new Response(comp).body.pipeThrough(new DecompressionStream('deflate-raw'));
        return new TextDecoder().decode(await new Response(stream).arrayBuffer());
      }
      i = dataStart + compSize;
    }
  } catch (_) {
    return '';
  }
  return '';
}

export async function renderResult(value, ctx) {
  const { out } = ctx;
  const dl = document.getElementById('tool-output-download');
  out.classList.remove('error');
  if (!value || !String(value).startsWith('data:application/zip;base64,')) {
    out.textContent = value ? String(value) : 'Paste rows above — your ZIP download will appear here.';
    if (dl) dl.hidden = true;
    return true;
  }
  const index = await readIndex(String(value));
  const okRows = (index.match(/,ok\n/g) || []).length;
  const errorRows = (index.match(/,error \(row /g) || []).length;
  out.textContent = `ZIP ready (${humanSize(approxBytes(String(value)))}). ${okRows} generated row(s)` +
    (errorRows ? `, ${errorRows} row error(s)` : '') + '. Click “Download ZIP”.';
  if (dl) {
    dl.href = String(value);
    dl.setAttribute('download', 'qr-batch.zip');
    dl.textContent = 'Download ZIP';
    dl.title = 'Download qr-batch.zip';
    dl.hidden = false;
  }
  return true;
}

export function renderError(message, ctx) {
  const { out } = ctx;
  const dl = document.getElementById('tool-output-download');
  if (dl) dl.hidden = true;
  out.classList.add('error');
  out.textContent = message;
  return true;
}
