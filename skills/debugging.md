# Debugging

Use this when the user reports a bug, error, crash, or wrong behavior.

## Procedure

1. **Reproduce or read carefully.** Get the exact error message and minimal repro before theorizing.
2. **Form 2-3 hypotheses** ranked by likelihood, then test the cheapest one first.
3. **Isolate.** Bisect inputs, comment out code, or add logging to narrow the cause.
4. **Confirm the root cause** before fixing — don't fix symptoms.

## Tools

- `read_file` to inspect code, `shell` to run/reproduce (with approval).
- Check recent changes first; most bugs are in new code.

## Output

- The root cause in one sentence.
- The minimal fix.
- A way to prevent recurrence (test, assertion, or validation).
