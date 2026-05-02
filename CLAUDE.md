# Sound Synthesizer — Claude Code Context

## What This Is

A learning project: building a sound synthesizer in Rust to learn music theory and DSP.
See PLAN.md for the roadmap and current progress.

## Architecture

- `src/audio.rs` — Polyphonic audio engine with 8 pitched voices plus 3 dedicated drum
  voices (kick, snare, hi-hat) synthesized inside the callback. Lock-free communication:
  `voice_commands` (`[AtomicU32; 8]` for immediate piano play/release), `voice_active`
  (`[AtomicBool; 8]`), global `waveform` (`AtomicU8`, used by piano mode + as a default),
  per-voice `voice_waveforms` (`[AtomicU8; 8]`, lets each track have its own timbre,
  read by the audio callback), global ADSR params (packed `AtomicU64`) plus per-voice
  ADSR (`voice_adsr: [AtomicU64; 8]`, 0 = use global), per-voice gain
  (`voice_gains: [AtomicU32; 8]`, f32 bits as u32, default 1.0), per-drum gain
  (`drum_gains: [AtomicU32; 3]`), `drum_schedule` (`[AtomicU64; 3]` packed
  target-sample + velocity for sample-accurate drum triggers), `voice_events`
  (`[AtomicU64; 8]` packed (kind, velocity, midi, sample) for sample-accurate note
  scheduling), and `sample_clock` (`AtomicU64`, advanced per-frame by the callback).
  Each voice and drum carries a velocity field set on trigger. Sound-design
  extensions: per-voice `voice_click` (`[AtomicU32; 8]`, semitones of pitch
  transient on note-on) and `voice_sub` (`[AtomicU32; 8]`, sub-octave sine
  layer amplitude); per-voice resonant lowpass filter via
  `voice_cutoff`/`voice_resonance`/`voice_filter_env` (`[AtomicU32; 8]`
  each, defaults bypassed) plus `voice_filter_adsr` (`[AtomicU64; 8]`,
  0 = follow amp ADSR); master `reverb_mix` (`AtomicU32`) drives a single
  Schroeder reverb on the final mix. Contains `Waveform`, `Drum`, `Voice`
  (now `pub` so the offline renderer reuses it; carries an `SvfLowpass`
  + filter envelope), `DrumVoice`, and `EngineHandle` — a clonable
  Send + Sync control surface used by the sequencer to schedule drums +
  pitched notes and to set per-voice waveforms, ADSR, gain, click, sub,
  filter, filter ADSR, and master reverb.
- `src/envelope.rs` — ADSR envelope generator. Per-sample state machine (Idle → Attack →
  Decay → Sustain → Release → Idle). Lives inside the audio callback closure.
- `src/filter.rs` — TPT state-variable lowpass (2-pole, 12 dB/oct,
  Andrew Simper topology). Per-voice resonant LPF with envelope-modulated
  cutoff; bypassed by default (cutoff 20 kHz, env depth 0) so voices that
  don't ask for one pay zero per-sample cost. Cutoff modulation is
  exponential — `cutoff = base * 2^(env * env_octaves)`.
- `src/pluck.rs` — Karplus-Strong plucked-string physical model. A delay
  line of length `sample_rate / freq` is filled with noise on trigger and
  self-feeds through a one-zero lowpass plus a small decay multiplier
  each loop, giving very convincing guitar/harp/koto-like tone for ~50
  lines of code. Voice switches between oscillator and pluck via
  `pluck_enabled`; the lowpass filter still applies on top of either path.
- `src/keyboard.rs` — Reads raw keyboard events from Linux evdev (`/dev/input/`).
  Sends note, waveform, octave, mode toggle, and arrow key events over an MPSC channel.
