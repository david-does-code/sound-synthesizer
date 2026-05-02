# Coldplay — "Clocks"
# Key: Eb major | BPM: 130 | Progression: Eb - Bbm - Fm
#
# The intro is JUST the piano riff, alone and exposed — that's the
# iconic sound. Drums and bass enter later. The piano arpeggio
# provides all the rhythmic drive, so drums stay minimal (no hi-hat
# competing with the riff). Kick on 1, snare on 3 for a half-time
# feel that lets the piano breathe.

bpm: 130
steps: 16
# Master reverb send. ~0.15 = subtle space; 0.3+ = obvious room.
reverb: 0.15

# Piano riff: triangle is the softest waveform we have — less buzzy
# than square. The ADSR mimics a piano hammer: sharp attack, fast
# decay to a low sustain, so each note "plinks" then fades rather
# than droning at full volume. No gate needed — the decay does the work.
piano.wave: triangle
piano.attack: 5ms
piano.decay: 0.25
piano.sustain: 0.15
piano.release: 200ms
piano.gain: 1.2
# Hammer click: tiny pitch bend on attack (semitones). Real piano hammers
# bend ~1-2 semitones; bigger numbers sound like a chirp / zap rather than
# a thwack.
piano.click: 3

# Pad: sustained chords underneath for warmth, quiet in the mix
pad.wave: triangle
pad.octave: 3
pad.attack: 200ms
pad.sustain: 0.6
pad.release: 300ms
pad.gain: 0.5

# Bass: root notes, round and present
bass.wave: sine
bass.attack: 10ms
bass.release: 60ms
bass.gate: 0.5
bass.gain: 1.1

# Drums: minimal — the piano IS the rhythm
kick.gain: 1.0
snare.gain: 0.8

# ─── INTRO: piano alone ──────────────────────────────────────────
# The riff exposed, no accompaniment — this is the sound everyone
# recognizes instantly.

[intro_eb]
piano:   Bb4 .  G4  .  Eb4 .  Bb4 .  G4  .  Eb4 .  Bb4 .  G4  .

[intro_bbm]
piano:   Bb4 .  F4  .  Db4 .  Bb4 .  F4  .  Db4 .  Bb4 .  F4  .

[intro_fm]
piano:   Ab4 .  F4  .  C4  .  Ab4 .  F4  .  C4  .  Ab4 .  F4  .

# ─── FULL: piano + bass + pad + drums ────────────────────────────

[eb]
kick:    x---------------
snare:   --------x-------
piano:   Bb4 .  G4  .  Eb4 .  Bb4 .  G4  .  Eb4 .  Bb4 .  G4  .
bass:    Eb2 .  .  .  .  .  .  .  Eb2 .  .  .  .  .  .  .
pad:     Eb  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

[bbm]
kick:    x---------------
snare:   --------x-------
piano:   Bb4 .  F4  .  Db4 .  Bb4 .  F4  .  Db4 .  Bb4 .  F4  .
bass:    Bb1 .  .  .  .  .  .  .  Bb1 .  .  .  .  .  .  .
pad:     Bbm .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

[fm]
kick:    x---------------
snare:   --------x-------
piano:   Ab4 .  F4  .  C4  .  Ab4 .  F4  .  C4  .  Ab4 .  F4  .
bass:    F2  .  .  .  .  .  .  .  F2  .  .  .  .  .  .  .
pad:     Fm  .  .  .  .  .  .  .  .  .  .  .  .  .  .  .

# Intro: 2 full cycles of piano alone, then full arrangement loops.
song: intro_eb x2 intro_bbm intro_fm intro_eb x2 intro_bbm intro_fm eb x2 bbm fm
