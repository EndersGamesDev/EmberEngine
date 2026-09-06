// The lab as a module rather than as a page.
//
// A lab that can only be driven through its control page cannot be measured by a script: every
// proof of a frame has had to load the whole document, write a row into localStorage, reload, click
// a button, poll the text of a facts grid, and read the canvas through a shim inserted into a
// private copy of the page. The shim is the worst of it, because a readback that needs an edit to
// the page edits the thing it is measuring.
//
// This module is the entry that removes all of that. It owns the boot, the worker URL, the ABI
// probe and the frame loop, and it hands back one object: apply a row, move a control, read the
// facts, wait until a named refinement level has settled, take the frame's pixels, stop. The
// control page becomes one client of that object rather than the only way in, so the page and a
// driver run the same loop over the same wasm boundary and cannot drift apart.
//
// Nothing in here touches the DOM beyond the canvas element it is handed, and nothing in here
// reads or writes localStorage. Those are the page's own concerns and they stay in the page.

const ABI = 3;

// The loader version the repository ships. It is one, it stays one, and a page-contract test says
// so: a bump here would leave every cached bundle in the field pointing at files that no longer
// answer. `version` in the options exists for a driver serving a build from somewhere else, and
// replaces the query on each default rather than being spliced into a path.
const LOADER_VERSION = "1";
const DEFAULT_PKG_URL = "./pkg/ember_lab_julibrot.js?v=1";
const DEFAULT_WASM_URL = "./pkg/ember_lab_julibrot_bg.wasm?v=1";
const DEFAULT_WORKER_URL = "./worker.js?v=1";

// The frame loop must not be able to latch. A `requestAnimationFrame` queued while the tab or the
// pane is not painting is never called, so a flag set at schedule time and cleared only inside that
// callback outlives the frame it was guarding: every later schedule returns immediately while
// `app_needs_refresh` still answers true, and the picture sleeps on a transient frame until a
// control move happens to reach a scheduling path. The flag is therefore cleared on frame entry,
// retired whenever the page becomes visible or focused again, and backed by a low-rate timer that
// runs the turn the animation callback did not. Whichever path arrives first for a schedule clears
// the flag and runs it, so a healthy `requestAnimationFrame` and the timer can never both drive one
// turn.
const FRAME_FALLBACK_MS = 250;

// A settle is bounded because an unbounded one is a hang with a nicer name. Two minutes is far
// longer than any level this lab has been measured at and far shorter than a driver's patience.
const SETTLE_TIMEOUT_MS = 120_000;

const versioned = (url, version) =>
  version === undefined || version === null ? url : `${url.split("?")[0]}?v=${version}`;

// The fields a row carries that are also individual controls, by the name `set` accepts. The
// mapping is to the app's own setters, so a control moved through here takes exactly the path the
// page's slider takes; nothing below reaches the worker by a route a user's own movement does not.
const OBJECT_FIELDS = ["o12", "o13", "o14", "o23", "o24", "o34"];
const CAMERA_FIELDS = ["q12", "q13", "q14", "q23", "q24", "q34", "q15", "q25", "q35", "q45"];
const TRANSLATION_FIELDS = ["t1", "t2", "t3", "t4", "t5"];
const ORIGIN_FIELDS = ["z_re", "z_im", "c_re", "c_im"];

/// A settle that ends without its level is an error carrying the facts that explain why.
class SettleError extends Error {
  constructor(message, facts) {
    super(message);
    this.name = "SettleError";
    this.facts = facts;
  }
}

/**
 * Boots the lab on one canvas element and returns the object a script drives it with.
 *
 * The canvas is an element, not an id, and there is no status element: a driver page carries a
 * canvas and nothing else, and a typed startup failure still reaches the console and the returned
 * promise's rejection. The control page passes its own canvas and gets the identical object.
 */