- `src/pattern.rs` — Pattern file format and parser. Defines `Pattern` (with `bpm`,
  `swing`, `reverb`, `steps_per_beat`, `sections: Vec<Section>`, `song: Vec<SongEntry>`),
  `Section` (with `name`, `steps`, optional `bpm`/`swing`/`steps_per_beat`
  overrides, `tracks`), `Track` (with optional `wave`,
  `attack`/`decay`/`sustain`/`release`, `gain`, `gate`, `click`, `sub`,
  `cutoff`/`resonance`/`filter_env` and `filter_attack`/`filter_decay`/
  `filter_sustain`/`filter_release` properties),
  `TrackKind`
  (`Drum(Vec<f32>)` velocities / `Notes(Vec<Cell>)` / `Chord(Vec<ChordCell>)`), `Cell`
  (`Rest | Sustain | Note(u8, f32)` with velocity) and `ChordCell` (`Rest | Sustain |
  Chord(Vec<(u8, f32)>)`), and `PatternParseError`. Format: `bpm:`/`steps:`/`swing:`
  global headers, then optional `[section]` headers (with per-section `bpm:`/`swing:`),
  track rows, per-track property lines (`name.wave: square`, `name.attack: 200ms`,
  `name.gain: 1.5`, `name.gate: 0.5`, `name.octave: 4`), and an optional
  `song: intro verse x2 chorus outro` chain. Velocity: drums use `X` (accent 1.0),
  `x` (normal 0.7), `o` (ghost 0.35); notes use `C4!` (accent) / `C4?` (ghost).
  Time values accept seconds (`0.2`) or milliseconds (`200ms`). Two-pass parser:
  pass 1 collects properties globally, pass 2 builds sections/tracks/song. Auto-detects
  per row: drum rows contain only `xXo-./whitespace`; chord rows contain at least one
  unambiguous chord token; everything else is a note row.
- `src/sequencer.rs` — Step sequencer that plays a `Pattern` via a background thread.
  Uses sample-accurate scheduling via `EngineHandle`: pre-computes the absolute audio
  sample for each step and writes it to per-drum/per-voice atomic slots, so playback
  timing is independent of scheduler thread wall-clock jitter. **Voice allocation is
  global across all sections** — `pre_resolve` walks every section and assigns each
  unique pitched/chord track name a stable voice slot, so a `bass` line that appears
  in both verse and chorus shares one voice slot and its envelope state carries cleanly
  across the section boundary. Pre-resolve also applies per-track waveform, ADSR, and
  gain to the engine once at startup, and computes per-section `samples_per_step` from
  each section's BPM (falling back to global). The play loop walks the song chain with
  per-section timing, applies swing offset to odd steps, dispatches velocity-aware
  note-on/drum events, and schedules automatic note-offs when gate < 1.0. At section
  transitions, voices owned by tracks that don't appear in the new section get released.
  Releases all owned voices on stop. Lookahead is ~100 ms.
- `src/main.rs` — Two interactive modes (piano + ADSR editor, Tab toggles) plus CLI
  modes: `cargo run -- --play <file.pat>` loads a pattern and plays it in a loop
  until Enter is pressed; `cargo run -- --render <file.pat> <out.wav>` renders one
  pass of the song to a 16-bit mono 44.1 kHz WAV (uses the offline renderer in
  `src/render.rs`). `--help` lists usage. To listen to a rendered WAV, use a
  system player: `mpv out.wav` (preferred), or `aplay`/`paplay`.
- `src/render.rs` — Offline (non-realtime) WAV renderer. Walks one pass of
  the pattern's song chain and synthesizes samples using the same `Voice` /
  `DrumVoice` primitives as the live engine, then runs the buffer through
  the master `Reverb`. Single-threaded, so it skips the atomic plumbing
  and applies events directly. Output is 16-bit mono 44.1 kHz.
- `src/reverb.rs` — Schroeder reverb (4 parallel feedback comb filters with
  one-pole lowpass damping in the loop, then 2 series allpass filters).
  Damping kills the metallic ring of the textbook design and produces a
  warmer, more natural room sound. Used by both the live audio callback
  and the offline renderer.
- `src/visualizer.rs` — Renders waveforms and ADSR envelopes using Unicode braille characters
  (2×4 dot grid per character). Shared `render_braille` function for both.
- `src/notes.rs` — Keyboard layout diagram.
- `patterns/` — Example `.pat` files (`four_on_the_floor.pat`, `cm_expressive.pat` which
  demos all 5d-5i features, and other diagnostic/demo patterns).

## Key Design Decisions

- **evdev over terminal input**: Terminals don't send key release events. We read
  `/dev/input/` directly for true press/release, which requires the `input` group.
- **Lock-free polyphony**: 8 voices, each with its own phase/freq/envelope inside the
  callback. The main thread sends play/release commands via `[AtomicU32; 8]`. The callback
  reports voice liveness via `[AtomicBool; 8]` — critical distinction: "no pending command"
  (`CMD_IDLE`) ≠ "voice is free" (`voice_active == false`). Getting this wrong caused all
  notes to steal voice 0.
