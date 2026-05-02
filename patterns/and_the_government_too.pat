# Jeremy Zucker - "and the government too!" (brent iii, 2024)
# Key: C major (verse opens on Am, giving the minor color)
# Tempo: 105 BPM, 4/4
# Form: V1 - C - V2 - C - Bridge - C
#
# Original is fingerpicked acoustic guitar + soft vocals + minimal production.
# v1 cover is a synth pad arrangement: triangle pad on the chord changes,
# sine bass on the roots, and very light percussion. Expect the harmonic
# motion and form to read clearly; the guitar pluck character is the
# obvious gap (need filter + filter envelope to fix that).

bpm: 105
steps_per_beat: 2
reverb: 0.18

# Pad - moved from triangle to saw + lowpass to get a pluck-pad character
# closer to fingerpicked acoustic. The filter envelope opens fast then closes
# back down, so each chord change has a percussive transient instead of the
# static wash we had in v1.
pad.wave: saw
pad.attack: 15ms
pad.decay: 0.4
pad.sustain: 0.45
pad.release: 350ms
pad.octave: 4
pad.gain: 0.6

pad.cutoff: 350Hz
pad.resonance: 0.25
pad.filter_env: 2.5
pad.filter_attack: 8ms
pad.filter_decay: 0.35
pad.filter_sustain: 0.15
pad.filter_release: 250ms

# Bass - sine roots, gentle pluck
bass.wave: sine
bass.attack: 8ms
bass.decay: 0.25
bass.sustain: 0.5
bass.release: 120ms
bass.gain: 1.5
bass.gate: 0.7
bass.sub: 0.15

# Soft percussion
kick.gain: 0.7
snare.gain: 0.5
hihat.gain: 0.15

# Each section: 64 steps = 8 bars at 8th-note grid (8 steps/bar)
# Chord progression sits one chord per bar.

[verse]
steps: 64
# Am | Em | F | C | F | C | G | G
kick:  x-------x-------x-------x-------x-------x-------x-------x-------
hihat: --o---o---o---o---o---o---o---o---o---o---o---o---o---o---o---o-
pad:   Am . . . . . . . Em . . . . . . . F  . . . . . . . C  . . . . . . . F  . . . . . . . C  . . . . . . . G  . . . . . . . G  . . . . . . .
bass:  A1 . . . . . . . E2 . . . . . . . F1 . . . . . . . C2 . . . . . . . F1 . . . . . . . C2 . . . . . . . G1 . . . . . . . G1 . . . . . . .

[chorus]
steps: 64
# F | C | F | C | F | Am | G | F
kick:  x---x---x---x---x---x---x---x---x---x---x---x---x---x---x---x---
snare: --x---x---x---x---x---x---x---x---x---x---x---x---x---x---x---x-
hihat: xoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxo
pad:   F  . . . . . . . C  . . . . . . . F  . . . . . . . C  . . . . . . . F  . . . . . . . Am . . . . . . . G  . . . . . . . F  . . . . . . .
bass:  F1 . . . . . . . C2 . . . . . . . F1 . . . . . . . C2 . . . . . . . F1 . . . . . . . A1 . . . . . . . G1 . . . . . . . F1 . . . . . . .

[bridge]
steps: 32
# 4 bars: Am | G | F | Am
kick:  x-------x-------x-------x-------
hihat: --o---o---o---o---o---o---o---o-
pad:   Am . . . . . . . G  . . . . . . . F  . . . . . . . Am . . . . . . .
bass:  A1 . . . . . . . G1 . . . . . . . F1 . . . . . . . A1 . . . . . . .

song: verse chorus verse chorus bridge chorus
