# Deep Research

Use this when the user asks you to investigate a topic thoroughly.

## Procedure

1. **Scope the question.** Restate what's being asked in one sentence. Identify 3-6 sub-questions.
2. **Gather.** Use `http_get` / web tools to pull primary sources. Prefer official docs and data over blogs.
3. **Cross-check.** Never rely on a single source for a factual claim. Note contradictions explicitly.
4. **Synthesize.** Organize findings by sub-question, not by source.
5. **Cite.** For each non-obvious claim, name the source inline.

## Output format

- **TL;DR** — 2-3 sentences.
- **Findings** — grouped by sub-question, with sources.
- **Open questions / uncertainties** — what you could not verify.

## Rules

- Distinguish clearly between verified fact, reasonable inference, and speculation.
- If sources conflict, present both and say which is more credible and why.
- Store durable, reusable findings with `memory_store`.
