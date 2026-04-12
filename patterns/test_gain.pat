# Test: per-track gain/mixing (5e)
# Compare against cm_chords.pat — same notes, but:
#   - kick is boosted to 1.5 (should punch noticeably harder)
#   - hihat is pulled way down to 0.2 (should be barely audible)
#   - pad is boosted to 1.8 (should dominate the mix)
#   - lead is pulled down to 0.3 (should sit behind the pad)
# If gain is working, the mix balance will be dramatically different.

bpm: 96
steps: 16

kick.gain: 1.5
hihat.gain: 0.2

kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-

bass.wave: sine
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .

pad.wave: triangle
pad.octave: 4
pad.gain: 1.8
pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .

lead.wave: square
lead.gain: 0.3
lead:    G4  .  Eb4 . C5  .  G4  .  Bb4 .  G4  .  F4  .  D4  .
