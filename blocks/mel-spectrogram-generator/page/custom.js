// mel-spectrogram-generator page module — the web export returns JSON with a
// PNG data URL plus a text summary. Render the PNG in the shared media slot.
//
// This is also a pure-Rust page with a file input: the generic non-ffmpeg
// driver only wires *field* inputs, so setup() below reads the uploaded audio
// as base64 and calls the export with the same argument order as web/src/lib.rs.

let selectedBase64 = '';
let runSeq = 0;

function arrayBufferToBase64(buffer) {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

function clearOutput(out) {
  const media = document.getElementById('tool-output-media');
  const dl = document.getElementById('tool-output-download');
  const copyImage = document.getElementById('tool-copy-image');
  if (media) media.hidden = true;
  if (dl) dl.hidden = true;
  if (copyImage) copyImage.hidden = true;
  out.classList.remove('error');
  out.textContent = '';
}

export function setup(ctx) {
  const { cfg, mod, helpers, out } = ctx;
  const fileMeta = cfg.inputs.find((i) => i.source === 'file');
  const fileInput = fileMeta ? document.getElementById(fileMeta.elementId) : null;
  if (!fileInput) return false;

  function gatherArgs() {
    return cfg.inputs.map((input) =>
      input.source === 'file'
        ? selectedBase64
        : helpers.readField(document.getElementById(input.elementId))
    );
  }

  function run() {
    const seq = ++runSeq;
    if (!selectedBase64) {
      clearOutput(out);
      return;
    }
    try {
      helpers.showResult(mod[cfg.export](...gatherArgs()));
    } catch (e) {
      if (seq !== runSeq) return;
      helpers.showError(typeof e === 'string' ? e : (e && e.message) || 'error');
    }
  }

  fileInput.addEventListener('change', async () => {
    const file = fileInput.files && fileInput.files[0];
    selectedBase64 = '';
    if (!file) {
      run();
      return;
    }
    const seq = ++runSeq;
    clearOutput(out);
    out.textContent = 'Reading audio…';
    try {
      const b64 = arrayBufferToBase64(await file.arrayBuffer());
      if (seq !== runSeq) return;
      selectedBase64 = b64;
      run();
    } catch {
      if (seq === runSeq) helpers.showError('Could not read the selected audio file.');
    }
  });

  fileInput.addEventListener('tool-file-reset', () => {
    selectedBase64 = '';
    runSeq++;
    clearOutput(out);
  });

  for (const input of cfg.inputs) {
    if (input.source !== 'field') continue;
    const el = document.getElementById(input.elementId);
    if (el) {
      el.addEventListener('input', run);
      el.addEventListener('change', run);
    }
  }

  // Paste-to-upload: Ctrl/Cmd+V a copied audio file anywhere on the page.
  document.addEventListener('paste', (event) => {
    const files = Array.from((event.clipboardData && event.clipboardData.files) || []);
    const audio = files.find((file) => String(file.type || '').startsWith('audio/'));
    if (!audio) return;
    event.preventDefault();
    const transfer = new DataTransfer();
    transfer.items.add(audio);
    fileInput.files = transfer.files;
    fileInput.dispatchEvent(new Event('change'));
  });

  clearOutput(out);
  return true;
}

function approxBytes(dataUrl) {
  const i = dataUrl.indexOf('base64,');
  if (i < 0) return 0;
  const b64 = dataUrl.slice(i + 7);
  const padding = b64.endsWith('==') ? 2 : b64.endsWith('=') ? 1 : 0;
  return Math.max(0, Math.floor((b64.length * 3) / 4) - padding);
}

function humanSize(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function renderResult(value, ctx) {
  const { out } = ctx;
  const media = document.getElementById('tool-output-media');
  const dl = document.getElementById('tool-output-download');
  const copyImage = document.getElementById('tool-copy-image');
  out.classList.remove('error');

  let result;
  try {
    result = JSON.parse(String(value));
  } catch {
    out.textContent = String(value || 'Upload audio to render a mel spectrogram.');
    if (media) media.hidden = true;
    if (dl) dl.hidden = true;
    if (copyImage) copyImage.hidden = true;
    return true;
  }

  if (!result.dataUrl || !String(result.dataUrl).startsWith('data:image/png;base64,')) {
    out.textContent = result.summary || 'No PNG was returned.';
    if (media) media.hidden = true;
    if (dl) dl.hidden = true;
    if (copyImage) copyImage.hidden = true;
    return true;
  }

  out.textContent = result.summary || `Rendered ${result.width}×${result.height} PNG (${humanSize(approxBytes(result.dataUrl))}).`;
  if (media) {
    media.src = result.dataUrl;
    media.hidden = false;
  }
  if (dl) {
    dl.href = result.dataUrl;
    dl.download = 'mel-spectrogram.png';
    dl.textContent = 'Download PNG';
    dl.title = 'Download mel-spectrogram.png';
    dl.hidden = false;
  }
  if (copyImage) copyImage.hidden = false;
  return true;
}

export function renderError(message, ctx) {
  const { out } = ctx;
  const media = document.getElementById('tool-output-media');
  const dl = document.getElementById('tool-output-download');
  const copyImage = document.getElementById('tool-copy-image');
  if (media) media.hidden = true;
  if (dl) dl.hidden = true;
  if (copyImage) copyImage.hidden = true;
  out.classList.add('error');
  out.textContent = message;
  return true;
}
