const ABI = 1;
const STATUS = document.getElementById("status");
const CANVAS = document.getElementById("julibrot");
const FACTS = document.getElementById("facts-grid");
let RAF_PENDING = false;

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

function liveStatus(facts) {
  if (facts.completed_scene_id === null || facts.completed_scene_id === undefined) {
    return "waiting for first completed scene";
  }
  return `showing scene ${facts.completed_scene_id}: ${facts.refinement_level} ${facts.delivered_width}x${facts.delivered_height} at cap ${facts.delivered_iteration_cap}`;
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

function bindControls(api) {
  const refreshFacts = () => {
    const facts = JSON.parse(api.app_facts_json());
    renderFacts(facts);
    return facts;
  };
  const scheduleFrame = () => {
    if (RAF_PENDING) return;
    RAF_PENDING = true;
    requestAnimationFrame(nowMs => {
      RAF_PENDING = false;
      try {
        api.app_refresh(nowMs);
        showStatus(liveStatus(refreshFacts()));
        if (api.app_needs_refresh()) scheduleFrame();
      } catch (error) {
        fail(error);
        if (api.app_needs_refresh()) scheduleFrame();
      }
    });
  };
  const guarded = operation => {
    try {
      operation();
      api.app_request_frame();
      refreshFacts();
      scheduleFrame();
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
    showStatus("frame request queued");
  }));
  document.getElementById("measure").addEventListener("click", () => guarded(() => {
    api.app_request_measurement();
    showStatus("measurement request queued; no result claimed");
  }));
  refreshFacts();
  api.app_request_frame();
  scheduleFrame();
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
    showStatus("waiting for first completed scene");
    bindControls(api);
  } catch (error) {
    fail(error);
  }
}

void boot();
