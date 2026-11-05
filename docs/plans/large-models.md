# Bounded support for the remaining large corpus models

## Goal
- Preserve complete geometry for slicing, provide a bounded browser preview, and handle every valid large corpus input. A correct HTTP 413 is rejection evidence, not model support.
- Preserve Piotr's layout. Invalid case32 remains rejected; never hide a failed conversion behind a manual-price success.

## Scope and context
- Cases8/14/19/20/21/29/37/40/41 are affected: five uploads exceed32 MiB and four smaller3MF scenes exceed current conversion limits.
- Sources:78.4 MB maximum input;3MF central-directory declarations reach522.1 MB expanded/344.8 MB per entry. These declarations are not proof of safe expansion or valid geometry.
- Current production: upload32 MiB, ZIP96 MiB, canonical STL50 MiB, GLB25 MiB, worker2 GiB/25s CPU/30s wall. No global limit increase during the feasibility step.
- The previously asked deployment-profile/default-budget choice has no answer. Local diagnostic work does not enable a new HTTP profile or treat silence as approval.
- Read `docs/corpus-report.md`, geometry `src/lib.rs`, API `models.rs`, slicer and private manifest before changing contracts.

## Milestones

### 1. Measure real conversion costs
- [x] Done: nine files measured; CPU/signal follow-up for three interrupted cases.
- Changes: explicit diagnostic-only example using the current public geometry parser with bounded larger input/archive/output allowances; no output meshes written. Run nine SHA-verified files sequentially without network/secrets, retaining current process RAM/CPU/wall budgets.
- Acceptance: record result, full validated geometry counts/dimensions when available, elapsed time/RSS and process limits. Keep malformed, timeout, allocation failure and output-size failures distinct. Count references alone is insufficient.
- Verify: `cargo build --offline --locked --manifest-path backend/Cargo.toml -p rapidmaker-geometry --example large_model_probe`; isolated runner saves private JSON, no API limit changes.

### 2. Complete the bounded conversion path
- [ ] Pending
- Changes: choose the smallest complete approach from measurements. Keep production mesh exact; preview reduction, if needed, must be explicit and independently tested. Keep ownership, quotas, archive/path checks, lease/retry and process isolation.
- Acceptance: no geometry/settings omitted from the slicer; valid bounds/units/transforms; preview within browser budget; output bytes accounted for. Decide the deployment/default budget before enabling it in HTTP.
- Verify: focused Rust fixtures for limits, repeated components, normals/units, malformed archives and preview-only reduction; exact full-mesh counts/bounds against independently measured inputs.

### 3. Prove upload, preview and quotation
- [ ] Pending
- Changes: extend real API/UI corpus expectations for the nine formerly rejected inputs; preserve small-model coverage.
- Acceptance: every valid supported input receives201 plus private nonempty preview, then a measured price or legitimate print-size/manual decision; no413/422 counted as successful support. Case32 still422 without checkout. Render desktop/mobile and both themes without redesign.
- Verify: opt-in `corpus.spec.ts`/`corpus-ui.spec.ts` with SHA-bound manifest; record failed cases honestly and check relevant concurrency/resource limits.

### 4. Review and integrate
- [ ] Pending
- Acceptance: GPT-6-Sol xhigh review in ten categories, concrete findings fixed, scoped evidence recorded. Do not invent10/10 or declare the whole goal complete from this feature alone.
- Verify: focused build/checks, diff, updated architecture/limits and current corpus report; commit verified changes on `codex/corpus-quality`.

## Progress
- Current: milestone2; production defaults unchanged. Private evidence: `frontend/tests/artifacts/large-model-probe/results-combined.json`, with per-case binary hashes/run provenance and isolated runner scripts.
- Full preview conversion succeeded for8/19/20/29/40:1.49–3.86M triangles,26.8–69.5 MB GLB,0.2–21.3s wall,peakRSS<0.5 GiB. None fits current25 MiB preview budget; this is not HTTP/STL/quote support.
-14 returned `MalformedInput` after14.3s; validity/compatibility unresolved, not declared corrupt.21/37/41 repeated with explicit metrics: childSIGKILL9,CPU25.004–25.009s at configured25/25,wall25.1s,RSS0.27–0.73GiB. Signal cause unresolved; no OOM claim.
- Initial infrastructure attempt lacked `/usr/bin/time` and did not execute parsers; retained separately. Metrics now use isolated Python `resource.getrusage`.
- Next: diagnose14, optimize3MF parsing for21/37/41, choose a bounded preview representation for all nine, then implement exact production mesh and quota/contracts. Budget/default choice remains unanswered; no expanded HTTP profile enabled.
