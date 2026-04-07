# A short cinematic song in C minor with real verse/chorus structure.
#
# Song chain: intro → verse (x2) → chorus → verse → chorus (x2) → outro
#   - intro:   drums fade in, no melody
#   - verse:   drums + bass walking the C minor 7 arpeggio
#   - chorus:  same drums + bass + the lead riff layered on top
#   - outro:   bass alone, drums die out
#
# Notice that `bass` and `lead` are *the same voice slot* across all sections —
# the verse's bass note carries naturally into the chorus, and the lead drops
# out cleanly during the outro because that section just doesn't mention it.

bpm: 100

bass.wave: sine
lead.wave: square

song: intro verse x2 chorus verse chorus x2 outro

[intro]
steps: 16
hihat: ----x-x-x-x-x-x-
kick:  ----x---x---x---

[verse]
steps: 16
kick:  x---x---x---x---
snare: ----x-------x---
hihat: x-x-x-x-x-x-x-x-
bass:  C2 . . . Eb2 . . . G2 . . . F2 . . .

[chorus]
steps: 16
kick:  x---x---x---x---
snare: ----x-------x---
hihat: x-x-x-x-x-x-x-x-
bass:  C2 . . . Eb2 . . . G2 . . . Bb2 . . .
lead:  Eb4 F4 G4 Bb4 C5 . Bb4 G4 F4 . Eb4 . D4 . C4 .

[outro]
steps: 16
kick:  x-------x-------
hihat: x-x-x-----------
bass:  C2 . . . . . . . . . . . . . . .
