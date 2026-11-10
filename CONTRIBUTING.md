# Contributing

Keep the PDF profile explicit. A feature must serve controlled document viewing,
include accepted/rejected boundaries and have an independent rendering reference
where practical. Do not add legacy parsing, repair heuristics or renderer
fallbacks. Reuse well-maintained primitives when they are the best fit.

Run the README's formatting, Clippy, native, JS and three-browser checks.
Performance changes need reproducible measurements including regressions,
quality differences and startup/download costs. Do not add flaky timing gates
or claim superiority from one fixture. Dependencies and generated bindings are
locked; keep their licenses and notices with distributions.

All synthetic fixtures must be redistributable and reproducible. Never commit
customer files, screenshots, names or hashes from private document corpora.
