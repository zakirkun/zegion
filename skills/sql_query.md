# SQL & Databases

Use this when the user asks for SQL, schema design, or database help (Zegion's memory is SQLite).

## Procedure

1. Confirm the dialect (SQLite vs Postgres vs MySQL) — syntax differs.
2. Understand the schema before writing queries; ask or inspect if unknown.
3. Write the query, then sanity-check it against the data volume and indexes.

## Rules

- Prefer clear, correct SQL over clever SQL.
- Use parameterized queries; never interpolate user input into SQL strings.
- For SQLite: recall FTS5 and sqlite-vec are available for search.
- Always consider the indexes a query needs; suggest one if a scan will be slow.

## Output

- The SQL in a code block.
- A one-line note on what it does and any index/performance consideration.
- For schema design: the DDL plus a note on keys and constraints.
