const ABI = 1;
const STATUS = document.getElementById("status");
const CANVAS = document.getElementById("julibrot");
const FACTS = document.getElementById("facts-grid");
const TARGET = document.getElementById("target");
const RUBBER = document.getElementById("rubber");
const MORPH = document.getElementById("morph");
const BOXES = { a: null, b: null };
// The frame loop must not be able to latch. A `requestAnimationFrame` queued while the tab or the
// pane is not painting is never called, so a flag set at schedule time and cleared only inside that
// callback outlives the frame it was guarding: every later schedule returns immediately while
// `app_needs_refresh` still answers true, and the picture sleeps on a transient frame until a
// control move happens to reach `guarded`. The flag is therefore cleared on frame entry, retired
// whenever the page becomes visible or focused again, and backed by a low-rate timer that runs the
// turn the animation callback did not. Whichever path arrives first for a schedule clears the flag
// and runs it, so a healthy `requestAnimationFrame` and the timer can never both drive one turn.
const FRAME_FALLBACK_MS = 250;
let RAF_PENDING = false;
let FRAME_TICKET = 0;
let FRAME_FALLBACK_TIMER = null;
const FRAME_COUNTS = { schedules: 0, raf: 0, fallback: 0, latch_clears: 0, wakeups: 0 };
let STORAGE_STATUS = "available";

// One key per box. Every access is wrapped because a private window, cleared site data, or a
// browser told to refuse storage makes the accessor itself throw rather than return nothing.
const STORAGE_KEY = box => `julibrot.view.${box}`;

function readStoredView(box) {
  try {
    const text = localStorage.getItem(STORAGE_KEY(box));
    if (!text) return null;
    const row = JSON.parse(text);
    if (!row || typeof row !== "object" || !row.centre || !Array.isArray(row.centre.coords)) return null;
    if (row.centre.coords.length !== 4) return null;
    return row;
  } catch (error) {
    STORAGE_STATUS = `unavailable: ${error && error.name ? error.name : "refused"}`;
    return null;
  }
}

function writeStoredView(box, row) {
  try {
    localStorage.setItem(STORAGE_KEY(box), JSON.stringify(row));
  } catch (error) {
    STORAGE_STATUS = `unavailable: ${error && error.name ? error.name : "refused"}`;
  }
}

function centreDigits(zoomLog2) {
  return Math.min(40, Math.max(3, Math.ceil(Number(zoomLog2) * Math.LOG10E * Math.LN2) + 3));
}

function describeRow(name, row) {
  if (!row) return `${name} is empty`;
  const digits = centreDigits(row.zoom_log2);
  const angles = [row.theta_1, row.theta_2, row.view_theta_1, row.view_theta_2, row.camera_yaw, row.camera_pitch]
    .map(value => Number(value).toFixed(3)).join(" ");
  const origin = row.origin.map(value => Number(value).toFixed(3)).join(" ");
  const centre = row.centre_f64.map(value => Number(value).toFixed(digits)).join(" ");
  return `${name}: angles ${angles} · origin ${origin} · zoom_log2 ${Number(row.zoom_log2).toFixed(3)} · centre ${centre}`;
}

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

function surviving(facts) {
  const count = facts.transient_fence_refusals;
  if (!count) return "";
  return ` — survived ${count} transient fence refusal${count === 1 ? "" : "s"}, last ${facts.last_transient_refusal}`;
}

function liveStatus(facts) {
  if (facts.completed_scene_id === null || facts.completed_scene_id === undefined) {
    return `waiting for first completed scene${surviving(facts)}`;
  }
  return `showing scene ${facts.completed_scene_id}: ${facts.refinement_level} ${facts.delivered_width}x${facts.delivered_height} at cap ${facts.delivered_iteration_cap}${surviving(facts)}`;
}

