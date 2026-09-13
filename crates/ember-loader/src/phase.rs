//! The phase machine: what may follow what, and what the page is told.
//!
//! The machine is driven from the browser, but it does not trust its driver.
//! Every call carries the clock, and every call is checked against the state
//! the load is actually in, so a page wired up in the wrong order produces a
//! visible failure rather than a percentage computed from a download that had
//! not started. That is also what makes the whole model testable here: a test
//! is a list of calls and timestamps, with no browser anywhere.
//!
//! Two tracks run at once. Host discovery and the bundle download start
//! together and neither waits for the other — the point of the loader is that
//! a page knows which server it will join long before its bundle is on the
//! machine — so a host failure leaves the bundle track running, exactly as a
//! page with no server still loads its game and says so.

use crate::event::{Event, Status};
use crate::progress::Progress;
use crate::text;

/// The phases of a load, in the order a page sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The loader itself is coming up.
    Boot,
    /// Host discovery.
    Host,
    /// The game bundle's bytes.
    Download,
    /// Compiling those bytes.
    Compile,
    /// The game's own initialisation.
    Init,
    /// The game's start, when the page hands one over.
    Start,
    /// Everything the page asked for is done.
    Ready,
}

impl Phase {
    /// The name a page sees.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boot => "boot",
            Self::Host => "host",
            Self::Download => "download",
            Self::Compile => "compile",
            Self::Init => "init",
            Self::Start => "start",
            Self::Ready => "ready",
        }
    }

    /// The phase a page named, or `None` when it named nothing this machine
    /// knows. An unknown name is a wiring mistake and is reported as one.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "boot" => Some(Self::Boot),
            "host" => Some(Self::Host),
            "download" => Some(Self::Download),
            "compile" => Some(Self::Compile),
            "init" => Some(Self::Init),
            "start" => Some(Self::Start),
            "ready" => Some(Self::Ready),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum State {
    #[default]
    Idle,
    Active,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, Default)]
struct Track {
    state: State,
    began_ms: f64,
}

impl Track {
    const fn active(self) -> bool {
        matches!(self.state, State::Active)
    }

    const fn done(self) -> bool {
        matches!(self.state, State::Done)
    }

    const fn started(self) -> bool {
        !matches!(self.state, State::Idle)
    }
}

/// The default quiet period before a download is called stalled.
pub const STALL_MS: f64 = 5_000.0;

/// The load, as a state machine.
#[derive(Debug, Clone)]
pub struct Machine {
    started_ms: f64,
    stall_ms: f64,
    boot: Track,
    host: Track,
    download: Track,
    compile: Track,
    init: Track,
    start: Track,
    progress: Progress,
    stalled: bool,
    ended: bool,
}

impl Machine {
    /// A machine whose clock starts at `now_ms`.
    #[must_use]
    pub fn new(now_ms: f64, stall_ms: f64) -> Self {
        Self {
            started_ms: now_ms,
            stall_ms: if stall_ms.is_finite() && stall_ms > 0.0 {
                stall_ms
            } else {
                STALL_MS
            },
            boot: Track::default(),
            host: Track::default(),
            download: Track::default(),
            compile: Track::default(),
            init: Track::default(),
            start: Track::default(),
            progress: Progress::new(now_ms),
            stalled: false,
            ended: false,
        }
    }

    /// Whether the load has finished or failed on the bundle track.
    ///
    /// A background host re-rank may still settle after this becomes true.
    #[must_use]
    pub const fn ended(&self) -> bool {
        self.ended
    }

    const fn track(&self, phase: Phase) -> Track {
        match phase {
            Phase::Boot => self.boot,
            Phase::Host => self.host,
            Phase::Download => self.download,
            Phase::Compile => self.compile,
            Phase::Init => self.init,
            Phase::Start | Phase::Ready => self.start,
        }
    }

    const fn track_mut(&mut self, phase: Phase) -> &mut Track {
        match phase {
            Phase::Boot => &mut self.boot,
            Phase::Host => &mut self.host,
            Phase::Download => &mut self.download,
            Phase::Compile => &mut self.compile,
            Phase::Init => &mut self.init,
            Phase::Start | Phase::Ready => &mut self.start,
        }
    }

