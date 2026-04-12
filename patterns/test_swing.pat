# Test: swing / shuffle (5h)
# Compare against cm_chords.pat — same notes, but with 25% swing.
# 0.25 is pretty heavy — every other 16th-note is pushed noticeably late.
# Listen for: the hihat should go from mechanical "tick-tick-tick-tick"
# to a loping "tick..tick-tick..tick" feel. The whole groove should feel
# looser and more human. Compare directly with cm_chords.pat to hear
# the difference — play one, then the other.

bpm: 96
swing: 0.25
steps: 16

kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-

bass.wave: sine
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .

pad.wave: triangle
pad.octave: 4
pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .

lead.wave: square
lead:    G4  .  Eb4 . C5  .  G4  .  Bb4 .  G4  .  F4  .  D4  .
