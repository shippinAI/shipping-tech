---
name: canon
description: >-
  Refuses a blind import or git pull of foreign code. Use before copying an
  upstream diff, a vendored crate, or a reference server into this repo.
---

# Canon

This folder is the intake context. `schema/intake.schema.json` is the spec. `data/*.json` are the decisions. `decision` is `write-here` or `reject`.

Foreign code is a source. It is not a commit.

1. Read the source and name the behavior it provides.
2. Name the owner. That is the crate or the `.ai` module that already owns the behavior. A new concern is a new module folder with `module.json`, `SKILL.md`, `schema/`, and `data/`.
3. Write the spec: the type or schema, and a proof of the whole behavior.
4. Align it with this repo before writing. Rocks is the one-node store. Shared state is FoundationDB, one S3 bucket, and NATS or Redis. Enterprise stays on the default build and fail-closed. The license stays `deciding` until the maintainer chooses. Engine scale stays ahead of a webmail, an admin UI, a Go control plane, or a second mail server.
5. Add `data/<id>.json` with `decision` `write-here` when we will code it, or `reject` when it stays outside.
6. Implement only the `write-here` behavior, in the named owner. Do not paste the foreign diff in.
