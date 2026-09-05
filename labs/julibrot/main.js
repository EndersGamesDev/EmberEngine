const ABI = 3;
globalThis.JULIBROT_WORKER_URL = "./worker.js?v=1788614526";
const STATUS = document.getElementById("status");
const CANVAS = document.getElementById("julibrot");
const FACTS = document.getElementById("facts-grid");
const TARGET = document.getElementById("target");
const RUBBER = document.getElementById("rubber");
const MORPH = document.getElementById("morph");
const BOXES = { a: null, b: null };
const OBJECT_IDS = ["o12", "o13", "o14", "o23", "o24", "o34"];
const CAMERA_IDS = ["q12", "q13", "q14", "q23", "q24", "q34", "q15", "q25", "q35", "q45"];
const TRANSLATION_IDS = ["t1", "t2", "t3", "t4", "t5"];
// The named rows every side can choose from, and which row each side last chose. Built-in rows are
// not in here: they come from the app, they are never deletable, and they never need storing.
const ROWS = { named: [], selection: { a: "", b: "" } };
// The page's own pan-versus-click threshold, which mirrors the boundary's box-versus-click one: a
// gesture too short to be a translation is the click it looked like, and the app decides the rest.
const CLICK_THRESHOLD_PX = 4;
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

// One key per box, and one more for the named rows and the two selections. Every access goes
// through this pair, because a private window, cleared site data, or a browser told to refuse
// storage makes the accessor itself throw rather than return nothing, and one wrapped reader and
// one wrapped writer are the whole of the page's exposure to that.
const STORAGE_KEY = box => `julibrot.view.${box}`;
const ROWS_KEY = "julibrot.rows";

function readStoredJson(key) {
  try {
    const text = localStorage.getItem(key);
    return text ? JSON.parse(text) : null;
  } catch (error) {
    STORAGE_STATUS = `unavailable: ${error && error.name ? error.name : "refused"}`;
    return null;
  }
}

function writeStoredJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (error) {
    STORAGE_STATUS = `unavailable: ${error && error.name ? error.name : "refused"}`;
  }
}

// A stored row is a row only if it carries the four-coordinate encoded centre. Anything else is
// absent, never a view the page claims to hold and could not decode.
function isStoredRow(row) {
  if (!row || typeof row !== "object" || !row.centre || !Array.isArray(row.centre.coords)) return false;
  const affine = [...OBJECT_IDS, ...CAMERA_IDS, ...TRANSLATION_IDS];
  return row.centre.coords.length === 4 && affine.every(field => Number.isFinite(row[field]));
}

function readStoredView(box) {
  const row = readStoredJson(STORAGE_KEY(box));
  return isStoredRow(row) ? row : null;
}

function writeStoredView(box, row) {
  writeStoredJson(STORAGE_KEY(box), row);
}

// The named rows and the two selections travel as one record, so one read restores both and a
// malformed half is dropped rather than taken as a list the page could not read.
function readStoredRows() {
  const stored = readStoredJson(ROWS_KEY);
  const rows = stored && Array.isArray(stored.rows) ? stored.rows : [];
  const selection = stored && stored.selection && typeof stored.selection === "object" ? stored.selection : {};
  return {
    rows: rows.filter(entry => entry && typeof entry.name === "string" && entry.name !== "" && isStoredRow(entry.row)),
    selection: {
      a: typeof selection.a === "string" ? selection.a : "",
      b: typeof selection.b === "string" ? selection.b : "",
    },
  };
}

function centreDigits(zoomLog2) {
  return Math.min(40, Math.max(3, Math.ceil(Number(zoomLog2) * Math.LOG10E * Math.LN2) + 3));
}

