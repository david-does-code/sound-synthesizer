# Vocal harmony stack + string pad demo.
# Goal: evaluate whether we can mimic the Lumï / Zucker-Cutler texture
# (lead vocal + parallel harmony) with a cinematic string pad underneath.
#
# Progression: Am - F - C - G (vi - IV - I - V in C major), ~75 BPM ballad.
# Three vocal voices: lead melody, harmony a diatonic third above,
# and an octave-down doubling. Strings: slow-attack saw pad on the chord.

bpm: 75
steps_per_beat: 2
reverb: 0.35

# --- Vocal stack ---
# Sine waves for clean pitch and a "voice-like" pure tone.
# Slight attack so it doesn't pop; long release so notes overlap softly.

lead.wave: sine
lead.octave: 4
lead.attack: 30ms
lead.decay: 0.3
lead.sustain: 0.75
lead.release: 350ms
lead.gain: 1.0
lead.gate: 0.95
lead.vibrato_rate: 5.2
lead.vibrato_depth: 0.12

harm3.wave: sine
harm3.octave: 4
harm3.attack: 45ms
harm3.decay: 0.3
harm3.sustain: 0.7
harm3.release: 350ms
harm3.gain: 0.55
harm3.gate: 0.95
# Slightly different LFO rate so the two voices drift in and out of phase
# instead of moving in lockstep — that's most of what makes a stack feel human.
harm3.vibrato_rate: 4.6
harm3.vibrato_depth: 0.10

harm8.wave: sine
harm8.octave: 3
harm8.attack: 50ms
harm8.decay: 0.3
harm8.sustain: 0.7
harm8.release: 350ms
harm8.gain: 0.45
harm8.gate: 0.95
harm8.vibrato_rate: 5.7
harm8.vibrato_depth: 0.08

# --- String pad ---
# Saw with very slow attack + long release + low cutoff = cinematic strings.
# Filter envelope opens slightly per note for subtle movement.
strings.wave: saw
strings.octave: 3
strings.attack: 250ms
strings.decay: 0.5
strings.sustain: 0.8
strings.release: 800ms
strings.gain: 0.35
strings.gate: 0.95
strings.cutoff: 5000
strings.resonance: 0.1
strings.filter_env: 0.4
strings.filter_attack: 400ms
strings.filter_release: 600ms
# Slow, shallow vibrato on the strings — that's the missing "movement" that
# made the pad sound synthy. Real string sections have a wider, slower wobble
# than singers (~3-4 Hz vs ~5 Hz on vocals).
strings.vibrato_rate: 3.5
strings.vibrato_depth: 0.08

# Unison detune: 3 oscillators per voice, ±5 cents around center. This is the
# "many slightly out-of-tune players" trick that gives real string sections
# their thickness — single oscillator = one violin, three detuned = a section.
strings.unison: 3
strings.detune: 18

# --- Bass ---
bass.wave: sine
bass.attack: 12ms
bass.decay: 0.25
bass.sustain: 0.6
bass.release: 200ms
bass.gain: 1.3
bass.gate: 0.85
bass.sub: 0.1

[verse]
steps: 32
# 4 bars, 8 steps/bar.
# Chord:    Am               F                C                G
#           |               |               |               |
strings: Am . . . . . . . F  . . . . . . . C  . . . . . . . G  . . . . . . .
bass:    A2 . . . . . . . F2 . . . . . . . C2 . . . . . . . G2 . . . . . . .

# Lead melody - now strictly chord-tone landings on every step.
# Am(A C E) | F(F A C) | C(C E G) | G(G B D)
lead:    E5 . . . C5 . . . F5 . . . A5 . . . E5 . . . G5 . . . D5 . . . B4 . . .

# Harmony = next chord tone ABOVE the lead within the current chord.
# Am: E->A, C->E   F: F->A, A->C   C: E->G, G->C   G: D->G, B->D
harm3:   A5 . . . E5 . . . A5 . . . C6 . . . G5 . . . C6 . . . G5 . . . D5 . . .

# Octave doubling below - mirrors the lead exactly, adds body
harm8:   E4 . . . C4 . . . F4 . . . A4 . . . E4 . . . G4 . . . D4 . . . B3 . . .

song: verse x2
