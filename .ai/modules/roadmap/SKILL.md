---
name: roadmap
description: >-
  Reads and updates the engine roadmap in this module. Use when resuming work
  or when a phase status changes.
---

# Roadmap

This folder is the roadmap context. `data/roadmap.json` is the only roadmap. A phase is `done` only when its `exit_proof` has been observed. Build stays `in_progress` until `cargo build -p shippin-tech` exits 0.
