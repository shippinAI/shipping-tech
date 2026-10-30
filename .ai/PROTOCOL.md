# Session protocol

Each context is one folder under `.ai/modules/<id>/`. The folder holds `module.json`, `SKILL.md`, and, when it stores anything, `schema/` and `data/`. The checker discovers modules by reading those folders. Adding a context means adding a folder. Removing the folder removes the context. Nothing else is registered.

The socket contract is `.ai/machine/module.schema.json`.

| Module | What it owns |
| --- | --- |
| `chats` | Raw transcript lines, messages, and tool calls. Filed by `.ai/machine/copy_chats.py`. |
| `journal` | Wardenclyffe `journal_entry` self-reports. Not a copy of the chat. |
| `plans` | Implementation plans, including COSTAR, sources, concerns, and proof. |
| `roadmap` | Engine phases and the proof that finishes each one. |
| `costar` | The six-field reading. `schema/reading.schema.json` and `data/reading.json`. |

`python .ai/machine/organize.py --check` walks every plugged-in module and rejects a record that does not match that module's schema.