export async function openLab({ canvas, pkgUrl, workerUrl, wasmUrl, version } = {}) {
  if (!canvas || typeof canvas.getContext !== "function") {
    throw new Error("openLab needs a canvas element");
  }
  // Published before the module is imported, because the worker owner reads the global when the
  // channel is built and a URL that arrives later is a URL the channel never saw.
  globalThis.JULIBROT_WORKER_URL = versioned(workerUrl ?? DEFAULT_WORKER_URL, version);
  const api = await import(versioned(pkgUrl ?? DEFAULT_PKG_URL, version));
  await api.default(versioned(wasmUrl ?? DEFAULT_WASM_URL, version));
  const moduleVersion = api.julibrot_abi_version();
  if (moduleVersion !== ABI) {
    throw new Error(`VersionSkew: main wasm ${moduleVersion}, loader ${ABI}`);
  }
  await api.start_julibrot_on_canvas(canvas);
  return new Lab(api, canvas);
}

/**
 * The frame loop and the app boundary, with the DOM left to whoever owns the document.
 */
class Lab {
  #api;
  #canvas;
  #rafPending = false;
  #ticket = 0;
  #fallbackTimer = null;
  #turnListeners = new Set();
  #stopped = false;
  #wake;
  #visibility;
  #counts = {
    frame_schedules: 0,
    frames_from_raf: 0,
    frames_from_fallback: 0,
    frame_latch_clears: 0,
    frame_loop_wakeups: 0,
  };

  constructor(api, canvas) {
    this.#api = api;
    this.#canvas = canvas;
    this.#wake = () => {
      this.#counts.frame_loop_wakeups += 1;
      this.#rafPending = false;
      this.#ticket += 1;
      if (this.turning()) this.schedule();
    };
    this.#visibility = () => {
      if (!document.hidden) this.#wake();
    };
    document.addEventListener("visibilitychange", this.#visibility);
    window.addEventListener("pageshow", this.#wake);
    window.addEventListener("focus", this.#wake);
  }

  /** The wasm module itself, for the controls this object does not name. */
  get api() {
    return this.#api;
  }

  /** The canvas the lab was booted on. */
  get canvas() {
    return this.#canvas;
  }

  /** The frame-loop counters the page publishes as its own facts. */
  counts() {
    return { ...this.#counts };
  }

  /** The app's full honest facts snapshot, parsed. */
  facts() {
    return JSON.parse(this.#api.app_facts_json());
  }

  /**
   * Applies one saved-view row, object or JSON string, through the single navigation transaction
   * the page's own row load uses. The stored slice point is cleared first, because a row replaces
   * the picture the point was named on.
   */
  applyRow(row) {
    const text = typeof row === "string" ? row : JSON.stringify(row);
    this.#api.app_clear_crosshair();
    this.#api.app_apply_saved_view(text);
    this.#api.app_request_frame();
    this.schedule();
    return this;
  }

  /**
   * Moves one control by the name the row carries for it.
   *
   * Every field reaches the same app setter the matching slider reaches. A field the lab has no
   * setter for is refused rather than silently dropped, so a driver that names a control wrongly
   * learns it here instead of measuring an unchanged picture.
   */
  set(field, value) {
    const api = this.#api;
    const number = Number(value);
    const view = () => JSON.parse(api.app_saved_view_json());
    const angles = (names, setter) => {
      const row = view();
      const values = names.map(name => (name === field ? number : Number(row[name])));
      setter(...values);
    };
    if (OBJECT_FIELDS.includes(field)) {
      angles(OBJECT_FIELDS, api.app_set_object_angles);
    } else if (CAMERA_FIELDS.includes(field)) {
      angles(CAMERA_FIELDS, api.app_set_camera_angles);
    } else if (TRANSLATION_FIELDS.includes(field)) {
      angles(TRANSLATION_FIELDS, api.app_set_camera_translation);
    } else if (ORIGIN_FIELDS.includes(field)) {
      const row = view();
      const origin = ORIGIN_FIELDS.map((name, index) =>
        name === field ? number : Number(row.origin[index]),
      );
      api.app_set_plane_origin(...origin);
    } else if (field === "height_scale" || field === "height") {
      api.app_set_height(number);
    } else if (field === "zoom_log2" || field === "scale") {
      api.app_set_scale(number);
    } else if (field === "distance_five" || field === "distance_four") {
      const row = view();
      api.app_set_distances(
        field === "distance_five" ? number : Number(row.distance_five),
        field === "distance_four" ? number : Number(row.distance_four),
      );
    } else if (field === "camera_yaw" || field === "camera_pitch") {
      const row = view();
      api.app_set_camera(
        field === "camera_yaw" ? number : Number(row.camera_yaw),
        field === "camera_pitch" ? number : Number(row.camera_pitch),
      );
    } else if (field === "iteration_cap") {
      api.app_set_iteration_cap(number);
    } else if (field === "palette") {
      api.app_set_palette(number);
    } else if (field === "precision") {
      api.app_set_precision_mode(number);
    } else if (field === "scene_mode") {
      api.app_set_scene_mode(number);
    } else {
      throw new Error(`no control is named ${field}`);
    }
    api.app_request_frame();
    this.schedule();
    return this;
  }

  /** Queues one explicit surface-frame request and asks the loop for a turn. */
  requestFrame() {
    this.#api.app_request_frame();
    this.schedule();
    return this;
  }

  /** Queues one adaptive measurement suite; no result is claimed by the call. */
  requestMeasurement() {
    this.#api.app_request_measurement();
    this.schedule();
    return this;
  }

  /** Restarts scene refinement at the staged pose, for a lab left in manual scene mode. */
  updateScene() {
    this.#api.app_update_scene();
    this.schedule();
    return this;
  }

  /**
   * Registers a listener called with the facts after every turn the loop runs.
   *
   * The control page renders its overlay from this, so the page's picture and the driver's numbers
   * come from one loop rather than from two that could disagree.
   */
  onTurn(listener) {
    this.#turnListeners.add(listener);
    return () => this.#turnListeners.delete(listener);
  }

  /**
   * Reports whether a yielded completion or refinement turn remains pending.
   *
   * The app answers false only once its loop has stopped for a typed cause, so a refusal it
   * survived still schedules the next turn; a throw from the query itself is treated as stopped.
   */
  turning() {
    if (this.#stopped) return false;
    try {
      return this.#api.app_needs_refresh();
    } catch (error) {
      console.error(error);
      return false;
    }
  }

  /** Asks for one loop turn, if one is not already scheduled. */
  schedule() {
    if (this.#stopped || this.#rafPending) return this;
    this.#rafPending = true;
    this.#counts.frame_schedules += 1;
    const ticket = (this.#ticket += 1);
    requestAnimationFrame(nowMs => this.#runTurn(ticket, nowMs, false));
    if (this.#fallbackTimer !== null) clearTimeout(this.#fallbackTimer);
    // The low rate is the point: it is a floor under a loop the browser has stopped painting for,
    // not a second animation clock. Arriving after a healthy callback it does nothing at all, and
    // arriving with nothing due it still releases the flag, because the callback it was waiting on
    // may never be called and the next control move must be able to schedule.
    this.#fallbackTimer = setTimeout(() => {
      this.#fallbackTimer = null;
      if (!this.#rafPending || ticket !== this.#ticket) return;
      if (this.turning()) {
        this.#runTurn(ticket, performance.now(), true);
      } else {
        this.#rafPending = false;
        this.#counts.frame_latch_clears += 1;
      }
    }, FRAME_FALLBACK_MS);
    return this;
  }

  // One turn per schedule, whichever path arrives with it. A stale animation callback carries an
  // older ticket, and the timer behind a callback that already ran finds the flag clear, so neither
  // can run a turn twice; the flag is cleared before the body so the turn may schedule the next.
  #runTurn(ticket, nowMs, viaFallback) {
    if (!this.#rafPending || ticket !== this.#ticket) return;
    this.#rafPending = false;
    if (viaFallback) this.#counts.frames_from_fallback += 1;
    else this.#counts.frames_from_raf += 1;
    let facts = null;
    let failure = null;
    try {
      this.#api.app_refresh(nowMs);
      facts = this.facts();
    } catch (error) {
      failure = error;
      try {
        facts = this.facts();
      } catch (factsError) {
        console.error(factsError);
      }
    }
    for (const listener of this.#turnListeners) {
      try {
        listener(facts, failure);
      } catch (error) {
        console.error(error);
      }
    }
    if (this.turning()) this.schedule();
  }

  /**
   * Resolves with the facts once the named refinement level is the delivered one, nothing is
   * pending and no scene is in flight; rejects with the facts on a stopped loop or a timeout.
   *
   * Settling is the whole of what a measurement needs and none of what it must not have: a moving
   * frame may be inaccurate, so a number read off one is a number about a picture that no longer
   * exists. The three conditions together are the statement "this picture is finished".
   */
  settle({ level = "Final", timeoutMs = SETTLE_TIMEOUT_MS } = {}) {
    return new Promise((resolve, reject) => {
      let timer = null;
      let release = null;
      const finish = (settle, value) => {
        if (timer !== null) clearTimeout(timer);
        if (release) release();
        settle(value);
      };
      const consider = facts => {
        if (!facts) return false;
        if (facts.loop_stopped_reason) {
          finish(reject, new SettleError(`the loop stopped: ${facts.loop_stopped_reason}`, facts));
          return true;
        }
        if (
          facts.refinement_level === level &&
          facts.refinement_pending === false &&
          facts.scene_update_pending === false &&
          (facts.in_flight_scene_id === null || facts.in_flight_scene_id === undefined)
        ) {
          finish(resolve, facts);
          return true;
        }
        return false;
      };
      release = this.onTurn(facts => {
        if (consider(facts)) return;
        // A loop that has gone quiet without the level will never reach it on its own, and waiting
        // out the timeout for a picture nothing is working on wastes the driver's whole budget.
        if (!this.turning()) {
          finish(
            reject,
            new SettleError(`the loop went quiet at ${facts?.refinement_level ?? "no level"} before ${level}`, facts),
          );
        }
      });
      timer = setTimeout(() => {
        let facts = null;
        try {
          facts = this.facts();
        } catch (error) {
          console.error(error);
        }
        finish(reject, new SettleError(`${level} was not reached in ${timeoutMs} ms`, facts));
      }, timeoutMs);
      let current = null;
      try {
        current = this.facts();
      } catch (error) {
        console.error(error);
      }
      if (!consider(current)) this.schedule();
    });
  }

  /**
   * Returns the presented frame's pixels: RGBA, four bytes per pixel, rows top-down.
   *
   * The copy is made inside the pass that already owns the frame texture, on the turn that presents
   * it, and it happens on request only — never per frame. What comes back is the image the browser
   * put on the canvas, not a re-render of it and not a picture read through a context flag the page
   * had to be edited to obtain.
   *
   * It is valid to call at any time and it is only worth reading when the picture is finished: call
   * it after `settle` resolves.
   */
  async frame({ timeoutMs = SETTLE_TIMEOUT_MS } = {}) {
    const capture = JSON.parse(this.#api.app_frame_capture_json());
    if (!capture.frame_capture_supported) {
      throw new Error(`this device cannot copy a presented frame: ${capture.frame_capture_status}`);
    }
    this.#api.app_request_frame_capture();
    this.requestFrame();
    const deadline = performance.now() + timeoutMs;
    for (;;) {
      const bytes = this.#api.app_take_frame_rgba();
      if (bytes) {
        const taken = JSON.parse(this.#api.app_frame_capture_json());
        return { width: taken.frame_capture_width, height: taken.frame_capture_height, rgba: bytes };
      }
      if (performance.now() > deadline) {
        throw new Error(`no frame was copied in ${timeoutMs} ms`);
      }
      // One turn of the loop per wait, on the loop's own clock: the copy completes on a turn, so a
      // waiter that does not let a turn run is a waiter that spins forever.
      this.schedule();
      await new Promise(resume => requestAnimationFrame(resume));
    }
  }

  /**
   * Retires the loop and its listeners. The wasm app is left exactly as it stands: stopping is the
   * driver saying it is finished, not the lab claiming the picture was wrong.
   */
  stop() {
    this.#stopped = true;
    this.#rafPending = false;
    this.#ticket += 1;
    if (this.#fallbackTimer !== null) {
      clearTimeout(this.#fallbackTimer);
      this.#fallbackTimer = null;
    }
    this.#turnListeners.clear();
    document.removeEventListener("visibilitychange", this.#visibility);
    window.removeEventListener("pageshow", this.#wake);
    window.removeEventListener("focus", this.#wake);
    return this;
  }
}

export { ABI, FRAME_FALLBACK_MS, LOADER_VERSION, SETTLE_TIMEOUT_MS, SettleError };
