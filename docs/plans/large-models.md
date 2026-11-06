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

## Explicit backend profile (not enabled on the user server)

`MODEL_PROFILE` accepts only `standard` (default) or `large`; large requires explicit `AUTH_MODE=localhost`. No `.env` changes. Frontend now reads the32/96MiB upload limit from public `/api/models/limits`; failure falls back to25MiB. Private GLB is independently capped at2M triangles/25MiB. Three real large UI cases are verified below; this is not general large-model readiness. Deployment/default-budget question stays open.

| API budget | standard | large (opt-in) |
| --- | ---: | ---: |
| Input |32MiB|96MiB|
| Expanded ZIP total/per entry; entries |96MiB;512|512MiB;512|
| Exact canonical STL |50MiB|200MiB|
| Unique vertices |1M|4M|
| Triangles from STL bytes |1,048,574|4,194,302|
| Private GLB |25MiB|25MiB|
| Geometry process |2GiB/25CPU/30wall|same|
| Bambu per actual batch |4GiB/60CPU/65wall|4GiB/120CPU/125wall|

Overall quote deadline150s and DB lease180s remain; two large batches may exceed the overall deadline and are not promised. Slicer library's existing public wrapper retains64MiB/60CPU; API explicitly passes50/200MiB and60/120CPU. No arbitrary client-supplied budgets. Source read is bounded and checked against file metadata before preparing the private STL. Existing aggregate quotas20models/256MiB per owner,1000/8GiB globally include source+STL+GLB and stay unchanged.

Migration0023 widens only absolute stored source/mesh checks to96/200MiB; default insert wrapper still32/50, large callers pass bounded explicit allowances. Previously accepted files remain privately downloadable after profile rollback under absolute persisted-read caps; current admission settings do not invalidate held order files. Ownership/admin mesh gates and exact stored lengths remain. Legacy analysis acquires the shared conversion permit before body extraction; total body+analysis deadline60s prevents an indefinitely held slot.

Evidence: `large-profile-api-final.json`3PASS25.8s forSHA-bound cases8/20/40, exact canonical74–84MB/counts/bounds and original hashes/privateGLB, outsider401/404 (before later CPU/admission/read fixes). Initial3FAIL only overstrict cache string expectation; actual `no-store` was valid, final asserts that directive. `large-profile-quote.json`2PASS/1FAIL:20 reached Bambu but3attempts exited137 at~33.4s; no OOM diagnosis. `large-profile-quote120.json`1PASS52.8s after explicit120CPU:20 got1measured plate/7687grosze with independent fixture invoice. Not production purchase costs/profitability.

`large-profile-storage.json`20PASS0skipped in isolated databases, including widened migration/explicit allowances/quotas. Geometry39PASS/1optionalignored; final API29PASS/1DBignored and slicer15PASS/3optionalignored; relevant DB cases run separately above. Standard integration `large-profile-standard-regression.json`:11PASS21.7s,3deliberate large-profile skips due former filename matching; renamed to `large-profile.spec.ts` so explicit `models.spec.ts` selection no longer includes it. Real stalled-body100Continue test gets429 before body completion and recovers200 after disconnect. Private-read unit covers persisted files above standard caps/admin denial/length errors; no actual process-restart proof. Clippy4packages/all-targets, normal app build, Python syntax, formatting/diff checks passed. Final `large-profile-complete.json`:3PASS/67.7s,0skip/flaky, all three uploads/full geometry/private GLB plus actual model20 slice+independent price, after read/admission/CPU fixes.

Run only on the disposable integration DB (never changes local `.env`):
```sh
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=large-profile-complete python frontend/tests/run-integration.py --large-models large-profile.spec.ts
```
Remaining: server cleanup/cancellation for removed models,19/29 preview>25MiB,41 SDK runtime, full newly accepted-model pricing/quantity/resource evidence. Topology-invalid14/21/37 and malformed32 stay explicit rejections. No413/422 counted as supported upload.

Final scoped GPT-6-Sol xhigh rereview after fixes: no blocker. Correctness9/security9/money9/resources8/errors9/concurrency8/evidence9/simplicity8/maintainability8, visual N/A. No reviewer rerun. Explicit remaining gaps: actual restart HTTP rollback, two large batches under150s, expanded browser support. Not10/10 or goal completion.

## Browser checkpoint and next action
- `large-profile-ui.json`:3PASS78.1s, cases8/20/40 through the real form; privateGLB, exact independently counted1,567,758/1,487,438/1,677,324 rendered triangles (zeroArea0), zoom/pixels/overflow,1440/390/light/dark.12captures visually inspected; viewport simulation is not physical mobile hardware evidence.
- `large-profile-ui-price.json`:1PASS78.9s, case20 option clicks→actual slicer→server cart7686grosze→same UI price/enabled checkout. Different run/configuration from earlier7687grosze; neither is a fixed expected production price. No payment/provider claim here.
- Codec24+mocked upload-limit7 contracts:31PASS6.4s (`glb-and-model-limits-contracts-final.json`). Initial collection failed only duplicate generated test name; corrected with value type.
- Standard regression:12PASS/1FAIL in13 tests (`large-ui-standard-regression.json`); stale25MB expectation updated to32MB and new32MB+1 rejection boundary. Focused `large-ui-standard-boundary.json`2PASS10.1s: repaired scenario plus valid padded ASCII STL25MiB+1→201/privateGLB12triangles. Do not combine these into one full-suite PASS.
- Backend limits endpoint HTTP unit/appbuild/Clippy and frontend Svelte-check0errors/0warnings+Vitebuild passed. No CSS/shader/normalizer changes/new deps. LocalSTL25MiB/1Mtri remains independent of backend. Metadata cached until reload; failed loads retry.
- GLB2M ceiling permits144,000,000B interleaved vertices plus72,000,000B temporary positions and25MiB wire; degenerate trimming may copy another output buffer; GPU/main/cache add memory. Old48MiB LRU evicts other meshes but can retain one oversized selected mesh. Do not claim low-memory phone readiness.

### Next: reclaim removed working uploads (before general large-profile readiness)
Review found existing local-only removal/sample replacement leaves accepted models and in-flight uploads charged to256MiB owner quota for24h. Implement owner-authorized, Origin/CSRF-protected removal with durable marking under the existing queue lock. Reuse cleanup; never delete queued/running-job files or held/paid order artifacts. Reconcile late upload results after local removal, and show real cleanup failures. Test same-owner remove→physical deletion/quota reuse, foreign owner denial, upload/removal race, active job and order hold protection. No new broker/schema unless owning code requires it; keep backend profile opt-in. This is actionable remaining work, not a passed requirement.
