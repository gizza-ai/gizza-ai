// Runs Transformers.js pipelines away from the page's main thread. The
// runtime itself is self-hosted; pinned model weights are fetched lazily and
// then served from Transformers.js' browser cache.

import { AutoModel, env, ImageProcessor, pipeline, RawImage } from "./transformers.min.js";

env.backends.onnx.wasm.wasmPaths = new URL("./", import.meta.url).href;
env.backends.onnx.wasm.proxy = false;
env.allowLocalModels = false;
env.allowRemoteModels = true;
env.useBrowserCache = true;

const loadedRunners = new Map();

const U2NET_PROCESSOR_CONFIG = {
  do_convert_rgb: true,
  do_normalize: true,
  do_pad: true,
  do_rescale: true,
  do_resize: true,
  keep_aspect_ratio: true,
  image_mean: [0.485, 0.456, 0.406],
  image_std: [0.229, 0.224, 0.225],
  pad_size: { width: 320, height: 320 },
  size: { longest_edge: 320 },
};

function errorMessage(error) {
  if (typeof error === "string") return error;
  if (error && typeof error.message === "string") return error.message;
  return "Unknown inference error";
}

function pipelineKey(model, device, dtype) {
  return [model.task, model.id, model.revision, device, dtype || "auto"].join("|");
}

async function assertDeviceAvailable(device) {
  if (device !== "webgpu") return;
  const gpu = self.navigator && self.navigator.gpu;
  if (!gpu || typeof gpu.requestAdapter !== "function") {
    throw new Error("WebGPU is not available in this browser");
  }
  const adapter = await gpu.requestAdapter();
  if (!adapter) {
    throw new Error("WebGPU is present but no usable GPU adapter is available");
  }
}

async function getRunner(id, model, device) {
  const dtype = model.dtypes && model.dtypes[device] ? model.dtypes[device] : undefined;
  const key = pipelineKey(model, device, dtype);
  let promise = loadedRunners.get(key);
  if (!promise) {
    const options = {
      revision: model.revision,
      device,
      progress_callback(progress) {
        postMessage({
          type: "progress",
          id,
          progress: { ...progress, stage: "download", device },
        });
      },
    };
    if (dtype) options.dtype = dtype;
    if (model.task === "u2net-background-removal") {
      promise = AutoModel.from_pretrained(model.id, options).then((loadedModel) => ({
        type: "u2net",
        model: loadedModel,
        processor: new ImageProcessor(U2NET_PROCESSOR_CONFIG),
      }));
    } else {
      promise = pipeline(model.task, model.id, options);
    }
    loadedRunners.set(key, promise);
    promise.catch(() => loadedRunners.delete(key));
  }
  return promise;
}

function alphaMetrics(image) {
  if (!image || image.channels !== 4 || !image.data) return null;
  const pixelCount = image.width * image.height;
  let alphaSum = 0;
  let foregroundPixels = 0;
  let backgroundPixels = 0;
  let opaquePixels = 0;
  let transparentPixels = 0;
  for (let index = 3; index < image.data.length; index += 4) {
    const alpha = image.data[index];
    alphaSum += alpha;
    if (alpha >= 16) foregroundPixels += 1;
    if (alpha <= 239) backgroundPixels += 1;
    if (alpha >= 239) opaquePixels += 1;
    if (alpha <= 16) transparentPixels += 1;
  }
  return {
    meanAlpha: alphaSum / pixelCount / 255,
    foregroundRatio: foregroundPixels / pixelCount,
    backgroundRatio: backgroundPixels / pixelCount,
    opaqueRatio: opaquePixels / pixelCount,
    transparentRatio: transparentPixels / pixelCount,
  };
}

async function runBackgroundRemoval(pipe, file) {
  const output = await pipe(file);
  const image = Array.isArray(output) ? output[0] : output;
  if (!image || typeof image.toBlob !== "function") {
    throw new Error("The background-removal model returned no image.");
  }
  return {
    blob: await image.toBlob("image/png"),
    metrics: alphaMetrics(image),
  };
}

async function runU2NetBackgroundRemoval(runner, file) {
  const image = await RawImage.fromBlob(file);
  const inputs = await runner.processor(image);
  const session = runner.model.sessions.model;
  const inputName = session.inputNames[0];
  const outputName = runner.model.config.output_composite || session.outputNames[0];
  const output = await runner.model({ [inputName]: inputs.pixel_values });
  const matte = output[outputName][0];
  const epsilon = 1e-5;
  if (matte.data.some((value) => value < -epsilon || value > 1 + epsilon)) {
    matte.sigmoid_();
  }

  let mask = await RawImage.fromTensor(matte.mul_(255).to("uint8"));
  const [reshapedHeight, reshapedWidth] = inputs.reshaped_input_sizes[0];
  if (mask.width !== reshapedWidth || mask.height !== reshapedHeight) {
    mask = await mask.crop([0, 0, reshapedWidth - 1, reshapedHeight - 1]);
  }
  mask = await mask.resize(image.width, image.height);
  const result = image.clone();
  result.putAlpha(mask);
  return {
    blob: await result.toBlob("image/png"),
    metrics: alphaMetrics(result),
  };
}

async function runTask(pipe, task, file) {
  switch (task) {
    case "background-removal":
      return runBackgroundRemoval(pipe, file);
    case "u2net-background-removal":
      return runU2NetBackgroundRemoval(pipe, file);
    default:
      throw new Error(`Unsupported browser model task: ${task}`);
  }
}

self.addEventListener("message", async (event) => {
  const request = event.data || {};
  if (request.type !== "run") return;
  const { id, model, file } = request;
  const devices = Array.isArray(model && model.devices) ? model.devices : [];
  const failures = [];

  for (let index = 0; index < devices.length; index += 1) {
    const device = devices[index];
    try {
      postMessage({
        type: "progress",
        id,
        progress: { stage: index ? "fallback" : "loading", device },
      });
      // Some browsers expose navigator.gpu even when requestAdapter() cannot
      // return a device (notably headless Chromium and disabled/blocked GPUs).
      // Probe before initializing ONNX: a failed WebGPU session can otherwise
      // poison backend selection and prevent the WASM retry from starting.
      await assertDeviceAvailable(device);
      const pipe = await getRunner(id, model, device);
      postMessage({ type: "progress", id, progress: { stage: "inference", device } });
      const result = await runTask(pipe, model.task, file);
      postMessage({
        type: "result",
        id,
        blob: result.blob,
        metrics: result.metrics,
        filename: "background-removed.png",
        backend: device,
      });
      return;
    } catch (error) {
      failures.push(`${device}: ${errorMessage(error)}`);
    }
  }

  postMessage({
    type: "error",
    id,
    error: failures.length
      ? `AI inference failed (${failures.join("; ")})`
      : "No compatible AI inference backend is available.",
  });
});
