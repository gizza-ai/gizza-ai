// Pure-Rust page with a file input: the generic non-ffmpeg driver only wires
// field inputs, so this small adapter reads the uploaded photo as base64 and
// then calls the wasm-bindgen export with the same argument order as
// web/src/lib.rs (image, sensitivity, min_radius, max_radius, max_regions).
let selectedBase64 = "";
let runSeq = 0;

function arrayBufferToBase64(buffer) {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export function setup(ctx) {
  const { cfg, mod, helpers, out } = ctx;
  const fileMeta = cfg.inputs.find((i) => i.source === "file");
  const fileInput = fileMeta ? document.getElementById(fileMeta.elementId) : null;
  const fieldInputs = cfg.inputs.filter((i) => i.source === "field");
  if (!fileInput) return false;

  function gatherArgs() {
    return cfg.inputs.map((input) =>
      input.source === "file"
        ? selectedBase64
        : helpers.readField(document.getElementById(input.elementId))
    );
  }

  function run() {
    const seq = ++runSeq;
    if (!selectedBase64) {
      out.classList.remove("error");
      out.textContent = "";
      return;
    }
    try {
      helpers.showResult(mod[cfg.export](...gatherArgs()));
    } catch (e) {
      if (seq !== runSeq) return;
      const msg = typeof e === "string" ? e : e && e.message ? e.message : "error";
      helpers.showError(msg);
    }
  }

  fileInput.addEventListener("change", async () => {
    const file = fileInput.files && fileInput.files[0];
    selectedBase64 = "";
    if (!file) {
      run();
      return;
    }
    const seq = ++runSeq;
    out.classList.remove("error");
    out.textContent = "Reading photo…";
    try {
      selectedBase64 = arrayBufferToBase64(await file.arrayBuffer());
      if (seq === runSeq) run();
    } catch (e) {
      if (seq === runSeq) helpers.showError("Could not read the selected photo.");
    }
  });

  fileInput.addEventListener("tool-file-reset", () => {
    selectedBase64 = "";
    runSeq++;
    out.classList.remove("error");
    out.textContent = "";
  });

  for (const input of fieldInputs) {
    const el = document.getElementById(input.elementId);
    if (el) {
      el.addEventListener("input", run);
      el.addEventListener("change", run);
    }
  }

  document.addEventListener("paste", (event) => {
    const files = Array.from((event.clipboardData && event.clipboardData.files) || []);
    const image = files.find((file) => String(file.type || "").startsWith("image/"));
    if (!image) return;
    event.preventDefault();
    const transfer = new DataTransfer();
    transfer.items.add(image);
    fileInput.files = transfer.files;
    fileInput.dispatchEvent(new Event("change"));
  });

  return true;
}