// Page-owned facts: the boxes live in this document and in localStorage, and `t` is where the user
// currently is between two saved rows rather than a place the picture can be, so neither is app
// state and neither is saved.
function pageFacts() {
  return {
    view_box_a_held: BOXES.a !== null,
    view_box_b_held: BOXES.b !== null,
    morph_t: MORPH.disabled ? null : Number(MORPH.value),
    view_box_storage: STORAGE_STATUS,
    frame_schedules: FRAME_COUNTS.schedules,
    frames_from_raf: FRAME_COUNTS.raf,
    frames_from_fallback: FRAME_COUNTS.fallback,
    frame_latch_clears: FRAME_COUNTS.latch_clears,
    frame_loop_wakeups: FRAME_COUNTS.wakeups,
  };
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

const NUMBER = id => Number(document.getElementById(id).value);
const SET = (id, value) => { document.getElementById(id).value = String(value); };

// Every origin slider is paired with a number box on the same value; writing one writes the other,
// so precision and dragging are two views of one control rather than two controls.
const ORIGIN = [
  ["origin-z-re", "z_re"],
  ["origin-z-im", "z_im"],
  ["origin-c-re", "c_re"],
  ["origin-c-im", "c_im"],
];

function bindControls(api) {
  const refreshFacts = () => {
    const facts = Object.assign(JSON.parse(api.app_facts_json()), pageFacts());
    renderFacts(facts);
    return facts;
  };
  // The app answers false only once its loop has stopped for a typed cause, so a refusal it
  // survived still schedules the next turn; a throw from the query itself is treated as stopped.
  const stillTurning = () => {
    try {
      return api.app_needs_refresh();
    } catch (error) {
      console.error(error);
      return false;
    }
  };
  // One turn per schedule, whichever path arrives with it. A stale animation callback carries an
  // older ticket, and the timer behind a callback that already ran finds the flag clear, so neither
  // can run a turn twice; the flag is cleared before the body so the turn may schedule the next.
  const runFrame = (ticket, nowMs, viaFallback) => {
    if (!RAF_PENDING || ticket !== FRAME_TICKET) return;
    RAF_PENDING = false;
    if (viaFallback) FRAME_COUNTS.fallback += 1;
    else FRAME_COUNTS.raf += 1;
    try {
      api.app_refresh(nowMs);
      const facts = refreshFacts();
      if (facts.loop_stopped_reason) fail(facts.loop_stopped_reason);
      else showStatus(liveStatus(facts));
      if (stillTurning()) scheduleFrame();
    } catch (error) {
      fail(error);
      try {
        renderFacts(JSON.parse(api.app_facts_json()));
      } catch (factsError) {
        console.error(factsError);
      }
      if (stillTurning()) scheduleFrame();
    }
  };
  const scheduleFrame = () => {
    if (RAF_PENDING) return;
    RAF_PENDING = true;
    FRAME_COUNTS.schedules += 1;
    const ticket = (FRAME_TICKET += 1);
    requestAnimationFrame(nowMs => runFrame(ticket, nowMs, false));
    if (FRAME_FALLBACK_TIMER !== null) clearTimeout(FRAME_FALLBACK_TIMER);
    // The low rate is the point: it is a floor under a loop the browser has stopped painting for,
    // not a second animation clock. Arriving after a healthy callback it does nothing at all, and
    // arriving with nothing due it still releases the flag, because the callback it was waiting on
    // may never be called and the next control move must be able to schedule.
    FRAME_FALLBACK_TIMER = setTimeout(() => {
      FRAME_FALLBACK_TIMER = null;
      if (!RAF_PENDING || ticket !== FRAME_TICKET) return;
      if (stillTurning()) {
        runFrame(ticket, performance.now(), true);
      } else {
        RAF_PENDING = false;
        FRAME_COUNTS.latch_clears += 1;
      }
    }, FRAME_FALLBACK_MS);
  };
  // Returning to a page the browser stopped painting for retires the schedule outright rather than
  // waiting on a callback that may never be called, and asks for a turn if one is still due.
  const wakeFrameLoop = () => {
    FRAME_COUNTS.wakeups += 1;
    RAF_PENDING = false;
    FRAME_TICKET += 1;
    if (stillTurning()) scheduleFrame();
  };
  document.addEventListener("visibilitychange", () => { if (!document.hidden) wakeFrameLoop(); });
  window.addEventListener("pageshow", wakeFrameLoop);
  window.addEventListener("focus", wakeFrameLoop);
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
  // One function per group of controls. Listeners call these, and so does the preset row, so a
  // preset cannot reach the worker by a path a user's own movement does not take.
  const APPLY = {
    plane: () => api.app_set_plane_angles(NUMBER("theta-1"), NUMBER("theta-2")),
    origin: () => api.app_set_plane_origin(NUMBER("origin-z-re"), NUMBER("origin-z-im"), NUMBER("origin-c-re"), NUMBER("origin-c-im")),
    view: () => api.app_set_view_angles(NUMBER("view-theta-1"), NUMBER("view-theta-2")),
    camera: () => api.app_set_camera(NUMBER("camera-yaw"), NUMBER("camera-pitch")),
    height: () => api.app_set_height(NUMBER("height")),
    distances: () => api.app_set_distances(NUMBER("distance-five"), NUMBER("distance-four")),
    // Last, because the origin handler resets the zoom to zero: a row applied in this order ends
    // at the scale it names rather than at the scale the origin change forced.
    scale: () => api.app_set_scale(NUMBER("scale")),
  };
  for (const id of ["theta-1", "theta-2"]) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.plane));
  }
  for (const id of ["view-theta-1", "view-theta-2"]) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.view));
  }
  for (const id of ["camera-yaw", "camera-pitch"]) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.camera));
  }
  document.getElementById("height").addEventListener("input", () => guarded(APPLY.height));
  for (const id of ["distance-five", "distance-four"]) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.distances));
  }
  document.getElementById("scale").addEventListener("input", () => guarded(APPLY.scale));
  // The origin is MAIN work — a new reference orbit — so it lands on release rather than on every
  // pixel of a drag; the paired number box mirrors it either way.
  for (const [id] of ORIGIN) {
    document.getElementById(id).addEventListener("change", () => {
      SET(`${id}-number`, NUMBER(id));
      guarded(APPLY.origin);
    });
    document.getElementById(`${id}-number`).addEventListener("change", () => {
      SET(id, NUMBER(`${id}-number`));
      guarded(APPLY.origin);
    });
  }
  document.getElementById("palette").addEventListener("change", event => guarded(() => api.app_set_palette(Number(event.target.value))));
  document.getElementById("iteration-cap").addEventListener("change", event => guarded(() => api.app_set_iteration_cap(Number(event.target.value))));
  document.getElementById("preset").addEventListener("change", event => guarded(() => {
    const row = JSON.parse(api.app_preset(Number(event.target.value)));
    SET("theta-1", row.theta_1);
    SET("theta-2", row.theta_2);
    for (const [index, [id]] of ORIGIN.entries()) {
      SET(id, row.origin[index]);
      SET(`${id}-number`, row.origin[index]);
    }
    SET("view-theta-1", row.view_theta_1);
    SET("view-theta-2", row.view_theta_2);
    SET("camera-yaw", row.camera_yaw);
    SET("camera-pitch", row.camera_pitch);
    SET("height", row.height_scale);
    SET("distance-five", row.distance_five);
    SET("distance-four", row.distance_four);
    // A preset names every control, and the scale is now one of them; the origin it carries resets
    // the zoom anyway, so the row that reaches the worker is the whole chart rather than the last
    // depth the user happened to be at.
    SET("scale", 0);
    for (const apply of Object.values(APPLY)) apply();
  }));
  // The marker and the rubber band are DOM elements over the canvas, never geometry in the scene
  // pass: the page can place them to the pixel, and the scene pass has one pipeline for a reason.
  const showTarget = (x, y) => {
    TARGET.style.left = `${x}px`;
    TARGET.style.top = `${y}px`;
    TARGET.hidden = false;
  };
  let drag = null;
  CANVAS.addEventListener("pointerdown", event => {
    event.preventDefault();
    const bounds = CANVAS.getBoundingClientRect();
    if (!(bounds.width > 0 && bounds.height > 0)) return;
    drag = [event.pointerId, event.clientX - bounds.left, event.clientY - bounds.top];
    CANVAS.setPointerCapture(event.pointerId);
  });
  CANVAS.addEventListener("pointermove", event => {
    if (!drag || drag[0] !== event.pointerId) return;
    const bounds = CANVAS.getBoundingClientRect();
    const [, x, y] = drag;
    const to = [event.clientX - bounds.left, event.clientY - bounds.top];
    RUBBER.style.left = `${Math.min(x, to[0])}px`;
    RUBBER.style.top = `${Math.min(y, to[1])}px`;
    RUBBER.style.width = `${Math.abs(to[0] - x)}px`;
    RUBBER.style.height = `${Math.abs(to[1] - y)}px`;
    RUBBER.hidden = false;
  });
  CANVAS.addEventListener("pointerup", event => {
    if (!drag || drag[0] !== event.pointerId) return;
    const bounds = CANVAS.getBoundingClientRect();
    const [, x, y] = drag;
    drag = null;
    RUBBER.hidden = true;
    if (!(bounds.width > 0 && bounds.height > 0)) return;
    const to = [event.clientX - bounds.left, event.clientY - bounds.top];
    // The marker goes where the pointer went first, so a refused edit leaves it where you asked;
    // on success the plane point under it is the centre, so it belongs at the middle of the canvas.
    showTarget((x + to[0]) / 2, (y + to[1]) / 2);
    guarded(() => {
      api.app_zoom_box(x, y, to[0], to[1], bounds.width, bounds.height);
      SET("scale", JSON.parse(api.app_facts_json()).requested_zoom_log2);
      showTarget(bounds.width / 2, bounds.height / 2);
    });
  });
  CANVAS.addEventListener("pointercancel", () => { drag = null; RUBBER.hidden = true; });

  // One row, one path: the sliders take the values and then the same handlers a user's own
  // movement reaches apply them. The centre is the one field with no widget, so it is one explicit
  // call after the row rather than a second way for the values that do have widgets to arrive.
  const applyRow = row => {
    SET("theta-1", row.theta_1);
    SET("theta-2", row.theta_2);
    for (const [index, [id]] of ORIGIN.entries()) {
      SET(id, row.origin[index]);
      SET(`${id}-number`, row.origin[index]);
    }
    SET("view-theta-1", row.view_theta_1);
    SET("view-theta-2", row.view_theta_2);
    SET("camera-yaw", row.camera_yaw);
    SET("camera-pitch", row.camera_pitch);
    SET("height", row.height_scale);
    SET("distance-five", row.distance_five);
    SET("distance-four", row.distance_four);
    SET("scale", row.zoom_log2);
    for (const apply of Object.values(APPLY)) apply();
    api.app_set_centre(JSON.stringify(row.centre));
  };

  const refreshBoxes = () => {
    for (const box of ["a", "b"]) {
      document.getElementById(`readout-${box}`).textContent = describeRow(box.toUpperCase(), BOXES[box]);
      document.getElementById(`load-${box}`).disabled = BOXES[box] === null;
    }
    MORPH.disabled = BOXES.a === null || BOXES.b === null;
  };

  for (const box of ["a", "b"]) {
    BOXES[box] = readStoredView(box);
    document.getElementById(`save-${box}`).addEventListener("click", () => guarded(() => {
      BOXES[box] = JSON.parse(api.app_saved_view_json());
      writeStoredView(box, BOXES[box]);
      refreshBoxes();
    }));
    document.getElementById(`load-${box}`).addEventListener("click", () => guarded(() => {
      if (BOXES[box]) applyRow(BOXES[box]);
    }));
  }
  MORPH.addEventListener("input", () => guarded(() => {
    if (!BOXES.a || !BOXES.b) return;
    applyRow(JSON.parse(api.app_morph_view(JSON.stringify(BOXES.a), JSON.stringify(BOXES.b), NUMBER("morph"))));
  }));
  refreshBoxes();
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
