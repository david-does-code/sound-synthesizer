---
name: AI music-theory claims need verification
description: Gemini gave a confident, wrong music-theory claim that wasted an iteration; treat AI music feedback as hypothesis, not fact
type: feedback
---

When Gemini (or any LLM) makes a music-theory claim about a song's meter,
key, chord progression, or rhythmic structure, treat it as a hypothesis
to verify against the actual recording, not as a ground-truth diagnosis.

**Why:** Gemini Pro confidently asserted Clocks was in eighth-note triplets
and called it "non-negotiable" to fix the meter. We rebuilt the pattern on
a 12-step triplet grid and it sounded clearly wrong ("doubled in speed").
Clocks is actually 8th notes in 4/4 — the polyrhythmic feel comes from a
3-note cell creating a 3-against-2 hemiola over the 4-beat bar, not from
triplet subdivision. Real Clocks: 130 BPM, 8th-note arpeggios in 4/4.

**How to apply:** Before changing meter / time signature / tempo / key based
on an LLM's listening analysis, double-check against a reference recording
(or sheet music search). Sound-design feedback (timbre is thin, mix is dry,
attack lacks transient) tends to be more reliable than rhythmic / theoretical
diagnosis. The audio-feedback loop is best for "does this sound like a
real instrument" questions, not "is this in the right meter" ones.
