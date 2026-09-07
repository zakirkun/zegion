# Skills

Skills are **markdown procedures** that teach Zegion reusable ways of working — no
retraining, no code. They live in `skills/` as `.md` files, are auto-loaded at startup,
and hot-reloaded when they change.

At startup Zegion injects a compact catalog of skill names + descriptions into the
system prompt, so the agent knows they exist and can apply the right one for a task.

## Included skills (16)

| skill             | purpose                                             |
| ----------------- | --------------------------------------------------- |
| `research`        | deep, sourced investigation with cross-checking     |
| `writing`         | drafting and editing prose                          |
| `code_review`     | severity-tagged code review                         |
| `data_analysis`   | profiling + answering questions over data           |
| `planning`        | task decomposition into verifiable steps            |
| `learning`        | what to store in long-term memory (and what not to) |
| `automation`      | safe, idempotent task automation                    |
| `translation`     | meaning-first translation (EN/ID)                   |
| `debugging`       | hypothesis-driven root-cause analysis               |
| `meeting_notes`   | summaries, decisions, action items                  |
| `brainstorm`      | divergent-then-convergent ideation                  |
| `decision`        | weighted option comparison                          |
| `explain_eli5`    | simple, analogy-first explanations                  |
| `security_audit`  | security review (OWASP-oriented)                    |
| `sql_query`       | SQL and schema help (SQLite-aware)                  |
| `summarize`       | TL;DR + key points + details summaries              |

## Writing a skill

Create `skills/<name>.md`:

```markdown
# My Skill

Use this when <trigger condition>.

## Procedure
1. Step one.
2. Step two.

## Output
- What to return.

## Rules
- Constraints and edge cases.
```

A good skill names its trigger, gives an ordered procedure, defines the output format,
and states hard rules. Keep it focused — one procedure per file.

## Hot reload

Drop a new `.md` into `skills/` (or edit one). The autoloader picks it up on the next
interval without restarting the agent.
