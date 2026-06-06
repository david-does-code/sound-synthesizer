# Pattern syntax v2 + sampler (record / playback / mix)

status: proposed

Two related goals, sequenced **syntax-first** (decided 2026-06-06):

1. **Simpler `.pat` syntax** optimized for Claude to author reliably (and for
   the future sampler/recorder tooling to emit).
2. **Samples**: record audio from input, store it, play it back as an
   instrument (both one-shot and pitched), and mix it alongside synth tracks.

Sequencing locked in by the user:
1. Syntax v2 (with a sample-track slot baked in)
2. Sample playback (load WAV -> voice)
3. Recording (`--record` / armed mode)

Syntax design was delegated to Claude with one constraint: **optimize for
reliable LLM authoring**. The sampler emits this syntax too, so it only has to
be clean for a program to generate.

---

## Phase 1 - Pattern syntax v2 (do this first)

Four additions to the `.pat` format. **All opt-in and backward-compatible** —
every one of the ~21 existing patterns must still parse byte-identically. The
backward-compat hook is that each new construct's absence = today's behavior.

### 1. Note durations (`:q`) — removes dot-padding and step-counting
A note/chord cell can declare its length instead of being padded with `.`:
- `:w` whole, `:h` half, `:q` quarter, `:e` eighth, `:s` sixteenth
- dotted `:q.`, triplet `:qt`
- bare note with no `:dur` = **1 step** (exactly today's behavior — the
  backward-compat hook)
- raw-step escape hatch `:3s` = "hold 3 steps" — always integer-safe, needed
  for odd grids (e.g. `steps_per_beat: 3` where an eighth wouldn't land on an
  integer step). Note-value letters are primary; `:Ns` is the universal
  fallback.
- velocity + duration compose: `C5!:e` = accented eighth. Token order is
  note, then velocity (`!`/`?`), then `:dur`.

Durations apply to **note and chord rows only**. Drum rows stay as the compact
char grid (`X---x---`) — a kick has no duration to declare.

Parser converts note-value -> steps via the section's `steps_per_beat`. If a
note-value doesn't divide evenly into integer steps for the current grid,
that's an error pointing the author at `:Ns`.

