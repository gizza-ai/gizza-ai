// Stand-in for the bundled transformers.min.js, so js/model-worker.test.js can
// run the real model-worker.js in Node. Every load is recorded in `fake.loads`
// and resolves when the test calls the load's `finish()`.
export const fake = { loads: [] };

export const env = { backends: { onnx: { wasm: {} } } };

export class BaseStreamer {}

export class InterruptableStoppingCriteria {
  interrupted = false;
  interrupt() {
    this.interrupted = true;
  }
}

function deferred() {
  let resolve;
  const promise = new Promise((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

function load(kind, id, options, value) {
  const d = deferred();
  const entry = { kind, id, options, finish: () => d.resolve(value) };
  fake.loads.push(entry);
  return d.promise;
}

const TOTAL = 4;

export const AutoProcessor = {
  from_pretrained(id, options) {
    const processor = async () => ({ input_ids: [1, 2, 3] });
    processor.num_image_tokens = TOTAL;
    return load("processor", id, options, processor);
  },
};

export const MultiModalityCausalLM = {
  from_pretrained(id, options) {
    return load("model", id, options, {
      async generate_images({ streamer, stopping_criteria }) {
        for (let i = 0; i <= TOTAL && !stopping_criteria.interrupted; i += 1) streamer.put();
        return [{ toBlob: async () => new Blob(["png"], { type: "image/png" }) }];
      },
    });
  },
};

export const AutoModel = { from_pretrained: () => Promise.reject(new Error("not faked")) };
export const pipeline = () => Promise.reject(new Error("not faked"));
export class ImageProcessor {}
export const RawImage = {};