    fn event(&self, phase: Phase, status: Status, now_ms: f64) -> Event {
        let began = self.track(phase).began_ms;
        let phase_ms = if self.track(phase).started() {
            now_ms - began
        } else {
            0.0
        };
        let mut ev = Event::new(phase, status, now_ms - self.started_ms, phase_ms);
        if matches!(phase, Phase::Download) {
            ev.loaded = Some(self.progress.loaded);
            ev.total = self.progress.total;
            ev.percent = self.progress.percent();
            ev.rate_bps = self.progress.rate_bps();
            ev.eta_ms = self.progress.eta_ms();
            ev.stalled = self.stalled;
            if self.stalled {
                ev.stalled_ms = now_ms - self.progress.last_ms;
            }
        }
        ev.text = text::render(&ev);
        ev
    }

    fn refuse(&self, phase: Phase, now_ms: f64, why: &str) -> Event {
        let mut ev = Event::new(phase, Status::Fail, now_ms - self.started_ms, 0.0);
        ev.reason = Some("out-of-order".to_owned());
        ev.detail = Some(why.to_owned());
        ev.text = text::render(&ev);
        ev
    }

    /// Refuse a phase name this machine does not know.
    ///
    /// A page that names a phase wrongly has a wiring mistake, and a silent
    /// no-op there is a load that stops reporting halfway with nothing said.
    #[must_use]
    pub fn unknown(&self, now_ms: f64, name: &str) -> Event {
        let mut ev = Event::new(Phase::Boot, Status::Fail, now_ms - self.started_ms, 0.0);
        ev.reason = Some("unknown-phase".to_owned());
        ev.detail = Some(format!("no phase is called {name}"));
        ev.text = text::render(&ev);
        ev
    }

    /// Open the load. Every other call is refused until this one has run.
    pub fn begin(&mut self, now_ms: f64) -> Event {
        if self.boot.started() {
            return self.refuse(Phase::Boot, now_ms, "the load has already begun");
        }
        self.boot = Track {
            state: State::Active,
            began_ms: now_ms,
        };
        self.event(Phase::Boot, Status::Begin, now_ms)
    }

