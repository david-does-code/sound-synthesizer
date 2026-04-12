# Test: note gate length (5g)
# Compare against cm_chords.pat — same notes, but:
#   - bass has gate 0.3 — very short, punchy, funky (staccato)
#   - lead has gate 0.5 — half-step, bouncy
#   - pad has no gate — stays legato (sustained) as before
# Listen for: bass should have clear silence between notes, lead should bounce,
# pad should still wash smoothly.

bpm: 96
steps: 16

kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-

bass.wave: sine
bass.gate: 0.3
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .

pad.wave: triangle
pad.octave: 4
pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .

lead.wave: square
lead.gate: 0.5
lead:    G4  .  Eb4 . C5  .  G4  .  Bb4 .  G4  .  F4  .  D4  .
