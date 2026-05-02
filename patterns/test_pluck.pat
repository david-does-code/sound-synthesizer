# Karplus-Strong pluck test. Bare strum across an open-position chord
# voicing, no other instrumentation. Use this to dial in pluck_decay /
# pluck_brightness by ear.

bpm: 80
steps: 16

pad.model: pluck
pad.pluck_decay: 0.998
pad.pluck_brightness: 0.5
pad.octave: 3
pad.attack: 5ms
pad.release: 800ms
pad.gain: 1.0

pad: Am . . . Em . . . F . . . C . . .
