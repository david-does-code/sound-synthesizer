# Jon Bellion - "Guillotine" ft. Travis Mendes (The Human Condition, 2016)
# Key: E minor. Tempo: ~99 BPM, 4/4.
# Chords/tempo/key transcribed from the official audio via librosa
# beat-synchronous chroma + diatonic triad template matching (see
# /tmp/jb_audio/analyze2.py). All chords are E-minor diatonic.
#
# Guillotine is an almost-a-cappella production: stacked harmony vocals,
# finger snaps on 2 & 4, a deep vocal "bass", almost no drum kit. So the
# faithful backing track IS the harmony stack. We fake the choir with a
# unison-detuned saw pad + light vibrato + heavy reverb (no formants, so
# it reads as "warm pad/strings" rather than literal voices), put the
# roots on a sub-heavy sine bass, and use a quiet snare as the snap.
#
# Form (from the bar analysis):
#   verse:  C  - Em - Em - C   (the iconic two-chord vamp, Am passing)
#   build:  G  - Em - D  - Bm  (then a G/Bm oscillation live)
#   chorus: C  - G  - D  - Bm  (the big E-minor-family lift)
#   outro:  Bm - Am - Em - D   (the descending breakdown)
#
# The lead vocal topline is NOT yet transcribed - that's the next pass
# (run basic-pitch on the demucs vocal stem, then fill the `lead` row).
# The harmony + snap groove already reads as Guillotine on its own.

bpm: 99
steps_per_beat: 2
reverb: 0.36

# --- Vocal-stack pad: unison-detuned saw = faux choir/string section.
# Open filter so the detune beating (the "many voices" shimmer) survives;
# slow-ish attack for a vocal swell; long release so chords bleed together
# like overlapping harmony takes.
pad.wave: saw
pad.octave: 3
pad.unison: 3
pad.detune: 16
pad.vibrato_rate: 5hz
pad.vibrato_depth: 0.09
pad.cutoff: 3.6kHz
pad.resonance: 0.12
pad.attack: 45ms
pad.decay: 0.3
pad.sustain: 0.8
pad.release: 480ms
pad.gain: 0.85

# --- Deep vocal bass: sine + sub octave on the chord roots.
bass.wave: sine
bass.octave: 2
bass.attack: 8ms
bass.decay: 0.25
bass.sustain: 0.55
bass.release: 130ms
bass.sub: 0.22
bass.gain: 1.5
bass.gate: 0.85

# --- Snap = quiet snare on beats 2 & 4. Soft kick heartbeat on 1, very
# light hihat for air. Keep the kit small; Guillotine lives on the snaps.
snare.gain: 0.4
kick.gain: 0.45
hihat.gain: 0.1

# --- Lead topline (next pass). Sine = clean pitch reference for transcribing.
lead.wave: sine
lead.octave: 4
lead.attack: 15ms
lead.decay: 0.2
lead.sustain: 0.7
lead.release: 180ms
lead.gain: 0.9
lead.gate: 0.85

# 32 steps = 4 bars at an 8th-note grid (8 steps/bar). One chord per bar.
# Snare snaps land on step 2 and 6 of each bar (beats 2 & 4).

[verse]
steps: 32
# C | Em | Em | C
kick:  x-------x-------x-------x-------
snare: --x---x---x---x---x---x---x---x-
hihat: --o---o---o---o---o---o---o---o-
pad:   C  . . . . . . . Em . . . . . . . Em . . . . . . . C  . . . . . . .
bass:  C2 . . . C2 . . . E2 . . . E2 . . . E2 . . . E2 . . . C2 . . . C2 . . .
lead:  - . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .

[build]
steps: 32
# G | Em | D | Bm
kick:  x-------x-------x-------x-------
snare: --x---x---x---x---x---x---x---x-
hihat: --o---o---o---o---o---o---o---o-
pad:   G  . . . . . . . Em . . . . . . . D  . . . . . . . Bm . . . . . . .
bass:  G1 . . . G1 . . . E2 . . . E2 . . . D2 . . . D2 . . . B1 . . . B1 . . .
lead:  - . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .

[chorus]
steps: 32
# C | G | D | Bm
kick:  x-------x-------x-------x-------
snare: --X---X---X---X---X---X---X---X-
hihat: xoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxo
pad:   C  . . . . . . . G  . . . . . . . D  . . . . . . . Bm . . . . . . .
bass:  C2 . . . C2 . . . G1 . . . G1 . . . D2 . . . D2 . . . B1 . . . B1 . . .
lead:  - . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .

[outro]
steps: 32
# Bm | Am | Em | D
kick:  x-------x-------x-------x-------
snare: --x---x---x---x---x---x---x---x-
hihat: --o---o---o---o---o---o---o---o-
pad:   Bm . . . . . . . Am . . . . . . . Em . . . . . . . D  . . . . . . .
bass:  B1 . . . B1 . . . A1 . . . A1 . . . E2 . . . E2 . . . D2 . . . D2 . . .
lead:  - . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .

song: verse verse build chorus chorus build outro