    /// What must already have happened before `phase` may begin.
    const fn ready_for(&self, phase: Phase) -> Option<&'static str> {
        match phase {
            Phase::Boot => Some("the load opens once"),
            Phase::Host | Phase::Download => None,
            Phase::Compile => {
                if self.download.started() {
                    None
                } else {
                    Some("the download has not begun")
                }
            }
            Phase::Init => {
                if self.compile.started() {
                    None
                } else {
                    Some("nothing has been compiled")
                }
            }
            Phase::Start | Phase::Ready => {
                if self.init.done() {
                    None
                } else {
                    Some("the engine is not initialised")
                }
            }
        }
    }

    /// Begin `phase`.
    ///
    /// Compilation may begin while the download is still running: a streaming
    /// compile starts at the first byte and that overlap is the whole reason
    /// the two are separate phases rather than one "loading" spinner.
    pub fn enter(&mut self, now_ms: f64, phase: Phase) -> Event {
        if self.ended {
            return self.refuse(phase, now_ms, "the load has already finished");
        }
        if !self.boot.started() {
            return self.refuse(phase, now_ms, "the load has not begun");
        }
        if matches!(phase, Phase::Ready) {
            return self.refuse(phase, now_ms, "ready is completed, never entered");
        }
        if self.track(phase).started() {
            return self.refuse(phase, now_ms, "that phase has already begun");
        }
        if let Some(why) = self.ready_for(phase) {
            return self.refuse(phase, now_ms, why);
        }
        if matches!(phase, Phase::Download) {
            self.progress = Progress::new(now_ms);
        }
        *self.track_mut(phase) = Track {
            state: State::Active,
            began_ms: now_ms,
        };
        self.event(phase, Status::Begin, now_ms)
    }

    /// Reopen host discovery after its previous ranking settled.
    ///
    /// The bundle download deliberately overlaps the first ranking. Once the
    /// bundle is ready, the browser may ask again if that early pass found no
    /// host or used a protocol the bundle disproves. The page is playable at
    /// that point, so this track may reopen after `ready`; it cannot start
    /// discovery for the first time or overlap a pass that is still running.
    pub fn rerank(&mut self, now_ms: f64) -> Event {
        if !self.boot.started() {
            return self.refuse(Phase::Host, now_ms, "the load has not begun");
        }
        if self.host.active() {
            return self.refuse(Phase::Host, now_ms, "host discovery is still running");
        }
        if !self.host.started() {
            return self.refuse(Phase::Host, now_ms, "host discovery has not run");
        }
        self.host = Track {
            state: State::Active,
            began_ms: now_ms,
        };
        self.event(Phase::Host, Status::Begin, now_ms)
    }

    /// What must already have finished before `phase` may finish.
    const fn closable(&self, phase: Phase) -> Option<&'static str> {
        match phase {
            Phase::Compile => {
                if self.download.done() {
                    None
                } else {
                    Some("the download has not finished")
                }
            }
            Phase::Init => {
                if self.compile.done() {
                    None
                } else {
                    Some("the compile has not finished")
                }
            }
            Phase::Ready => {
                if !self.init.done() {
                    Some("the engine is not initialised")
                } else if self.start.active() {
                    Some("the game has not started")
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Finish `phase`. Finishing `ready` ends the load.
    pub fn done(&mut self, now_ms: f64, phase: Phase) -> Event {
        if self.ended && !matches!(phase, Phase::Host) {
            return self.refuse(phase, now_ms, "the load has already finished");
        }
        if let Some(why) = self.closable(phase) {
            return self.refuse(phase, now_ms, why);
        }
        if matches!(phase, Phase::Ready) {
            self.ended = true;
            let mut ev = self.event(Phase::Ready, Status::Done, now_ms);
            ev.phase_ms = now_ms - self.started_ms;
            ev.text = text::render(&ev);
            return ev;
        }
        if !self.track(phase).active() {
            return self.refuse(phase, now_ms, "that phase is not running");
        }
        if matches!(phase, Phase::Download) {
            self.stalled = false;
        }
        self.track_mut(phase).state = State::Done;
        self.event(phase, Status::Done, now_ms)
    }

    /// Record the declared length of the download, or its absence.
    pub fn total(&mut self, now_ms: f64, total: Option<u64>) -> Event {
        if self.ended || !self.download.active() {
            return self.refuse(Phase::Download, now_ms, "no download is running");
        }
        self.progress.total = total.filter(|t| *t > 0);
        self.event(Phase::Download, Status::Progress, now_ms)
    }

    /// Record one arrived chunk.
    pub fn bytes(&mut self, now_ms: f64, bytes: u64) -> Event {
        if self.ended || !self.download.active() {
            return self.refuse(Phase::Download, now_ms, "no download is running");
        }
        self.progress.chunk(now_ms, bytes);
        self.stalled = false;
        self.event(Phase::Download, Status::Progress, now_ms)
    }

    /// Let the clock run. Returns an event only when the stall state flips,
    /// so a page may call this on every frame for nothing.
    pub fn tick(&mut self, now_ms: f64) -> Option<Event> {
        if self.ended || !self.download.active() {
            return None;
        }
        let quiet = self.progress.stalled(now_ms, self.stall_ms);
        if quiet == self.stalled {
            return None;
        }
        self.stalled = quiet;
        Some(self.event(Phase::Download, Status::Progress, now_ms))
    }

    /// Fail `phase` with a code and a message.
    ///
    /// A host failure is not the end of the load: the game still downloads,
    /// still starts, and says there is no server. Every other phase is on the
    /// bundle track, and a bundle that did not arrive has nothing after it.
    pub fn fail(&mut self, now_ms: f64, phase: Phase, reason: &str, detail: &str) -> Event {
        if self.ended && !matches!(phase, Phase::Host) {
            return self.refuse(phase, now_ms, "the load has already finished");
        }
        if !self.track(phase).started() {
            return self.refuse(phase, now_ms, "that phase has not begun");
        }
        self.track_mut(phase).state = State::Failed;
        if !matches!(phase, Phase::Host) {
            self.ended = true;
        }
        let mut ev = self.event(phase, Status::Fail, now_ms);
        ev.reason = Some(reason.to_owned());
        ev.detail = Some(detail.to_owned());
        ev.text = text::render(&ev);
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::{Machine, Phase};
    use crate::event::Status;

    fn opened(now: f64) -> Machine {
        let mut m = Machine::new(now, 5_000.0);
        let ev = m.begin(now);
        assert_eq!((ev.phase, ev.status), (Phase::Boot, Status::Begin));
        m
    }

    /// A load that runs the way a page runs it, with the two tracks
    /// overlapping, produces exactly this sequence.
    #[test]
    fn a_whole_load_reports_every_phase_once() {
        let mut m = opened(0.0);
        let mut seen = vec![];
        let mut note = |ev: crate::event::Event| {
            assert_ne!(
                ev.status,
                Status::Fail,
                "unexpected refusal: {:?}",
                ev.detail
            );
            seen.push(format!("{}/{}", ev.phase.as_str(), ev.status.as_str()));
        };
        note(m.enter(1.0, Phase::Host));
        note(m.enter(1.0, Phase::Download));
        note(m.total(20.0, Some(1_000)));
        note(m.done(300.0, Phase::Host));
        note(m.enter(320.0, Phase::Compile));
        note(m.bytes(400.0, 600));
        note(m.bytes(900.0, 400));
        note(m.done(900.0, Phase::Download));
        note(m.done(1_200.0, Phase::Compile));
        note(m.enter(1_200.0, Phase::Init));
        note(m.done(1_800.0, Phase::Init));
        note(m.enter(1_800.0, Phase::Start));
        note(m.done(1_900.0, Phase::Start));
        note(m.done(1_900.0, Phase::Ready));
        assert_eq!(
            seen,
            vec![
                "host/begin",
                "download/begin",
                "download/progress",
                "host/done",
                "compile/begin",
                "download/progress",
                "download/progress",
                "download/done",
                "compile/done",
                "init/begin",
                "init/done",
                "start/begin",
                "start/done",
                "ready/done",
            ]
        );
        assert!(m.ended());
    }

    #[test]
    fn the_download_event_carries_the_arithmetic() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        m.total(0.0, Some(1_000));
        m.bytes(1_000.0, 100);
        let ev = m.bytes(2_000.0, 400);
        assert_eq!(ev.loaded, Some(500));
        assert_eq!(ev.total, Some(1_000));
        assert_eq!(ev.percent, Some(50));
        assert_eq!(ev.rate_bps, Some(500.0));
        assert_eq!(ev.eta_ms, Some(1_000.0));
        assert_eq!(
            ev.text,
            "downloading 500 B of 1.0 kB · 50% · 500 B/s · 1.0 s left"
        );
    }

    #[test]
    fn a_response_with_no_length_still_reports_bytes() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        m.total(0.0, None);
        m.bytes(1_000.0, 2_000);
        let ev = m.bytes(2_000.0, 2_000);
        assert_eq!(ev.total, None);
        assert_eq!(ev.percent, None);
        assert_eq!(ev.eta_ms, None);
        assert_eq!(ev.text, "downloading 4.0 kB · 4.0 kB/s");
    }

    #[test]
    fn a_declared_length_of_zero_is_no_length() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        let ev = m.total(0.0, Some(0));
        assert_eq!(ev.total, None);
    }

    #[test]
    fn a_stall_arms_once_and_clears_on_the_next_chunk() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        m.total(0.0, Some(1_000));
        m.bytes(1_000.0, 100);
        assert!(m.tick(5_999.0).is_none(), "not quiet long enough yet");
        let armed = m.tick(6_000.0).expect("five seconds of silence");
        assert!(armed.stalled);
        assert_eq!(armed.stalled_ms, 5_000.0);
        assert_eq!(
            armed.text,
            "downloading 100 B of 1.0 kB · 10% · no data for 5.0 s"
        );
        assert!(m.tick(7_000.0).is_none(), "the stall is announced once");
        let back = m.bytes(7_500.0, 100);
        assert!(!back.stalled);
        assert!(m.tick(7_600.0).is_none());
    }

    #[test]
    fn a_finished_download_stops_ticking() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        m.bytes(10.0, 10);
        m.done(20.0, Phase::Download);
        assert!(m.tick(60_000.0).is_none());
    }

    #[test]
    fn nothing_runs_before_the_load_opens() {
        let mut m = Machine::new(0.0, 5_000.0);
        let ev = m.enter(1.0, Phase::Download);
        assert_eq!(ev.status, Status::Fail);
        assert_eq!(ev.reason.as_deref(), Some("out-of-order"));
        assert_eq!(ev.detail.as_deref(), Some("the load has not begun"));
        assert_eq!(ev.text, "the download failed: the load has not begun");
    }

    #[test]
    fn the_load_opens_once() {
        let mut m = opened(0.0);
        assert_eq!(m.begin(1.0).status, Status::Fail);
    }

    #[test]
    fn every_out_of_order_drive_is_refused() {
        let refusal = |build: &dyn Fn(&mut Machine) -> crate::event::Event, detail: &str| {
            let mut m = opened(0.0);
            let ev = build(&mut m);
            assert_eq!(ev.status, Status::Fail, "expected a refusal for {detail}");
            assert_eq!(ev.detail.as_deref(), Some(detail));
        };
        refusal(&|m| m.bytes(1.0, 10), "no download is running");
        refusal(&|m| m.total(1.0, Some(10)), "no download is running");
        refusal(
            &|m| m.enter(1.0, Phase::Compile),
            "the download has not begun",
        );
        refusal(&|m| m.enter(1.0, Phase::Init), "nothing has been compiled");
        refusal(
            &|m| m.enter(1.0, Phase::Start),
            "the engine is not initialised",
        );
        refusal(
            &|m| m.enter(1.0, Phase::Ready),
            "ready is completed, never entered",
        );
        refusal(
            &|m| m.done(1.0, Phase::Ready),
            "the engine is not initialised",
        );
        refusal(&|m| m.done(1.0, Phase::Host), "that phase is not running");
        refusal(
            &|m| m.fail(1.0, Phase::Init, "x", "y"),
            "that phase has not begun",
        );
        refusal(
            &|m| {
                m.enter(1.0, Phase::Host);
                m.enter(2.0, Phase::Host)
            },
            "that phase has already begun",
        );
        refusal(
            &|m| {
                m.enter(1.0, Phase::Download);
                m.enter(2.0, Phase::Compile);
                m.done(3.0, Phase::Compile)
            },
            "the download has not finished",
        );
        refusal(
            &|m| {
                m.enter(1.0, Phase::Download);
                m.enter(2.0, Phase::Compile);
                m.done(3.0, Phase::Download);
                m.enter(4.0, Phase::Init);
                m.done(5.0, Phase::Init)
            },
            "the compile has not finished",
        );
    }

    /// The game starts before `ready` is claimed, when a page hands one over.
    #[test]
    fn a_running_start_holds_ready_back() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        m.done(1.0, Phase::Download);
        m.enter(1.0, Phase::Compile);
        m.done(2.0, Phase::Compile);
        m.enter(2.0, Phase::Init);
        m.done(3.0, Phase::Init);
        m.enter(3.0, Phase::Start);
        let ev = m.done(4.0, Phase::Ready);
        assert_eq!(ev.detail.as_deref(), Some("the game has not started"));
        m.done(5.0, Phase::Start);
        assert_eq!(m.done(6.0, Phase::Ready).status, Status::Done);
    }

    /// A page with no server still loads its game. This is the whole reason
    /// discovery is a separate track rather than a step.
    #[test]
    fn a_host_failure_leaves_the_bundle_track_running() {
        let mut m = opened(0.0);
        m.enter(1.0, Phase::Host);
        m.enter(1.0, Phase::Download);
        let ev = m.fail(500.0, Phase::Host, "no-host", "no server answered");
        assert_eq!(ev.status, Status::Fail);
        assert_eq!(ev.reason.as_deref(), Some("no-host"));
        assert_eq!(ev.text, "no server: no server answered");
        assert!(!m.ended());
        assert_eq!(m.bytes(600.0, 10).status, Status::Progress);
    }

    #[test]
    fn host_discovery_can_rerank_after_failure_or_success() {
        let mut failed = opened(0.0);
        failed.enter(1.0, Phase::Host);
        failed.fail(2.0, Phase::Host, "no-host", "no server answered");
        assert_eq!(failed.rerank(3.0).status, Status::Begin);
        assert_eq!(failed.done(4.0, Phase::Host).status, Status::Done);

        let mut finished = opened(0.0);
        finished.enter(1.0, Phase::Host);
        finished.done(2.0, Phase::Host);
        assert_eq!(finished.rerank(3.0).status, Status::Begin);
        assert_eq!(finished.done(4.0, Phase::Host).status, Status::Done);
    }

    #[test]
    fn a_background_host_rerank_settles_after_ready() {
        let finish_bundle = |machine: &mut Machine| {
            machine.enter(3.0, Phase::Download);
            machine.done(4.0, Phase::Download);
            machine.enter(4.0, Phase::Compile);
            machine.done(5.0, Phase::Compile);
            machine.enter(5.0, Phase::Init);
            machine.done(6.0, Phase::Init);
            assert_eq!(machine.done(7.0, Phase::Ready).status, Status::Done);
        };

        let mut found = opened(0.0);
        found.enter(1.0, Phase::Host);
        found.fail(2.0, Phase::Host, "no-host", "no server answered");
        finish_bundle(&mut found);
        assert_eq!(found.rerank(8.0).status, Status::Begin);
        assert_eq!(found.done(9.0, Phase::Host).status, Status::Done);

        let mut silent = opened(0.0);
        silent.enter(1.0, Phase::Host);
        silent.done(2.0, Phase::Host);
        finish_bundle(&mut silent);
        assert_eq!(silent.rerank(8.0).status, Status::Begin);
        assert_eq!(
            silent
                .fail(9.0, Phase::Host, "discovery", "mirror unavailable")
                .status,
            Status::Fail
        );
    }

    #[test]
    fn host_rerank_refuses_a_missing_or_running_first_pass() {
        let mut missing = opened(0.0);
        let ev = missing.rerank(1.0);
        assert_eq!(ev.status, Status::Fail);
        assert_eq!(ev.detail.as_deref(), Some("host discovery has not run"));

        let mut running = opened(0.0);
        running.enter(1.0, Phase::Host);
        let ev = running.rerank(2.0);
        assert_eq!(ev.status, Status::Fail);
        assert_eq!(
            ev.detail.as_deref(),
            Some("host discovery is still running")
        );
    }

    #[test]
    fn a_bundle_failure_ends_the_load() {
        let mut m = opened(0.0);
        m.enter(1.0, Phase::Download);
        let ev = m.fail(50.0, Phase::Download, "http", "404 Not Found");
        assert_eq!(ev.text, "the download failed: 404 Not Found");
        assert!(m.ended());
        assert_eq!(m.bytes(60.0, 10).status, Status::Fail);
        assert_eq!(
            m.enter(60.0, Phase::Compile).detail.as_deref(),
            Some("the load has already finished")
        );
    }

    #[test]
    fn a_failure_with_no_message_still_says_something() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        m.done(1.0, Phase::Download);
        m.enter(1.0, Phase::Compile);
        m.done(2.0, Phase::Compile);
        m.enter(2.0, Phase::Init);
        let ev = m.fail(3.0, Phase::Init, "engine", "");
        assert_eq!(ev.text, "the engine could not start: engine");
    }

    #[test]
    fn nothing_follows_a_finished_load() {
        let mut m = opened(0.0);
        m.enter(0.0, Phase::Download);
        m.done(1.0, Phase::Download);
        m.enter(1.0, Phase::Compile);
        m.done(2.0, Phase::Compile);
        m.enter(2.0, Phase::Init);
        m.done(3.0, Phase::Init);
        let ready = m.done(3.0, Phase::Ready);
        assert_eq!(ready.text, "ready in 0.0 s");
        assert_eq!(m.done(4.0, Phase::Ready).status, Status::Fail);
        assert_eq!(m.enter(4.0, Phase::Host).status, Status::Fail);
    }

    #[test]
    fn the_clocks_are_the_pages_own() {
        let mut m = opened(1_000.0);
        m.enter(1_500.0, Phase::Download);
        let ev = m.bytes(2_500.0, 10);
        assert_eq!(ev.elapsed_ms, 1_500.0);
        assert_eq!(ev.phase_ms, 1_000.0);
    }

    #[test]
    fn phase_names_round_trip() {
        for phase in [
            Phase::Boot,
            Phase::Host,
            Phase::Download,
            Phase::Compile,
            Phase::Init,
            Phase::Start,
            Phase::Ready,
        ] {
            assert_eq!(Phase::parse(phase.as_str()), Some(phase));
        }
        assert_eq!(Phase::parse("engine"), None);
    }
}