- **Phase accumulation**: Track oscillator phase as 0.0–1.0 and increment by `freq/sample_rate`
  each sample. Avoids floating-point drift that occurs with `sin(2π × freq × t)` over time.
- **Voice allocation**: Main thread maps MIDI notes → voice indices (`HashMap<u8, usize>`).
  NoteOn finds a free voice (via `voice_active`), NoteOff releases the specific voice.
  When all 8 voices are busy, voice 0 is stolen.
- **ADSR via packed AtomicU64**: Four f32 params are quantized to u16 and packed into a
  single u64 for atomic transfer. Each voice has its own envelope state machine.
- **Gain staging**: Voices are summed (each multiplied by its per-voice gain) and
  scaled by `0.4 / √8` to prevent clipping. Drums are mixed in separately at 0.5 gain
  (each multiplied by its per-drum gain) on top of the pitched voices.
- **Drum synthesis recipes**: Each drum is a tiny formula run per sample. Kick = pitch-swept
  sine (150 Hz → 40 Hz, exp decay). Snare = white noise + 180 Hz body tone. HiHat = fast-
  decaying noise. Each voice deactivates only once amplitude falls below 0.001 (no hard
  time cutoff — that caused tail clicks at exactly the inter-hit interval).
- **Sample-accurate sequencer scheduling**: The sequencer writes absolute sample numbers
  to per-drum atomic slots; the audio callback compares against `sample_clock` each frame
  and triggers when the time arrives. The wall-clock sleep in the sequencer thread only
  controls *when* events get scheduled, not *when* they play. ~100 ms of lookahead is
  needed to absorb audio buffer batching (smaller lookahead caused occasional late hits
  on Linux desktop audio).
- **Live area redraw**: Each mode has a fixed-height region that redraws in place via
  cursor-up. Mode switches clear the entire screen (`\x1b[2J\x1b[H`).
- **Raw mode newlines**: Headers printed before raw mode use `println!`. Headers printed
  during raw mode (mode switches) use `raw_println` which converts `\n` to `\r\n`.

## Development Notes

- Linux-only (evdev dependency). No plans for cross-platform yet.
- `cargo run` to test — requires a real terminal (not a backgrounded process).
- Number keys 1-4 switch waveforms; Z/X shift octave; Tab toggles ADSR editor; arrows
  navigate/adjust in ADSR mode. All handled in `keyboard.rs`.
