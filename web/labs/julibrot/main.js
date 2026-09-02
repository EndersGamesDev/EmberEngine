const ABI = 1;
const STATUS = document.getElementById("status");
const CANVAS = document.getElementById("julibrot");
const FACTS = document.getElementById("facts-grid");
let ORBIT_WORKER = null;

function timerProbe() {
  const readLimit = 4000000;
  const transitionTarget = 32;
  const deadlineMs = 500;
  const started = performance.now();
  let previous = started;
  let reads = 0;
  let transitions = 0;
  let zeroTransitions = 0;
  let quantum = null;
  while (reads < readLimit && transitions < transitionTarget) {
    const current = performance.now();
    reads += 1;
    const delta = current - previous;
    if (delta > 0) {
      transitions += 1;
      quantum = quantum === null ? delta : Math.min(quantum, delta);
    } else if (delta === 0) {
      zeroTransitions += 1;
    }
    previous = current;
    if (current - started >= deadlineMs) break;
  }
  return { quantum_ms: quantum, reads, positive_transitions: transitions, zero_transitions: zeroTransitions, wall_ms: Math.max(0, previous - started) };
}

function fail(error) {
  STATUS.className = "status failed";
  STATUS.textContent = String(error);
  console.error(error);
}

function showStatus(message) {
  STATUS.className = "status";
  STATUS.textContent = message;
}

function renderFacts(facts) {
  FACTS.replaceChildren();
  for (const [name, value] of Object.entries(facts)) {
    const term = document.createElement("dt");
    const detail = document.createElement("dd");
    term.textContent = name;
    detail.textContent = value === null ? "unavailable" : typeof value === "object" ? JSON.stringify(value) : String(value);
    FACTS.append(term, detail);
  }
}

function workerHandshake() {
  return new Promise((resolve, reject) => {
    const worker = new Worker("./worker.js?v=1", { type: "module", name: "julibrot-orbit" });
    const deadline = setTimeout(() => {
      worker.terminate();
      reject(new Error("VersionSkew: worker handshake exceeded 4000 ms"));
    }, 4000);
    worker.onmessage = event => {
      const message = event.data;
      if (message?.kind === "WorkerReady" && message.version === ABI) {
        worker.postMessage({ kind: "AbiProbe", version: ABI });
      } else if (message?.kind === "AbiAccepted" && message.version === ABI) {
        clearTimeout(deadline);
        resolve(worker);
      } else if (message?.kind === "VersionSkew" || message?.kind === "ChannelError") {
        clearTimeout(deadline);
        worker.terminate();
        reject(new Error(`VersionSkew: ${JSON.stringify(message)}`));
      }
    };
    worker.onerror = event => {
      clearTimeout(deadline);
      worker.terminate();
      reject(new Error(`VersionSkew: worker load failed: ${event.message}`));
    };
  });
}

function bindControls(api) {
  const refreshFacts = () => renderFacts(JSON.parse(api.app_facts_json()));
  const guarded = operation => {
    try {
      operation();
      refreshFacts();
    } catch (error) {
      fail(error);
    }
  };
  const angles = () => guarded(() => api.app_set_plane_angles(Number(document.getElementById("theta-1").value), Number(document.getElementById("theta-2").value)));
  document.getElementById("theta-1").addEventListener("input", angles);
  document.getElementById("theta-2").addEventListener("input", angles);
  document.getElementById("view").addEventListener("change", event => guarded(() => api.app_set_view(Number(event.target.value))));
  document.getElementById("palette").addEventListener("change", event => guarded(() => api.app_set_palette(Number(event.target.value))));
  document.getElementById("iteration-cap").addEventListener("change", event => guarded(() => api.app_set_iteration_cap(Number(event.target.value))));
  document.getElementById("preset").addEventListener("change", event => guarded(() => api.app_set_preset(Number(event.target.value), Number(document.getElementById("julia-re").value), Number(document.getElementById("julia-im").value))));
  for (const id of ["julia-re", "julia-im"]) {
    document.getElementById(id).addEventListener("change", () => {
      if (document.getElementById("preset").value === "1") {
        guarded(() => api.app_set_preset(1, Number(document.getElementById("julia-re").value), Number(document.getElementById("julia-im").value)));
      }
    });
  }
  CANVAS.addEventListener("wheel", event => {
    event.preventDefault();
    const bounds = CANVAS.getBoundingClientRect();
    const x = event.clientX - bounds.left - bounds.width / 2;
    const yUp = bounds.height / 2 - (event.clientY - bounds.top);
    guarded(() => api.app_wheel_zoom(-event.deltaY * 0.002, x, yUp));
  }, { passive: false });
  let pointer = null;
  CANVAS.addEventListener("pointerdown", event => {
    pointer = [event.pointerId, event.clientX, event.clientY];
    CANVAS.setPointerCapture(event.pointerId);
  });
  CANVAS.addEventListener("pointermove", event => {
    if (!pointer || pointer[0] !== event.pointerId) return;
    const [id, x, y] = pointer;
    pointer = [id, event.clientX, event.clientY];
    guarded(() => api.app_drag_pan(event.clientX - x, event.clientY - y));
  });
  CANVAS.addEventListener("pointerup", () => { pointer = null; });
  document.getElementById("one-frame").addEventListener("click", () => guarded(() => {
    api.app_request_frame();
    showStatus("frame request queued; not yet submitted");
  }));
  document.getElementById("measure").addEventListener("click", () => guarded(() => {
    api.app_request_measurement();
    showStatus("measurement request queued; no result claimed");
  }));
  refreshFacts();
}

async function artifactBytes(path) {
  const response = await fetch(path, { cache: "force-cache" });
  if (!response.ok) throw new Error(`artifact fetch ${path} returned ${response.status}`);
  return (await response.arrayBuffer()).byteLength;
}

async function boot() {
  try {
    const api = await import("./pkg/ember_lab_julibrot.js?v=1");
    await api.default("./pkg/ember_lab_julibrot_bg.wasm?v=1");
    const mainVersion = api.julibrot_abi_version();
    if (mainVersion !== ABI) throw new Error(`VersionSkew: main wasm ${mainVersion}, loader ${ABI}`);
    ORBIT_WORKER = await workerHandshake();
    if (!ORBIT_WORKER) throw new Error("VersionSkew: worker did not publish ownership");
    await api.start_julibrot("julibrot", "status");
    const facts = JSON.parse(api.app_facts_json());
    const timer = timerProbe();
    facts.timer_quantum_ms = timer.quantum_ms;
    facts.timing_status = timer.quantum_ms === null ? "unavailable: timer exposed no positive transition" : "requires visible replay";
    facts.timer_probe = timer;
    facts.wasm_bundle_bytes = await artifactBytes("./pkg/ember_lab_julibrot_bg.wasm?v=1");
    facts.javascript_bundle_bytes = await artifactBytes("./pkg/ember_lab_julibrot.js?v=1");
    facts.wasm_instance_count = 2;
    renderFacts(facts);
    bindControls(api);
    showStatus("waiting for first completed scene");
  } catch (error) {
    fail(error);
  }
}

void boot();
