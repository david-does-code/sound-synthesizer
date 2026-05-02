---
status: proposed
---

# Filter + filter envelope

Add a per-voice resonant lowpass with its own ADSR modulating the cutoff.
Single biggest sound-design unlock: turns the static triangle/saw pad into
a pluck, opens the door to wobble/sweep/wah, and is the foundation for
nearly every subtractive synth sound. Motivated by the
"and the government too" cover wanting fingerpicked-guitar character that
the current oscillator+amp-ADSR chain can't produce.

## Design choices

- **Topology**: TPT (Topology-Preserving Transform) state-variable filter,
  2-pole / 12 dB per octave. Stable to Nyquist (unlike classic Chamberlin),
  cheap, gives LP/HP/BP outputs from one structure. v1 exposes lowpass
  only; HP/BP wiring is essentially free if we want it later.
- **Modulation**: exponential. Cutoff is `base_hz * 2^(env_value * depth_octaves)`.
  Musical (one unit of envelope = one octave of sweep), avoids the dead
  feel of linear Hz modulation.
- **Filter envelope**: separate ADSR per voice. Reuse the existing
  `Envelope` struct — same state machine, just a second instance.
- **Default**: cutoff = 20 kHz, env depth = 0. Existing patterns sound
  identical with no changes.

## File-by-file

### New: `src/filter.rs`
- `pub struct SvfLowpass { ic1eq: f32, ic2eq: f32 }` — two integrator states.
- `process(&mut self, input: f32, cutoff_hz: f32, resonance: f32, sample_rate: f32) -> f32`.
  Implements TPT SVF: `g = tan(pi * cutoff / sr)`, `k = 2 - 2*resonance`, etc.
  Resonance 0..~0.97 (clamp to avoid self-oscillation blowing up).
- Cheap enough to run per-sample per-voice (8 voices × ~10 flops + one tan).
  Cache `g` if cutoff is constant for the buffer; recompute when modulated.

### `src/audio.rs`
- `Voice` gains: `filter: SvfLowpass`, `filter_env: Envelope`.
- New atomics, one slot per voice (mirroring `voice_adsr`):
  - `voice_filter: [AtomicU64; 8]` — packs base cutoff (u16 quantized log-Hz),
    resonance (u8), env depth in octaves (u8 fixed-point), reserved.
    Sentinel 0 = filter bypassed.
  - `voice_filter_adsr: [AtomicU64; 8]` — same packing as `voice_adsr`,
    sentinel 0 = "use amp envelope" (cheap default — one envelope drives both).
- Audio callback: per-sample, advance `filter_env`, compute modulated cutoff,
  run sample through SVF before summing into the mix bus.
- `EngineHandle`: add `set_voice_filter(slot, cutoff, resonance, env_depth)`
  and `set_voice_filter_adsr(slot, a, d, s, r)`.

### `src/render.rs`
- Mirror the per-voice filter in the offline renderer. Same code path
  conceptually — `Voice` already lives in `audio.rs` and is `pub`, so the
  renderer continues to reuse it. Just make sure filter state is initialized
  per voice and processed per sample.

### `src/pattern.rs`
- New per-track properties:
  - `name.cutoff: 800Hz` (base cutoff; default 20000)
  - `name.resonance: 0.4` (0..0.97; default 0)
  - `name.filter_env: 3.0` (octaves of upward sweep at peak; default 0)
  - `name.filter_attack: 5ms` (default = amp attack)
  - `name.filter_decay: 0.15` (default = amp decay)
  - `name.filter_sustain: 0.2` (default = amp sustain)
  - `name.filter_release: 100ms` (default = amp release)
- Same `parse_time` / `parse_freq` helpers; freq parser accepts `800`, `800Hz`,
  `1.2kHz`. Add to the existing properties map and store on `Track`.

### `src/sequencer.rs`
- In `pre_resolve`, after applying waveform/ADSR/gain to each track's voice
  slot, push the filter params via the new `EngineHandle` setters.

### CLAUDE.md
- Update the Architecture section: mention the new filter module and the
  new `voice_filter` / `voice_filter_adsr` atomics.
- Add the new pattern properties to the `pattern.rs` description.

## Test plan

1. **New unit test pattern** `patterns/test_filter.pat`: one pad track,
   sweep cutoff envelope from 200 Hz to ~5 kHz with 3 octaves of env depth.
   Render and listen — should clearly hear "wow" filter open.
2. **Resonance test**: same pattern with `resonance: 0.85` — should hear
   a pronounced peak at the cutoff sweep.
3. **Regression**: render `cm_expressive.pat` and `clocks_coldplay.pat`
   with no filter properties. Should be sample-identical to the
   pre-change render (filter bypassed).
4. **Live-mode smoke test**: `cargo run` with no args, play a few notes.
   Default sound unchanged.
5. **Apply to the song**: update `and_the_government_too.pat` — give the
   pad a moderate filter sweep (cutoff ~600 Hz, env depth 2.5 octaves,
   short filter decay) so it reads as "pluck pad" rather than "wash".
   Render and listen; verify the reverb tail also sounds more controlled
   (the filtered tail won't have the same fizzy top end).

## Out of scope (next time)

- 4-pole / Moore-ladder filter. 2-pole is enough to evaluate the impact.
- Highpass / bandpass mode selectors.
- LFO modulation of cutoff (covered by separate "LFO + vibrato" Phase 8 item).
- Key tracking (cutoff follows pitch). Would be ~10 lines but adds a
  parameter and isn't needed for the song.
- MIDI/CC live-tweak of cutoff in piano mode. Needs the ADSR-editor-style
  TUI work that's already its own track.

## Estimated effort

Half-day to a day. Filter math + atomics is the bulk; pattern parsing
is mechanical. The render-parity step is where the existing "voices live
in audio.rs but are reused by render.rs" structure pays off — no
duplication needed.
