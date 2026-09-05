    import { chooseHost, listLobbies, probeHost, rankHosts, renderChip }
      from '../../../hosts.js';
    import { vehicles, selectedVehicle, formatTime, EngineAudio } from './garage.js?v=86086a2';

    const $ = (id) => document.getElementById(id);
    const sound = new EngineAudio();
    let practice = true;
    let resultsShown = false;
    let awaitingRestart = false;
    const fail = (msg) => {
      const el = $('loading');
      if (el) el.textContent = 'failed to start: ' + (msg || 'see browser console');
      $('engine-note').textContent = 'Could not load the race. Reload to try again. ' + (msg || '');
      $('engine-note').className = 'note bad';
      if(!$('stage').classList.contains('hidden')) {
        $('results').classList.remove('hidden');
        $('result-title').textContent = 'RACE INTERRUPTED';
        $('session-note').textContent = String(msg || 'Reload the garage to try again.');
        $('btn-rematch').classList.add('hidden');
      }
    };
    window.addEventListener('error', (e) => fail(e.message));
    window.addEventListener('unhandledrejection', (e) => fail(e.reason));
    // Stop the arrow keys and space scrolling the page out from under the race.
    addEventListener('keydown', (e) => {
      if (!['INPUT','TEXTAREA'].includes(e.target.tagName) && ['ArrowUp','ArrowDown','ArrowLeft','ArrowRight',' '].includes(e.key) && !$('stage').classList.contains('hidden')) e.preventDefault();
    }, { passive: false });
    $('ember-root').addEventListener('pointerdown', () => document.querySelector('#ember-root canvas')?.focus());
    $('sound-toggle').onclick = () => {
      const on = sound.toggle();
      $('sound-toggle').textContent = on ? 'SOUND ON' : 'SOUND OFF';
      $('sound-toggle').setAttribute('aria-pressed', String(on));
      if(on) sound.start().catch(()=>{});
      document.querySelector('#ember-root canvas')?.focus();
    };
    document.addEventListener('visibilitychange', () => {
      if(sound.ctx) { if(document.hidden) sound.ctx.suspend(); else if(sound.enabled) sound.ctx.resume(); }
    });
    $('btn-garage').onclick = () => location.assign('./');
    $('btn-rematch').onclick = () => {
      if(!practice) { location.assign('./'); return; }
      wasm.restart_local(selectedVehicle());
      awaitingRestart = true;
      resultsShown = false;
      $('results').classList.add('hidden');
      sound.start().catch(()=>{});
      document.querySelector('#ember-root canvas')?.focus();
    };

    const ROOT = new URL('../../../', location);
    // `v` is the deploy stamp and still lives in the book's legacy top-level
    // key, which every publish keeps writing.
    const V = '86086a2';

    const wasm = await import(`./pkg/fire.js?v=${V}`);
    await wasm.default({ module_or_path: `./pkg/fire_bg.wasm?v=${V}` });
    const PROTO = wasm.proto_version();
    $('btn-practice').disabled = false;
    $('btn-online').disabled = false;
    $('btn-practice').textContent = 'Start race ↗';
    $('engine-note').textContent = 'Ready to race. Seven AI opponents. No account needed.';

    // ---- host discovery ----------------------------------------------------
    // Fire Racer has never had a manual URL override — the one in the hub's
    // server settings is the arena's address and pointing this page at it
    // would be a worse bug than having no override at all. The address book
    // is the only source here.
    let chosen = null;    // the host we will race on
    let candidates = [];  // it, then the fallbacks, in order
    let wrongProto = [];  // answered, but speaking another protocol
    // Declared here rather than beside the launchers: `discover()` reads it to
    // drop a probe that finished after a launch had already committed.
    let launched = false;

    // One address, probed once, dressed as a ranked candidate so everything
    // downstream deals with exactly one shape.
    async function onlyHost(url) {
      const entry = { name: '', fire_ws: url, version: '', commit: '', updated: '' };
      const probes = new Map([['', await probeHost(url, { proto: PROTO })]]);
      return rankHosts([entry], { game: 'fire', probes });
    }

    async function discover() {
      const r = await chooseHost(ROOT.href, { game: 'fire', proto: PROTO });
      if (launched) return; // a race started while the probes were out
      chosen = r.chosen;
      candidates = r.candidates;
      wrongProto = r.wrongProto || [];
      renderChip($('host-chip'), chosen, { wrongProto, proto: PROTO });
    }

    // "Nothing answered" and "everything answered on another protocol" are
    // different news: the second still has a playable build behind it, and it
    // is exactly what a protocol bump looks like from the page that shipped
    // first.
    const offlineText = () => (wrongProto.length
      ? `${wrongProto[0].name || 'the server'} is on protocol ${wrongProto[0].proto} and this build speaks ${PROTO}`
        + ' — open the games page and pick the build that matches'
      : 'no online server is published right now — practice mode still works');

    // ---- lobby browser -----------------------------------------------------
    // Its own short-lived socket, exactly like the arena's. The engine loop
    // never sees a menu. The chosen host's lobbies come first and every other
    // host on this protocol below, each row tagged: two people who landed on
    // different machines still have to be able to find each other.
    const showLobbies = async ({ quiet = false } = {}) => {
      const rows = $('lobby-rows');
      const note = (text, bad) => {
        const s = document.createElement('span');
        s.className = 'note' + (bad ? ' bad' : '');
        s.textContent = text;
        rows.replaceChildren(s);
      };
      // The placeholder belongs to the first look only. This function empties
      // the list BEFORE it awaits the probes, so a polled call that showed it
      // would blank a populated list every ten seconds — and take the Join
      // button out from under the cursor with it.
      if (!quiet) note('looking for lobbies…');
      await discover();
      if (!candidates.length) {
        note(offlineText());
        return;
      }

      const lists = await Promise.all(candidates.map((c) => listLobbies(c.url, { proto: PROTO })));
      const all = [];
      candidates.forEach((host, i) => {
        for (const l of lists[i]) all.push({ host, lobby: l });
      });
      if (!all.length) { note('no lobbies yet — create one below'); return; }

      rows.replaceChildren(...all.map(({ host, lobby: l }) => {
        const row = document.createElement('div');
        row.className = 'lobby';
        const nm = document.createElement('span');
        nm.className = 'nm';
        nm.textContent = l.name + (l.has_password ? ' 🔒' : '');
        const meta = document.createElement('span');
        meta.className = 'meta';
        meta.textContent = `${l.players}/${l.cap} · host ${l.host}${l.racing ? ' · racing' : ''}`;
        const tag = document.createElement('span');
        tag.className = 'pill';
        tag.textContent = host.name || 'server';
        tag.title = host.url;
        const go = document.createElement('button');
        go.textContent = 'Join';
        go.onclick = () => {
          const pass = l.has_password ? prompt(`Password for "${l.name}"`) : null;
          if (l.has_password && pass === null) return;
          launchOnline(l.name, pass, false, host); // this row's host, not the chosen one
        };
        row.append(nm, meta, tag, go);
        return row;
      }));
    };

    // ---- the menu poll -----------------------------------------------------
    // The arena's cadence, and the contract in docs/hosts.md §5: the menu
    // re-ranks as it re-lists, so a host that dies while the player reads the
    // panel leaves the chip and the fallback list instead of sitting there
    // looking playable — and a lobby someone else opens ten seconds from now
    // appears without anyone pressing Refresh. It stops the moment the page
    // goes online.
    let lobbyPoll = null;
    let ticking = false;
    async function tick() {
      if (ticking) return;
      ticking = true;
      try { await showLobbies({ quiet: true }); } finally { ticking = false; }
    }
    const pollOn = () => { if (lobbyPoll === null) lobbyPoll = setInterval(tick, 10000); };
    const pollOff = () => { if (lobbyPoll !== null) { clearInterval(lobbyPoll); lobbyPoll = null; } };

    // ---- launching ---------------------------------------------------------
    const showStage = () => {
      $('start').classList.add('hidden');
      $('stage').classList.remove('hidden');
      $('session-toolbar').classList.remove('hidden');
      document.body.classList.add('racing');
    };

    const launchLocal = () => {
      if (launched) return;
      launched = true;
      practice = true;
      sound.start().catch(()=>{});
      pollOff(); // practice hides the panel; it must not keep opening sockets
      showStage();
      try { wasm.start_local_vehicle(selectedVehicle()); }
      catch(e) { fail(e); return; }
      pollHud();
    };

    const launchOnline = async (lobby, password, create, target) => {
      if (launched) return;
      sound.start().catch(()=>{});
      const wanted = target || chosen;
      const note = $('online-note');
      if (!wanted) {
        note.className = 'note bad';
        note.textContent = offlineText();
        pollOn(); // includes a handover whose host is already gone: keep looking
        return;
      }
      launched = true;
      pollOff();

      // Probe again RIGHT before the engine takes the socket. The list can be
      // seconds old and a quick tunnel dies without warning; once the wasm
      // client owns the connection there is no failover left, so this is the
      // last moment a dead host can still be turned into a live one.
      note.className = 'note';
      note.textContent = 'connecting…';

      // A Create walks the fallbacks — one server is as good as another for a
      // new lobby. A JOIN does not: that lobby lives on that machine and
      // nowhere else, so walking on would put the player in a stranger's
      // lobby of the same name, or in none, and tell them it "moved". The
      // `wanted === chosen` arm keeps the failover for the address-less
      // handover below, where the target came from the ranking, not a row.
      const order = create
        ? [wanted, ...candidates.filter((c) => c.url !== wanted.url)]
        : [wanted];
      let live = null;
      for (const c of order) {
        // Serial on purpose: the next host only matters once this one failed.
        const p = await probeHost(c.url, { proto: PROTO, timeoutMs: 3000 });
        if (p.ok) { live = c; break; }
      }
      if (!live) {
        launched = false;
        note.className = 'note bad';
        note.textContent = order.length === 1
          ? `${wanted.name || 'that lobby’s host'} stopped answering — the lobby may be gone`
          : 'no server answered — try again in a moment';
        pollOn();
        tick();
        return;
      }
      if (live.url !== wanted.url) {
        note.textContent =
          `${wanted.name || 'that server'} stopped answering — moved to ${live.name || 'another server'}`;
      }
      chosen = live;
      renderChip($('host-chip'), live, { proto: PROTO });

      showStage();
      practice = false;
      $('btn-rematch').textContent = 'Find another race ↗';
      try {
        wasm.start_online(JSON.stringify({
          ws: live.url,
          handle: ($('handle').value || 'driver').trim(),
          lobby, password: password || '', create, vehicle: selectedVehicle(),
        }));
        pollHud();
      } catch (e) {
        launched = false;
        $('stage').classList.add('hidden');
        $('start').classList.remove('hidden');
        $('session-toolbar').classList.add('hidden');
        document.body.classList.remove('racing');
        note.className = 'note bad';
        note.textContent = String(e);
        pollOn();
      }
    };

    $('btn-practice').onclick = launchLocal;
    $('btn-online').onclick = () => {
      $('online-panel').classList.toggle('hidden');
      $('btn-online').classList.add('primary');
      $('btn-practice').classList.remove('primary');
      if (!$('online-panel').classList.contains('hidden')) { showLobbies(); pollOn(); }
      else pollOff(); // a closed panel polls nothing
    };
    // Not `= showLobbies`: the handler would hand the click event in as the
    // options object.
    $('btn-refresh').onclick = () => showLobbies();
    $('btn-create').onclick = () => {
      const name = ($('newlobby').value || '').trim();
      if (!name) { $('online-note').className = 'note bad'; $('online-note').textContent = 'give the lobby a name'; return; }
      launchOnline(name, ($('newpass').value || '').trim(), true, chosen);
    };

    // ---- a join handed over from the hub's live-lobby showcase
    // The hub already picked the machine: this lobby lives on THAT host and
    // nowhere else, so its address is used verbatim with no re-ranking. It is
    // still probed once, so a dead handover says so instead of hanging.
    let pending = null;
    try { pending = JSON.parse(sessionStorage.getItem('ember-pending')); } catch {}
    sessionStorage.removeItem('ember-pending');
    if (pending && pending.lobby) {
      $('online-panel').classList.remove('hidden');
      $('btn-online').classList.add('primary');
      $('btn-practice').classList.remove('primary');
      if (pending.ws) {
        const r = await onlyHost(pending.ws);
        chosen = r.chosen;
        candidates = r.candidates;
        wrongProto = r.wrongProto || [];
      } else {
        // A handover from a page that predates the host list: no address came
        // with it, so the normal pick applies.
        await discover();
      }
      if (chosen && pending.host) chosen.name = chosen.name || pending.host;
      renderChip($('host-chip'), chosen, { wrongProto, proto: PROTO });
      // No poll here: this branch opens the panel only to race straight into
      // the handed-over lobby. `launchOnline` starts one if that fails.
      launchOnline(pending.lobby, pending.password, pending.action === 'create', chosen);
    } else {
      // Rank once on load so the chip is honest before anyone opens the
      // online panel; the panel's own refresh re-ranks from there.
      discover();
    }

    // ---- HUD ---------------------------------------------------------------
    // The renderer has one scene pass, no 2D layer and no text, so the HUD
    // lives here and reads a snapshot the game publishes every frame.
    const speedEl = document.querySelector('#speed .n');
    const lapEl = document.querySelector('#lap .n');
    const placeEl = $('place');
    const boostEl = $('boost');
    const stateEl = $('state');
    const driftEl = $('drift');
    const MAX_BOOST = 3;
    const items = [
      ['?', 'EMPTY', 'DRIVE THROUGH A BOX'], ['ϟ', 'NITRO', 'E / ACCELERATE'],
      ['◇', 'SHIELD', 'E / PROTECT'], ['◎', 'PULSE', 'E / ATTACK'],
      ['≋', 'OIL SLICK', 'E / DROP'], ['⌁', 'GRIP', 'E / CORNER FASTER'],
    ];
    let pips = [];

    const setPips = (n) => {
      if (pips.length !== MAX_BOOST) {
        boostEl.querySelectorAll('.pip').forEach((p) => p.remove());
        pips = Array.from({ length: MAX_BOOST }, () => {
          const d = document.createElement('div');
          d.className = 'pip';
          boostEl.appendChild(d);
          return d;
        });
      }
      pips.forEach((p, i) => p.classList.toggle('on', i < n));
    };

    const pollHud = () => {
      const hud = $('hud');
      const tick = () => {
        try {
          const h = JSON.parse(wasm.hud_json());
          if(awaitingRestart) {
            if(h.finished) { requestAnimationFrame(tick); return; }
            awaitingRestart = false;
          }
          if(!h.finished && resultsShown) {
            resultsShown = false;
            $('results').classList.add('hidden');
          }
          hud.classList.add('live');
          speedEl.textContent = Math.round(h.speed);
          lapEl.textContent = `${Math.min(h.lap + 1, h.laps)}/${h.laps}`;
          placeEl.textContent = `${h.place}/${h.racers}`;
          setPips(h.boost);
          $('gear').textContent = h.gear === 0 ? 'R' : h.gear || '1';
          $('hud-car').textContent = vehicles[h.vehicle || 0]?.name || 'GT-01';
          $('race-time').textContent = formatTime(h.race_time);
          const item = items[h.item] || items[0];
          $('item-symbol').textContent = item[0];
          $('item-name').textContent = item[1];
          $('item-use').textContent = item[2];
          if(h.item === 3 && h.place === 1) $('item-use').textContent = 'NO TARGET AHEAD';
          $('item').style.borderColor = h.item ? '#9ddbc280' : '#d7e6e72a';
          const charge = Math.max(0, Math.min(1, h.drift_charge || 0));
          driftEl.style.display = h.drifting || charge > .02 ? 'block' : 'none';
          $('drift-label').textContent = charge >= .65 ? 'RELEASE TO BOOST' : 'BUILD YOUR DRIFT';
          document.querySelector('#drift-meter i').style.width = `${charge * 100}%`;
          const network = !practice && wasm.online_status ? wasm.online_status() : '';
          $('effect').textContent = network || (h.hit > .1 ? 'IMPACT · RECOVERING' : h.shield > .1 ? 'SHIELD ACTIVE' : h.grip > .1 ? 'GRIP ACTIVE' : h.boosting ? 'BOOST ACTIVE' : '');
          sound.update(h);
          if (h.countdown > 0.01) {
            stateEl.style.display = 'block';
            stateEl.textContent = Math.ceil(h.countdown);
          } else if (h.finished) {
            stateEl.style.display = 'none';
            if(!resultsShown) {
              resultsShown = true;
              $('result-title').textContent = h.place===1 ? 'VICTORY.' : 'RACE COMPLETE.';
              $('result-place').textContent = `P${h.place} / ${h.racers}`;
              $('result-time').textContent = formatTime(h.race_time);
              $('results').classList.remove('hidden');
              $('session-note').textContent = practice ? 'Try a different line. Save an item for the final lap.' : 'Race classified. Return to the garage to join or host the next race.';
              sound.cue(h.place===1 ? 1046 : 659, .7);
            }
          } else {
            stateEl.style.display = h.race_time > 0 && h.race_time < .65 ? 'block' : 'none';
            stateEl.textContent = 'GO';
          }
        } catch {}
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    };
