# ADR-0000: record architecture decisions

- **Date:** 2026-05-19
- **Status:** Accepted
- **Context:** nanobook had no formal record of why specific simulator behaviors (fill timing, cost model wiring, price input shape) were chosen. The Phase 0 audit of nanotrade's backtesting pipeline revealed three architectural defects in nanobook that were undocumented decisions. Without a decision record, future maintainers cannot evaluate whether defaults are still appropriate or whether changes are breaking.
- **Decision:** nanobook adopts Architectural Decision Records for all non-obvious choices. The ADR directory is `docs/adr/`. Files are immutable once accepted; decisions are superseded, not deleted.
- **Alternatives Considered:** (1) inline comments with rationale -- rejected because comments rot when implementation changes; (2) a single DECISIONS.md file -- rejected because it becomes a merge-conflict magnet and difficult to cross-reference; (3) no formal record -- rejected because it perpetuates the problem this audit found.
- **Consequences:** Every new behavioral parameter or interface contract requires one ADR before landing. Code files get a single pointer line above each governed value. The Phase 0 audit document (`nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md`) serves as the first Evidence artifact for ADRs 0001-0004.
- **Evidence:** `nanotrade/plan.md` (Decision traceability section); `nanotrade/docs/audit/2026-05-19-nanobook-fill-model.md`
