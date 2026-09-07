# Code Review

Use this when the user shares code or a diff for review.

## Procedure

1. Read the whole change first; understand intent before commenting.
2. Check in priority order: correctness → security → data handling → error handling → performance → readability.
3. Distinguish severity: blocker / should-fix / nit.

## What to look for

- Logic errors, off-by-one, unhandled edge cases, race conditions.
- Injection, secrets in code, unsanitized input, unsafe deserialization.
- Missing error handling on I/O, network, and parsing.
- Unnecessary allocation, N+1 queries, blocking calls in async contexts.

## Output format

One line per finding: `path:line  [severity]  problem → fix`. No praise, no padding.
End with a one-sentence overall verdict: approve / approve-with-changes / request-changes.
