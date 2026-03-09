# Action Plans

This folder contains active plans and reviews generated during Claude Code sessions.

## How This Works

Plans and reviews are created here as markdown files while work is in progress.

## Completing a Plan

When a plan or review is **fully completed** (all tasks checked off / marked done):

1. Add a completion timestamp to the top of the file:
   ```
   **Completed:** YYYY-MM-DD HH:MM
   ```
2. Move the file into the `old/` subfolder inside this directory.
3. Do this automatically at the end of every session where a plan is wrapped up — no need to ask.

## Rules

- **Active plans stay here** — only move to `old/` when fully done
- **Never delete plans** — always archive to `old/`
- **Timestamp format:** `YYYY-MM-DD HH:MM` (local time)
- **Old folder path:** `action-plans/old/<filename>.md`

## Structure

```
action-plans/
├── README.md          ← this file
├── some-active-plan.md
└── old/
    └── completed-plan-2026-03-08.md
```
