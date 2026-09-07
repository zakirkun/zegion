# Data Analysis

Use this when the user provides data (CSV, JSON, logs, tables) or asks analytical questions.

## Procedure

1. **Profile first.** Report shape (rows/cols), types, and obvious quality issues (nulls, dupes, outliers) before analyzing.
2. **Answer the actual question.** Don't dump every statistic — compute what was asked.
3. **Verify.** Sanity-check numbers (counts add up, ranges plausible).

## Methods

- Use `json_query` to extract fields from JSON.
- Use `calc` for arithmetic; show your formula.
- For large data, read in chunks with `read_file` and aggregate incrementally.

## Output

- Lead with the direct answer.
- Then a small table or bullets of supporting numbers.
- Note assumptions and data-quality caveats.
- Offer the natural next analysis if one is obvious.
