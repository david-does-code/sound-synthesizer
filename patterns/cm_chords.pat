# A C minor chord progression with drums + bass + chord pad + lead.
#
# The chord progression is the classic i - VI - III - VII (Cm - Ab - Eb - Bb)
# in C minor — a moody, cinematic loop you'll hear in countless film scores
# and indie game soundtracks.
#
# Each track has its own waveform so the parts have distinct character:
#   - bass:  sine     (round, sub-y)
#   - pad:   triangle (soft, mellow)
#   - lead:  square   (chiptuney, cuts through)

bpm: 96
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
