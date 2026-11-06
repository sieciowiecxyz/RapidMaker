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
- [ ] In progress: exact compact GLB implemented; expanded HTTP limits still disabled.
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
- Initial probe:14 returned `MalformedInput` after14.3s; validity/compatibility unresolved, not declared corrupt.21/37/41 repeated with explicit metrics: childSIGKILL9,CPU25.004–25.009s at configured25/25,wall25.1s,RSS0.27–0.73GiB. Signal cause unresolved; no OOM claim.
- Initial infrastructure attempt lacked `/usr/bin/time` and did not execute parsers; retained separately. Metrics now use isolated Python `resource.getrusage`.
- Next: keep topology rejections explicit, investigate remaining CPU-bound41, choose a bounded preview representation for valid inputs, then implement exact production mesh and quota/contracts. Budget/default choice remains unanswered; no expanded HTTP profile enabled.

- Development codec/XML optimizations: `zlib-rs` and `quick-xml` now use opt-level2, with inherited debug/overflow checks. No dependency versions, release policy or API budgets changed. Private A/B: `large-model-probe-xml-opt/comparison.json` (five fixed-SHA cases, separate binary hashes).
- Codec-only measurement before SDK diagnosis: case20 retained the same preview geometry/counts/dimensions/GLB length: observed15.674s→12.893s with zlib→10.034s with both. One sample per profile, not a latency guarantee.14 still `MalformedInput` (8.884s);21/37 now reach `MalformedInput` at22.302/20.542s instead of SIGKILL. Their validity remains unresolved.41 still SIGKILL9 at25s CPU.
-31 geometry tests PASS/1 optional ignored; `xml-zlib-models.json`:5 real API tests PASS/4.1s; main/example builds and fmt/diff-check PASS. No new supported HTTP case. Next remains diagnosing parser rejections and bounded preview/production output.

- Rejection diagnosis resolved for14/21/37: exact existing `geometry_package` followed by identical SDK config rejects a non-manifold edge shared by4 triangles (maximum2). Private `sdk-rejection/normalized-results.json` retains SHA/source provenance; raw SDK results are separate and were not substituted for normalized evidence. Temporary diagnostic example removed after preserving source privately.
- New `NonManifoldMesh` keeps the same rejection, maps upload worker exit67 toHTTP422 `model_topology_invalid` with a static repair message. No raw SDK text/object IDs are exposed; changed SDK wording falls back to generic rejection. Large diagnostic rerun14/21/37 confirms this typed result (9.040/22.522/20.447s), not accepted HTTP support.
- Synthetic934B fixture:33 geometry tests PASS/1 optional ignored; API resource classifier1 PASS. `topology-models-ui-final.json`:6 PASS/5.5s, exact safe rejection, no accepted model/checkout/preview, then valid recovery. Initial test hit Chromium response-body eviction; browser status/UI plus same-browser-context identical-input HTTP check replaces inspector-body dependency. Error-row label now says analysis failed, no CSS/layout change; screenshot inspected. Backend/example/frontend builds, Clippy geometry/API all-targets, fmt/Prettier/diff-check PASS.

- Lossless GLB step: up to64 local-u16 primitives, legacy u32 when smaller; exact positions/order/degenerates and bounded JSON/output. Five valid large diagnostics reduced bytes19–32%;8/20/40 fit25MiB,19/29 do not. Same counts/bounds; no HTTP support claim or cap increase. See corpus-report.md for byte table and source/binary provenance.
- Geometry38 PASS/1 optional ignored; codec22 PASS; final API/models6 PASS; large UI24/39 two PASS with8 inspected captures. The UI run precedes the accessor min/max precision fix; final gltf-reader unit+API regression cover it. No UI redesign/new dependencies.

- Final GPT-6-Sol xhigh read-only review: no blocker. Scores/10: correctness9, security9, money9, resources9, errors9, concurrency N/A, evidence9, simplicity8, maintainability8, visual compatibility9. Reviewer did not rerun tests. Full64-chunk test exercises planner/layout only; small fixtures independently round-trip, large24/39 actual API/UI. Whole goal remains open.

- Follow-up CPU diagnosis: explicitly Stored internal replacement ZIP entries were trialed then fully reverted: case20 baseline10.022s vs10.018s;41 both SIGKILL9 near25s CPU. No observed improvement, memory tradeoff not retained. Private matched-source/binary provenance: `large-model-probe-zip-comparison/`. A separate temporary phase-only binary places case41 normalization at1.247s; no root-validation completion marker before25s CPU. Bottleneck is inside the existing full SDK root/package validation call; do not skip it or call the signal an OOM. Instrumented source preserved privately, all probe-only source changes removed; normal app/probe rebuilt.
