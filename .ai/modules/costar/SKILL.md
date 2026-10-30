---
name: costar
description: >-
  Infers a full COSTAR reading (context, objective, style, tone, audience,
  response) from a user prompt and asks one focused multiple-choice question
  when a missing field would change the work. Use on every user request before
  planning or editing.
---

# COSTAR

This folder is the COSTAR context. `schema/reading.schema.json` is the six fields. `data/reading.json` is the current reading.

COSTAR is Context, Objective, Style, Tone, Audience, Response. Style is the form. Tone is the register. Audience is the reader.

1. Write the six fields from the user's words, the open files, and the repo rules. Mark a field inferred only when the user did not say it.
2. If a missing field would change which files you touch or which proof you run, ask one question with two to four options. Put the repo-backed option first.
3. If the user already chose, or the gap does not change the artifact, record the six fields in the plan and continue.