### 2. Bar lines (`|`) — turns miscounting into a precise, located error
Optional `|` markers in a row. Parser checks each bar sums to a full bar of
duration and reports *which* bar is wrong (`bar 2 has 3.5 beats, expected 4`)
instead of silently misaligning the whole row. Biggest reliability win for
LLM authoring. Absent `|` = no bar checking (today's behavior).

v1 assumes 4 beats/bar. A `time: 3/4` header is **explicitly out of scope for
v1** but the bar-checking code should be structured so it drops in later
without rework.

### 3. Inline track properties (`{...}`) — removes the dotted-line blocks
`bass {wave:sine gain:1.1 gate:0.4}: ...` replaces the six separate
`bass.attack:` / `bass.gain:` lines. Old dotted lines still work (parser
already collects them globally in pass 1); inline `{...}` is sugar that
collapses each track to a single self-contained line — which also removes the
"wrong dotted-block associated with wrong row" failure mode.

### 4. Sample slot — the hook the sampler/recorder writes into
Same inline-prop mechanism, just new keys:
- `{sample:kick.wav}` = one-shot (play at native pitch)
- `{sample:ahh.wav root:C4}` = pitched (`root` = the recorded pitch, so
  resampling can transpose it across the keyboard)

Parsing the slot can land in Phase 1; the engine support for it is Phase 2.

### Before / after (cm_expressive verse)

Now — ~20 lines of dotted props + dot-padded rows:
```
bass.wave: sine
bass.attack: 5ms
bass.decay: 0.08
bass.sustain: 0.6
bass.release: 50ms
bass.gain: 1.1
bass.gate: 0.4
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .
lead:    G4  .  Eb4 . C5! .  G4? .  Bb4 .  G4  .  F4  .  D4? .
```

v2 — one self-contained line per track, nothing to count:
```
kick  {gain:1.3}: X---x---X---x---
snare {gain:0.9}: ----X-------X-o-
hihat {gain:0.5}: xoxxoxoxoxxoxoxo
bass  {wave:sine attack:5ms decay:0.08 sustain:0.6 release:50ms gain:1.1 gate:0.4}: | C2:q Ab1:q Eb2:q Bb1:q |
pad   {wave:tri octave:4 attack:300ms decay:0.2 sustain:0.8 release:400ms}:          | Cm:q Ab:q Eb:q Bb:q |
lead  {wave:square attack:10ms decay:0.15 sustain:0.5 release:100ms gate:0.6}:        | G4:e Eb4:e C5!:e G4?:e Bb4:e G4:e F4:e D4?:e |
```

### Phase 1 implementation notes
- All changes live in `src/pattern.rs` (parser) — sequencer/engine unaffected
  for everything except the sample slot.
- Keep the two-pass parser; inline `{...}` props feed the same `TrackProps`
  struct that dotted lines already populate.
- Tokenizer for a note/chord cell now splits `note [!|?] [:dur]`.
- **Tests must cover: every existing `patterns/*.pat` still parses identically**
  (snapshot the parsed `Pattern` before/after), plus new unit tests for
  durations, `:Ns`, bar validation errors, inline props, and the sample slot.
- Convert one or two existing patterns to v2 as living examples once the parser
  lands (keep the originals too, to prove both styles parse).

### Open micro-question to confirm before coding Phase 1
User was asked whether the denser v2 look is acceptable (vs the column-aligned
grid where every step lines up) and whether the duration letters / `{}` / `|`
notation is right. **Not yet answered** — confirm at pickup before building.

---

## Phase 2 - Sample playback (after syntax)

Both one-shot and pitched (user picked "both").

- Add `SynthModel::Sample` (mirrors how `SynthModel::Pluck` was added).
- Load referenced WAVs once at startup into `Arc<[f32]>` (shared, read-only;
  no file IO in the audio callback — same realtime-safety rule already
  followed). `hound` (already a dep) decodes WAV.
- A sample voice = a read cursor over the buffer. One-shot: cursor increments
  by 1.0/frame, stops at end. Pitched: cursor increment = `target_freq /
  sample_root_freq`, with linear interpolation between samples.
- Reuse existing per-voice plumbing: gain, filter, ADSR, reverb all apply on
  top, exactly like the oscillator/pluck paths.
- Once a sample track type exists, **mixing is free** — the sequencer already
  mixes arbitrary tracks.
- Samples live in a `samples/` dir, referenced by the Phase-1 `{sample:...}`
  slot. (gitignore generated/recorded WAVs, consistent with current WAV
  handling.)

## Phase 3 - Recording (after playback)

The mirror image of the existing output path.

- `cargo run -- --record out.wav` (and/or an interactive "armed" mode) opens a
  cpal **input** stream (`build_input_stream`) and captures to a preallocated
  buffer, then writes WAV via `hound`.
- The full loop: record -> drop file in `samples/` -> reference it from a
  pattern via `{sample:...}`.

### Must verify live before committing Phase 3 (external-claim rule)
Linux/ALSA **input** via cpal — device selection and input-vs-output
sample-rate mismatch are the usual friction points. Write a ~30-line
standalone probe that records ~2s and writes a WAV **first**, to settle the
device + sample-rate questions before touching production code. Keep the probe
committed (per the persistent-probe convention).

---

## Pickup checklist for next session
1. Confirm the v2 look + notation with the user (the open micro-question above).
2. Build Phase 1 in `src/pattern.rs`, parser-first, with the
   "all existing patterns still parse" test as the regression gate.
3. Then Phase 2 (sample voice), then Phase 3 (recording, probe-first).
