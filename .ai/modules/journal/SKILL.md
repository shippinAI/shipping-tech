---
name: journal
description: >-
  Appends Wardenclyffe journal_entry records through .ai/machine/organize.py.
  Use when recording a decision, result, correction, or hypothesis. Do not use
  this to copy a chat.
---

# Journal

This folder is the journal context. The entry types are fixed by `schema/journal-entry.schema.json`.

```text
python .ai/machine/organize.py --journal --session SESSION --type ENTRY_TYPE --content "TEXT"
```

`ENTRY_TYPE` is `hypothesis_formed`, `command_run`, `result_observed`, `hypothesis_invalidated`, `hypothesis_confirmed`, `gotcha_hit`, `decision_made`, `blocked_external`, `user_correction`, `subagent_spawn`, or `subagent_return`.
