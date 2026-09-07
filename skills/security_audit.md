# Security Review

Use this when reviewing code, config, or a design for security issues.

## Procedure

1. Map the trust boundaries: what input is attacker-controlled?
2. Trace data from every untrusted source to every sink (exec, query, file, network, render).
3. Check the OWASP-relevant classes for the context.

## Focus areas

- Injection (command, SQL, template, path traversal).
- Secrets handling: none in code/logs; loaded from env; never returned to the model.
- AuthZ/AuthN gaps, overly broad permissions.
- Deserialization, dependency risk, error messages leaking internals.

## Output

- Findings as `location [severity] issue → fix`, ordered by severity.
- A short "top risk" callout.
- Note what's done well only if it informs a decision.
