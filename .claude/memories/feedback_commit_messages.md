---
name: Concise commit messages
description: User prefers short, terse git commit messages over multi-paragraph explanations
type: feedback
---

Keep git commit messages concise. Lead with a one-line subject (under 70 chars),
and if a body is needed at all, keep it to a few short sentences. Don't enumerate
every changed file or restate what the diff already shows.

**Why:** User said "we can make the commit message more concise" after I wrote a
20-line commit body for slice 5a/5b that re-explained the implementation in detail.

**How to apply:** Default to subject-only commits unless the change is genuinely
hard to understand from the diff. When a body is warranted, write *why* not *what*,
and stop after 2-3 sentences.
