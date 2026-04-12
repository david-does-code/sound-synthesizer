# Hans Zimmer — "Time" (Inception)
# Key: A minor | BPM: 60 | Progression: Am - Em - G - D
#
# The soul of this piece is the slow build from nothing to full intensity.
# Each section adds a layer: pad alone → add bass → add melody → add drums.
# The pad should swell in (slow attack) and the bass should pulse cleanly.

bpm: 60
steps: 16

# Pad: slow swell, sustained, warm
pad.wave: triangle
pad.octave: 3
pad.attack: 400ms
pad.decay: 0.3
pad.sustain: 0.7
pad.release: 500ms
pad.gain: 1.2

# Bass: clean and round, short gate so it pulses
bass.wave: sine
bass.attack: 10ms
bass.release: 80ms
bass.gate: 0.6
bass.gain: 1.0

# Melody: the iconic piano motif — square gives it a haunting, simple quality
melody.wave: square
melody.attack: 30ms
melody.decay: 0.2
melody.sustain: 0.4
melody.release: 300ms
melody.gain: 0.6

# Drums: minimal, just marking the pulse
kick.gain: 0.8
hihat.gain: 0.3

# ─── Section 1: Just the pad, empty and ethereal ─────────────────
[intro]
pad:     Am  .  .  .  .  .  .  .  Em  .  .  .  .  .  .  .

# ─── Section 2: Bass enters, grounding the harmony ───────────────
[build1]
pad:     Am  .  .  .  .  .  .  .  Em  .  .  .  .  .  .  .
bass:    A2  .  .  .  A2  .  .  .  E2  .  .  .  E2  .  .  .

# ─── Section 3: Second half of progression, bass + pad ───────────
[build2]
pad:     G   .  .  .  .  .  .  .  D   .  .  .  .  .  .  .
bass:    G2  .  .  .  G2  .  .  .  D2  .  .  .  D2  .  .  .

# ─── Section 4: Melody enters — the famous rising motif ──────────
[theme1]
pad:     Am  .  .  .  .  .  .  .  Em  .  .  .  .  .  .  .
bass:    A2  .  .  .  A2  .  .  .  E2  .  .  .  E2  .  .  .
melody:  A4  .  .  .  B4  .  .  .  C5  .  .  .  B4  .  .  .

[theme2]
pad:     G   .  .  .  .  .  .  .  D   .  .  .  .  .  .  .
bass:    G2  .  .  .  G2  .  .  .  D2  .  .  .  D2  .  .  .
melody:  A4  .  .  .  G4  .  .  .  A4  .  .  .  B4  .  .  .

# ─── Section 5: Climax — drums arrive, melody reaches higher ─────
[climax1]
kick:    X---x-------x---
hihat:   --x---x---x---x-
pad:     Am  .  .  .  .  .  .  .  Em  .  .  .  .  .  .  .
bass:    A2  .  .  .  A2  .  .  .  E2  .  .  .  E2  .  .  .
melody:  E5! .  .  .  D5  .  C5  .  B4  .  .  .  A4  .  .  .

[climax2]
kick:    X---x-------x---
hihat:   --x---x---x---x-
pad:     G   .  .  .  .  .  .  .  D   .  .  .  .  .  .  .
bass:    G2  .  .  .  G2  .  .  .  D2  .  .  .  D2  .  .  .
melody:  A4  .  B4  .  C5  .  D5  .  E5! .  .  .  D5  .  .  .

# ─── Section 6: Outro — just pad fading ──────────────────────────
[outro]
pad:     Am  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

# The song: sparse → layered → full → sparse
song: intro x4 build1 build2 x2 theme1 theme2 x2 climax1 climax2 x3 outro x2
