---
name: plans
description: >-
  Writes an implementation plan from files read in this session and from web
  sources when an external contract is involved, records concerns, then
  implements only after a whole-tree proof is named. Use before editing
  product code or another .ai module.
---

# Plans

This folder is the plans context. A plan is `data/<id>.json` and must match `schema/plan.schema.json`.

## Before any edit

1. Read every file the change will touch. Put those paths in `sources` with `"kind": "code"`.
2. If the change depends on an external schema or API, add `"kind": "web"` sources from a search. Do not invent the contract.
3. Fill `concerns` with what can make the change false.
4. `verification` needs at least two checks. One must fail if the whole result is wrong, not if a single fixture passes.
5. `boundaries` names what stays as it is.

## After the edit

Run the commands. Put the observed output in each `verification[].result` before setting `status` to `implemented`.