- Control keys must be physical scancodes (evdev), not characters — matters for non-US layouts
  (e.g., `[`/`]` are AltGr combos on Nordic keyboards, so `KEY_LEFTBRACE` won't fire).
- Arrow keys allow repeat events (value == 2) for continuous adjustment when held.
- Octave offset is tracked in main.rs and applied to base MIDI notes. Note names are computed
  dynamically via `midi_to_name()` rather than hardcoded.
- Generated WAV files are gitignored.

## What's Next

Phases 1-4 done. Phase 5 mostly done: parser, drums, sequencer with
sample-accurate timing, melodic notes (5a), chord shorthand (5b), song
sections + chains (5c), per-track ADSR (5d), volume/mixing (5e),
velocity (5f), gate (5g), swing (5h), per-section BPM (5i), and **WAV
export (5l)** — `cargo run -- --render in.pat out.wav`.

Sound-design layer added on top of 5l: hammer click (`name.click`),
sub-octave layer (`name.sub`), master reverb (`reverb:` global header),
hi-hat lowpass, `steps_per_beat` for non-16th-grid songs, and
**resonant lowpass + filter envelope** (`name.cutoff`, `name.resonance`,
`name.filter_env`, plus optional `name.filter_attack/decay/sustain/release`
falling back to amp ADSR), **Karplus-Strong pluck synthesis**
(`name.model: pluck`, optional `name.pluck_decay`, `name.pluck_brightness`)
for plucked-string tones the oscillator can't reach, and **per-voice
pitch vibrato** (`name.vibrato_rate: 5hz`, `name.vibrato_depth: 0.15`
semitones) — sine LFO modulating oscillator pitch. Phase persists across
notes so stacked voices with similar rates drift naturally relative to
each other; that's most of what makes a vocal harmony stack feel human.
Pluck path is intentionally not modulated (delay-line resampling is its
own problem). Also added **per-voice unison detune** (`name.unison: 3`,
`name.detune: 18` cents) — stacks 2-3 oscillators per voice spread by
±cents/2, summed with √N normalization (decorrelated sum, not coherent).
This is the "supersaw" trick that turns a single saw into a string section.
Skipped on the pluck path. The detune-beat lives in the upper harmonics,
so any voice using unison wants a relatively open filter (cutoff ≥ ~3 kHz)
or the interesting beating gets filtered out. See
`patterns/harmony_stack_demo.pat` for the full demo.

**Pick next session by mood:**
- **Add a lead line to atgt** (in progress — pipeline set up, melody not
  yet transcribed). The Zucker cover (`patterns/and_the_government_too.pat`)
  has chords + bass + drums + plucked-string pad. Vocal stem already
  extracted to disk via yt-dlp + demucs (see "Vocal extraction pipeline"
  below) — the next session should run `basic-pitch` on the stem (or
  transcribe by ear) to get the topline, then add it as a `lead` track
  in the existing pattern. Try sine first (clean), then filtered saw,
  then pluck — and consider a second user-composed lead variant in
  parallel since both `(a) faithful` and `(b) original` lead options
  were left on the table.
- **5j + 5k**: TUI grid view + live step editing. Biggest composition
  unlock — turns the iteration loop from "edit-render-listen" into live.
- **Strings**: vibrato + unison/detune both done. Remaining moves toward
  "real strings": (a) **amp tremolo** (LFO on amplitude — easy mirror of
  the vibrato code), (b) **chorus** as a master send/insert (detune-only
  gets you ensemble thickness; chorus adds the modulating-delay shimmer
  that real recorded sections have from mic placement + reverb tails),
  (c) **per-unison-oscillator vibrato phases** (currently all detuned
  oscillators share one LFO; staggering them would give a "section vibrato"
  feel where players don't wobble in sync).
- **Phase 8 starters**: portamento, 4-pole/ladder filter upgrade,
  key-tracking on the filter cutoff. See PLAN.md.
- **Velocity humanization**: small per-step timing / velocity jitter in
  the sequencer, addresses the "rigidly quantized" complaint without
  touching pattern files.
- **More songs**: transcribe something other than Clocks / atgt — the
  engine has enough to compose with now.

## Vocal extraction pipeline

Established this session for sourcing topline melodies from a finished
recording. Tools live in the shared venv at
`/home/david/Work/productivity/claude_skill_building/.venv/` (installed
ad-hoc, **not declared** in that repo's `requirements.txt`).

1. **Pull audio from YouTube** (artist's official upload preferred):
   ```bash
   /home/david/Work/productivity/claude_skill_building/.venv/bin/python3 \
     -m yt_dlp -x --audio-format mp3 -o "/tmp/audio/%(title)s.%(ext)s" \
     "<youtube-url>"
   ```
2. **Separate into stems with Demucs** (HT-Demucs default, ~2 min CPU):
   ```bash
   /home/david/Work/productivity/claude_skill_building/.venv/bin/python3 \
     -m demucs --two-stems=vocals -o /tmp/demucs "/tmp/audio/<file>.mp3"
   ```
   Output: `/tmp/demucs/htdemucs/<title>/vocals.wav` + `no_vocals.wav`.
3. **Transcribe** the vocal stem — by ear, or with `basic-pitch` (Spotify
   open-source, not yet installed).

For atgt the stem is at
`/tmp/demucs/htdemucs/Jeremy Zucker, Chelsea Cutler - and the government too! (Official Lyric Video)/vocals.wav`
(may need re-running if /tmp is wiped). Quality is excellent on this track —
near-acapella, easy to hum along with for transcription.

**Caveats**:
- `python3 -m demucs` (and `-m yt_dlp`), not the bare `demucs` /
  `yt-dlp` scripts — the wrapper shebangs in this venv are broken.
- Demucs needs `torchcodec` to actually save WAVs on the current
  torchaudio version; install it alongside demucs.
- Don't add demucs/yt-dlp/torchcodec to skill_building's requirements.txt
  — they're heavy ML deps and not part of any tool there.

**Feedback-loop note**: Gemini audio analysis (`tools/video` with
`audio/wav`) is useful for sound-design diagnosis (thin tone, no space,
missing attack) but unreliable for rhythm / meter / key claims —
treat those as hypotheses (see `.claude/memories/feedback_ai_music_theory.md`).
