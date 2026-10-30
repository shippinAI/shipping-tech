---
name: chats
description: >-
  Files Cursor transcripts into the chats module with .ai/machine/copy_chats.py.
  Use when saving, resuming, or checking chat history.
---

# Chats

This folder is the chats context. Schema, sessions, and the byte index stay here.

Run `python .ai/machine/copy_chats.py --sync`. Unchanged transcripts add no lines. The Cursor hook calls the same program. Do not hand-copy a transcript into this folder.
