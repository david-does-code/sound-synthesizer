# C minor expressive groove — demos all Phase 5d-5i features.
# Per-track ADSR, gain, gate, velocity dynamics, swing, and per-section BPM.
bpm: 96
swing: 0.12

# Per-track instruments and envelopes
kick.gain: 1.3
snare.gain: 0.9
hihat.gain: 0.5

bass.wave: sine
bass.attack: 5ms
bass.decay: 0.08
bass.sustain: 0.6
bass.release: 50ms
bass.gain: 1.1
bass.gate: 0.4

pad.wave: triangle
pad.attack: 300ms
pad.decay: 0.2
pad.sustain: 0.8
pad.release: 400ms
pad.octave: 4

lead.wave: square
lead.attack: 10ms
lead.decay: 0.15
lead.sustain: 0.5
lead.release: 100ms
lead.gate: 0.6

steps: 16

[verse]
kick:    X---x---X---x---
snare:   ----X-------X-o-
hihat:   xoxxoxoxoxxoxoxo

bass:    C2  .  .  .  Ab1 .  .  .  Eb2 .  .  .  Bb1 .  .  .

pad:     Cm  .  .  .  Ab  .  .  .  Eb  .  .  .  Bb  .  .  .

lead:    G4  .  Eb4 . C5! .  G4? .  Bb4 .  G4  .  F4  .  D4? .

[chorus]
bpm: 100
kick:    X--xX--xX--xX--x
snare:   ----X-------X---
hihat:   xxxxxxxxxxxxxxxx

bass:    C2  .  .  .  Eb2 .  .  .  Ab1 .  .  .  Bb1 .  .  .

pad:     Cm  .  .  .  Eb  .  .  .  Ab  .  .  .  Bb  .  .  .

lead:    C5! .  Bb4 . G4  .  Eb4 .  F4! .  Eb4 .  D4  .  C4  .

song: verse x2 chorus x2
