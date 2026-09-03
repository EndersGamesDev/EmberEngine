const ABI = 2;

async function boot() {
  try {
    const module = await import("./pkg/ember_lab_julibrot.js?v=1");
    await module.default("./pkg/ember_lab_julibrot_bg.wasm?v=1");
    const delivered = module.worker_main(ABI);
    if (delivered !== ABI) {
      throw new Error(`VersionSkew: worker wasm ${delivered}, loader ${ABI}`);
    }
    self.postMessage({ kind: "WorkerReady", version: delivered });
    self.onmessage = event => {
      if (event.data?.kind === "AbiProbe" && event.data.version === ABI) {
        self.postMessage({ kind: "AbiAccepted", version: ABI });
      } else {
        self.postMessage({ kind: "VersionSkew", expected: ABI, actual: event.data?.version ?? null });
      }
    };
  } catch (error) {
    self.postMessage({ kind: "ChannelError", code: "VersionSkew", message: String(error) });
  }
}

void boot();
