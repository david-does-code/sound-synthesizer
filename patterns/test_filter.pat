# Filter sweep test. A sustained sawtooth chord with a slow filter envelope:
# the cutoff opens from ~250 Hz up by 4 octaves over the attack, then settles
# and slowly releases. Resonance pushes a clear peak at the cutoff.

bpm: 60
steps: 8

pad.wave: saw
pad.attack: 5ms
pad.decay: 1.5
pad.sustain: 0.6
pad.release: 600ms
pad.gain: 0.6

pad.cutoff: 250Hz
pad.resonance: 0.7
pad.filter_env: 4.0
pad.filter_attack: 800ms
pad.filter_decay: 1.2
pad.filter_sustain: 0.3
pad.filter_release: 400ms

pad: Cm . . . . . . .
