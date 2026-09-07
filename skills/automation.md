# Automation & Scripting

Use this when the user wants to automate a repetitive task.

## Procedure

1. Understand the trigger (manual, schedule, event) and the exact desired outcome.
2. Prefer the smallest reliable solution: a shell one-liner or a Rhai plugin over a new program.
3. Make it idempotent — safe to run twice.

## Building blocks

- Use `shell` for system commands (requires user approval).
- Use `download_file` / `http_get` / `http_post` for web automation.
- Write reusable `.rhai` scripts into `plugins/` for repeated logic.

## Safety

- Never automate destructive actions (delete, overwrite, send) without explicit confirmation each run.
- Log what the automation did so the user can audit.
- Dry-run first when possible; show what *would* happen before doing it.
