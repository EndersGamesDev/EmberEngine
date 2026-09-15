# ember-loader sources

`phase.rs` is the machine: which phase may follow which, what each event carries, and what an out-of-order drive is told. `progress.rs` is the download arithmetic — percent, mean rate, estimate and the stall test — where every value is optional because a response is not obliged to declare its length. `text.rs` is the wording, held in one place so six pages say the same thing. `event.rs` is the single event shape all of them produce.

`wasm.rs` is the only file that knows about a browser: it is the `cfg(target_arch = "wasm32")` surface the module emitted from `web/loader.ts.j2` drives, and it converts an event into a plain object. Everything else builds and is tested natively.
