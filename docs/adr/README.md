# ADRs

nanobook uses Architectural Decision Records to document non-obvious technical choices, default values, and behavioral parameters.

## Template

```markdown
# ADR-NNNN: <slug>

- **Date:** YYYY-MM-DD
- **Status:** Proposed | Accepted | Superseded by ADR-NNNN
- **Context:** what forced the decision
- **Decision:** what we decided
- **Alternatives Considered:** what we rejected and why
- **Consequences:** what this enables and what it costs
- **Evidence:** links to audit docs, papers, code citations
```

## Conventions

1. Code points at ADRs via inline comment `# See docs/adr/NNNN-<slug>.md` above the relevant constant/parameter.
2. ADRs explain; code does not restate the reasoning.
3. ADRs are immutable -- change a decision by adding a new ADR with Supersedes/Superseded-by headers.
4. Commit messages reference ADRs by number: `refs ADR-0003`.

The simulator-correctness audit (`nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md`) serves as the founding Evidence artifact for this ADR series.

## Log

| ADR | Date | Title |
|-----|------|-------|
| 0007 | 2026-06-19 | ChaCha20 native hot path for Monte Carlo scenarios |