function describeRow(name, row) {
  if (!row) return `${name} is empty`;
  const digits = centreDigits(row.zoom_log2);
  const angles = [...OBJECT_IDS, ...CAMERA_IDS].map(field => Number(row[field]).toFixed(3)).join(" ");
  const translation = TRANSLATION_IDS.map(field => Number(row[field]).toFixed(3)).join(" ");
  const origin = row.origin.map(value => Number(value).toFixed(3)).join(" ");
  const centre = row.centre_f64.map(value => Number(value).toFixed(digits)).join(" ");
  // The view scalars are named rather than folded into the angle run. A row differing only in
  // height is a different picture and, at a tilted slice, a different reprojection problem; a
  // summary that omits it sends anyone comparing two rows looking in the wrong place.
  const view = [
    `height ${Number(row.height_scale).toFixed(3)}`,
    `d5 ${Number(row.distance_five).toFixed(3)}`,
    `d4 ${Number(row.distance_four).toFixed(3)}`,
    `yaw ${Number(row.camera_yaw).toFixed(3)}`,
    `pitch ${Number(row.camera_pitch).toFixed(3)}`,
  ].join(" ");
  return `${name}: angles ${angles} · view ${view} · translation ${translation} · origin ${origin} · zoom_log2 ${Number(row.zoom_log2).toFixed(3)} · centre ${centre}`;
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
  if (facts.scene_mode === "manual" && facts.scene_update_pending) {
    if (facts.warp_kind === "HoldStale") {
      return `manual: warp refused, showing scene ${facts.completed_scene_id} unmoved — Update scene to re-render${surviving(facts)}`;
    }
    return `manual: changes pending since scene ${facts.completed_scene_id}${surviving(facts)}`;
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
    named_rows_held: ROWS.named.length,
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

// A row is written into the controls field by field, by name: the control that carries a field is
// the one whose id is the field's name with underscores as dashes, and the two fields whose control
// is named differently are the only exceptions. Nothing below counts the fields, so a row that
// grows a new angle reaches its slider with no edit here, as long as the slider is named after it.
const ROW_CONTROL_ALIAS = { height_scale: "height", zoom_log2: "scale" };
const controlFor = field => document.getElementById(ROW_CONTROL_ALIAS[field] ?? field.replaceAll("_", "-"));

function bindControls(api) {
  // The crosshair is a projection, never a memory of a pixel: the app is asked where the stored
  // point falls under the current map, and the marker goes there. That is what makes it an
  // accuracy oracle — pan, zoom, or let a reference land, and it stays on its feature or it does
  // not, visibly, with the numbers beside it in the facts.
  const drawCrosshair = () => {
    const bounds = CANVAS.getBoundingClientRect();
    if (!(bounds.width > 0 && bounds.height > 0)) return {};
    const crosshair = JSON.parse(api.app_crosshair_json(bounds.width, bounds.height));
    if (!crosshair.crosshair_present) {
      TARGET.hidden = true;
      return crosshair;
    }
    TARGET.style.left = `${crosshair.crosshair_css[0]}px`;
    TARGET.style.top = `${crosshair.crosshair_css[1]}px`;
    TARGET.hidden = !crosshair.crosshair_on_surface;
    return crosshair;
  };
  const refreshFacts = () => {
    const facts = Object.assign(JSON.parse(api.app_facts_json()), pageFacts(), drawCrosshair());
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
    object: () => api.app_set_object_angles(...OBJECT_IDS.map(NUMBER)),
    origin: () => api.app_set_plane_origin(NUMBER("origin-z-re"), NUMBER("origin-z-im"), NUMBER("origin-c-re"), NUMBER("origin-c-im")),
    ambientCamera: () => api.app_set_camera_angles(...CAMERA_IDS.map(NUMBER)),
    cameraTranslation: () => api.app_set_camera_translation(...TRANSLATION_IDS.map(NUMBER)),
    observer: () => api.app_set_camera(NUMBER("camera-yaw"), NUMBER("camera-pitch")),
    height: () => api.app_set_height(NUMBER("height")),
    distances: () => api.app_set_distances(NUMBER("distance-five"), NUMBER("distance-four")),
    // Last, because the origin handler resets the zoom to zero: a row applied in this order ends
    // at the scale it names rather than at the scale the origin change forced.
    scale: () => api.app_set_scale(NUMBER("scale")),
    precision: () => api.app_set_precision_mode(NUMBER("precision")),
  };
  for (const id of OBJECT_IDS) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.object));
  }
  for (const id of CAMERA_IDS) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.ambientCamera));
  }
  for (const id of TRANSLATION_IDS) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.cameraTranslation));
  }
  for (const id of ["camera-yaw", "camera-pitch"]) {
    document.getElementById(id).addEventListener("input", () => guarded(APPLY.observer));
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
  document.getElementById("precision").addEventListener("change", () => guarded(APPLY.precision));
  // One pointer gesture, read three ways here and confirmed at the boundary: shift draws a box,
  // a plain drag past the click threshold translates the picture, and anything shorter is the click
  // it looked like. A click is not a navigation edit at all — it names a point on the slice and the
  // picture stays exactly where it was — so nothing here places the marker; the projection does.
  let drag = null;
  CANVAS.addEventListener("pointerdown", event => {
    event.preventDefault();
    const bounds = CANVAS.getBoundingClientRect();
    if (!(bounds.width > 0 && bounds.height > 0)) return;
    drag = {
      pointer: event.pointerId,
      box: event.shiftKey,
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top,
      last: [event.clientX, event.clientY],
      moved: false,
    };
    CANVAS.setPointerCapture(event.pointerId);
  });
  CANVAS.addEventListener("pointermove", event => {
    if (!drag || drag.pointer !== event.pointerId) return;
    const bounds = CANVAS.getBoundingClientRect();
    const to = [event.clientX - bounds.left, event.clientY - bounds.top];
    if (Math.max(Math.abs(to[0] - drag.x), Math.abs(to[1] - drag.y)) >= CLICK_THRESHOLD_PX) drag.moved = true;
    if (drag.box) {
      RUBBER.style.left = `${Math.min(drag.x, to[0])}px`;
      RUBBER.style.top = `${Math.min(drag.y, to[1])}px`;
      RUBBER.style.width = `${Math.abs(to[0] - drag.x)}px`;
      RUBBER.style.height = `${Math.abs(to[1] - drag.y)}px`;
      RUBBER.hidden = false;
      return;
    }
    // A translation lands on every move rather than on release: the displacement from the accepted
    // reference is the one HOT field built to carry a pan smoothly, and a pan the eye cannot follow
    // is not a pan. The step is the movement since the last one, so the sum is the whole drag.
    if (!drag.moved) return;
    const step = [event.clientX - drag.last[0], event.clientY - drag.last[1]];
    drag.last = [event.clientX, event.clientY];
    guarded(() => api.app_pan_px(step[0], step[1], bounds.width, bounds.height));
  });
  CANVAS.addEventListener("pointerup", event => {
    if (!drag || drag.pointer !== event.pointerId) return;
    const bounds = CANVAS.getBoundingClientRect();
    const started = drag;
    drag = null;
    RUBBER.hidden = true;
    if (!(bounds.width > 0 && bounds.height > 0)) return;
    const to = [event.clientX - bounds.left, event.clientY - bounds.top];
    if (started.box) {
      guarded(() => {
        api.app_zoom_box(started.x, started.y, to[0], to[1], bounds.width, bounds.height);
        SET("scale", JSON.parse(api.app_facts_json()).requested_zoom_log2);
      });
      return;
    }
    if (started.moved) return;
    guarded(() => api.app_set_target(to[0], to[1], bounds.width, bounds.height));
  });
  CANVAS.addEventListener("pointercancel", () => { drag = null; RUBBER.hidden = true; });

  // One row, one path, and no list of field names: every numeric field is written into the control
  // named after it, and then the same handlers a user's own movement reaches apply them. The centre
  // is the one field with no widget, so it is one explicit call after the row rather than a second
  // way for the values that do have widgets to arrive. A built-in row has no centre and no depth.
  const applyRow = row => {
    if (Array.isArray(row.origin)) {
      for (const [index, [id]] of ORIGIN.entries()) {
        SET(id, row.origin[index]);
        SET(`${id}-number`, row.origin[index]);
      }
    }
    for (const [field, value] of Object.entries(row)) {
      if (field === "origin" || typeof value !== "number") continue;
      const element = controlFor(field);
      if (element) element.value = String(value);
    }
    // A row that names no depth starts at the top: the origin it carries resets the zoom anyway, so
    // the row that reaches the worker is the whole chart rather than the depth the user was at.
    if (typeof row.zoom_log2 !== "number") SET("scale", 0);
    for (const apply of Object.values(APPLY)) apply();
    if (row.centre) api.app_set_centre(JSON.stringify(row.centre));
  };
  const loadRow = row => {
    api.app_clear_crosshair();
    applyRow(row);
  };

  // The built-in rows come from the app, in its own order, for as long as it has them: the page
  // never names a preset, so a row added there appears on both sides with no edit here.
  const BUILT_IN = [];
  for (let id = 0; ; id += 1) {
    let row = null;
    try {
      row = JSON.parse(api.app_preset(id));
    } catch {
      break;
    }
    BUILT_IN.push({ key: `preset:${id}`, name: row.name, row, builtin: true });
  }
  const rowEntries = () => BUILT_IN.concat(
    ROWS.named.map(entry => ({ key: `named:${entry.name}`, name: entry.name, row: entry.row, builtin: false })),
  );
  const entryFor = key => rowEntries().find(entry => entry.key === key) ?? null;
  const say = (box, text) => { document.getElementById(`message-${box}`).textContent = text; };
  const captureRow = () => JSON.parse(api.app_saved_view_json());
  const writeStoredRows = () => writeStoredJson(ROWS_KEY, { rows: ROWS.named, selection: ROWS.selection });

  // A row copied for a bug report is the exact row first and the sentence second: line one is the
  // JSON the page itself round-trips through the saved-view API, every field the row carries and
  // nothing added, so it can be handed straight back; line two is the summary the box already
  // shows, so a human reading the report knows what they are looking at without decoding limbs.
  const rowText = (label, row) => `${JSON.stringify(row)}\n${describeRow(label, row)}`;
  const FALLBACK = document.getElementById("copy-fallback");
  // The clipboard is a permission, not a function: a browser may not expose it, and one that does
  // may still refuse. Neither is an error worth losing the row over, so the refusal path puts the
  // text on the page, selected, and says so — the copy still happens, by hand.
  const offerSelection = (target, text) => {
    FALLBACK.value = text;
    FALLBACK.hidden = false;
    FALLBACK.focus();
    FALLBACK.select();
    say(target, "the clipboard refused; the row is selected below");
  };
  const copyText = (target, text) => {
    if (!navigator.clipboard || typeof navigator.clipboard.writeText !== "function") {
      offerSelection(target, text);
      return;
    }
    navigator.clipboard.writeText(text).then(
      () => {
        FALLBACK.hidden = true;
        say(target, "row copied");
      },
      () => offerSelection(target, text),
    );
  };

  // Both sides list the same rows, so a row saved on one is choosable on the other; the selection
  // is only ever a key into that one list, and a key that no longer names a row falls back to none.
  const refreshRows = () => {
    for (const box of ["a", "b"]) {
      const select = document.getElementById(`rows-${box}`);
      const chosen = entryFor(ROWS.selection[box]) ? ROWS.selection[box] : "";
      select.replaceChildren();
      const none = document.createElement("option");
      none.value = "";
      none.textContent = "— choose a row —";
      select.append(none);
      for (const entry of rowEntries()) {
        const option = document.createElement("option");
        option.value = entry.key;
        option.textContent = entry.builtin ? entry.name : `${entry.name} (saved)`;
        select.append(option);
      }
      select.value = chosen;
      ROWS.selection[box] = chosen;
      document.getElementById(`delete-${box}`).disabled = !chosen.startsWith("named:");
    }
  };

  const refreshBoxes = () => {
    for (const box of ["a", "b"]) {
      document.getElementById(`readout-${box}`).textContent = describeRow(box.toUpperCase(), BOXES[box]);
      document.getElementById(`load-${box}`).disabled = BOXES[box] === null;
      document.getElementById(`copy-${box}`).disabled = BOXES[box] === null;
    }
    MORPH.disabled = BOXES.a === null || BOXES.b === null;
    refreshRows();
  };

  const holdRow = (box, row) => {
    BOXES[box] = row;
    writeStoredView(box, row);
    refreshBoxes();
  };

  // Choosing a row on a side makes that row that side's view, which is what the two load buttons
  // used to do on their own. A built-in row names every control but the centre and the depth, so
  // what the side ends up holding is the whole row the app is showing once it has been applied; a
  // saved row is already whole and is held exactly as it was saved.
  const chooseRow = (box, key) => {
    ROWS.selection[box] = key;
    say(box, "");
    const entry = entryFor(key);
    if (!entry) {
      writeStoredRows();
      refreshBoxes();
      return;
    }
    guarded(() => {
      loadRow(entry.row);
      holdRow(box, entry.builtin ? captureRow() : entry.row);
      writeStoredRows();
    });
  };

  // A save needs a name, and a name is refused rather than silently overwriting a row that has it.
  const saveRow = box => {
    const field = document.getElementById(`name-${box}`);
    const name = field.value.trim();
    if (name === "") {
      say(box, "a saved row needs a name");
      return;
    }
    if (rowEntries().some(entry => entry.name === name)) {
      say(box, `the name ${name} is already taken`);
      return;
    }
    guarded(() => {
      const row = captureRow();
      ROWS.named.push({ name, row });
      ROWS.selection[box] = `named:${name}`;
      field.value = "";
      holdRow(box, row);
      writeStoredRows();
      say(box, `saved as ${name}`);
    });
  };

  // Only a saved row can be deleted, and deleting it takes it off both sides at once.
  const deleteRow = box => {
    const key = ROWS.selection[box];
    if (!key.startsWith("named:")) {
      say(box, "a built-in row cannot be deleted");
      return;
    }
    const name = key.slice("named:".length);
    ROWS.named = ROWS.named.filter(entry => entry.name !== name);
    for (const side of ["a", "b"]) {
      if (ROWS.selection[side] === key) ROWS.selection[side] = "";
    }
    writeStoredRows();
    refreshBoxes();
    say(box, `deleted ${name}`);
  };

  const storedRows = readStoredRows();
  ROWS.named = storedRows.rows;
  ROWS.selection = storedRows.selection;
  for (const box of ["a", "b"]) {
    BOXES[box] = readStoredView(box);
    document.getElementById(`rows-${box}`).addEventListener("change", event => chooseRow(box, event.target.value));
    document.getElementById(`save-${box}`).addEventListener("click", () => saveRow(box));
    document.getElementById(`delete-${box}`).addEventListener("click", () => deleteRow(box));
    document.getElementById(`load-${box}`).addEventListener("click", () => guarded(() => {
      if (BOXES[box]) loadRow(BOXES[box]);
    }));
    document.getElementById(`copy-${box}`).addEventListener("click", () => {
      if (BOXES[box]) copyText(box, rowText(box.toUpperCase(), BOXES[box]));
    });
  }
  // The live row, not a saved one: a broken state is reported from where it broke, without having
  // to be saved into a box first, because saving it is one more edit between the bug and the report.
  document.getElementById("copy-current").addEventListener("click", () => guarded(() => {
    copyText("current", rowText("current", captureRow()));
  }));
  MORPH.addEventListener("input", () => guarded(() => {
    if (!BOXES.a || !BOXES.b) return;
    applyRow(JSON.parse(api.app_morph_view(JSON.stringify(BOXES.a), JSON.stringify(BOXES.b), NUMBER("morph"))));
  }));
  refreshBoxes();
  document.getElementById("one-frame").addEventListener("click", () => guarded(() => {
    api.app_request_frame();
    showStatus("frame request queued");
  }));
  document.getElementById("auto-scene").addEventListener("change", event => {
    try {
      api.app_set_scene_mode(event.target.checked ? 1 : 0);
      const facts = refreshFacts();
      showStatus(liveStatus(facts));
      if (stillTurning()) scheduleFrame();
    } catch (error) {
      fail(error);
    }
  });
  document.getElementById("update-scene").addEventListener("click", () => {
    try {
      api.app_update_scene();
      refreshFacts();
      showStatus("scene update queued");
      scheduleFrame();
    } catch (error) {
      fail(error);
    }
  });
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
    const api = await import("./pkg/ember_lab_julibrot.js?v=1788614526");
    await api.default("./pkg/ember_lab_julibrot_bg.wasm?v=1788614526");
    const mainVersion = api.julibrot_abi_version();
    if (mainVersion !== ABI) throw new Error(`VersionSkew: main wasm ${mainVersion}, loader ${ABI}`);
    await api.start_julibrot("julibrot", "status");
    const facts = JSON.parse(api.app_facts_json());
    const timer = timerProbe();
    facts.timer_quantum_ms = timer.quantum_ms;
    facts.timing_status = timer.quantum_ms === null ? "unavailable: timer exposed no positive transition" : "requires visible replay";
    facts.timer_probe = timer;
    facts.wasm_bundle_bytes = await artifactBytes("./pkg/ember_lab_julibrot_bg.wasm?v=1788614526");
    facts.javascript_bundle_bytes = await artifactBytes("./pkg/ember_lab_julibrot.js?v=1788614526");
    facts.wasm_instance_count = 2;
    renderFacts(facts);
    showStatus("waiting for first completed scene");
    bindControls(api);
  } catch (error) {
    fail(error);
  }
}

void boot();
