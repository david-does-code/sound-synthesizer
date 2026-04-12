# Test: velocity / dynamics (5f)
# Compare against cm_chords.pat — same pattern, but with accents and ghosts.
#   - kick: accented on beats 1 and 3 (X), normal on 2 and 4 (x)
#   - hihat: alternating ghost (o) and normal (x) — should create a soft/loud pulse
#   - snare: ghost note added on the "and" of beat 4 (the o before the last rest)
#   - lead: accent on the peak note C5, ghost on the passing tones
# Listen for: dynamic variation within each track, not just on/off.

bpm: 96
steps: 16

kick:    X---x---X---x---
snare:   ----X-----o-X---
hihat:   xoxoxoxoxoxoxoxo

bass.wave: sine
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .

pad.wave: triangle
pad.octave: 4
pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .

lead.wave: square
lead:    G4? .  Eb4 . C5! .  G4? .  Bb4 .  G4  .  F4? .  D4  .
