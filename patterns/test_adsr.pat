# Test: per-track ADSR (5d)
# Compare against cm_chords.pat — same notes, but:
#   - pad has slow 500ms attack (should swell in instead of starting immediately)
#   - bass has very short 20ms release (should cut off cleanly between notes)
#   - lead has long 600ms release (notes should bleed/tail into each other)
# If ADSR is working, each instrument will feel distinctly different.

bpm: 96
steps: 16

kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-

bass.wave: sine
bass.attack: 5ms
bass.release: 20ms
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .

pad.wave: triangle
pad.octave: 4
pad.attack: 500ms
pad.decay: 0.3
pad.sustain: 0.6
pad.release: 400ms
pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .

lead.wave: square
lead.attack: 10ms
lead.release: 600ms
lead:    G4  .  Eb4 . C5  .  G4  .  Bb4 .  G4  .  F4  .  D4  .
