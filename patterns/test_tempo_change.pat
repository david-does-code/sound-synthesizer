# Test: per-section BPM (5i)
# Three sections at different tempos with the same pattern.
# The verse is slow (80 BPM), the chorus speeds up (120 BPM),
# and the outro slows way down (60 BPM).
# Listen for: clear tempo shifts at each section boundary.

bpm: 80
steps: 16

bass.wave: sine
pad.wave: triangle
pad.octave: 4
lead.wave: square

[verse]
kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .
pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .
lead:    G4  .  Eb4 . C5  .  G4  .  Bb4 .  G4  .  F4  .  D4  .

[chorus]
bpm: 120
kick:    x---x---x---x---
snare:   ----x-------x---
hihat:   x-x-x-x-x-x-x-x-
bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .
pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .
lead:    G4  .  Eb4 . C5  .  G4  .  Bb4 .  G4  .  F4  .  D4  .

[outro]
bpm: 60
kick:    x-------x-------
hihat:   x-x-x-----------
bass:    C2  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

song: verse x2 chorus x2 outro
