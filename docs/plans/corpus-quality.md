# Real-model and end-to-end reliability

## Goal
- Finish 100 distinct MakerWorld source downloads, retain provenance, and test actual model processing and price calculation.
- Verify images, configuration, checkout/payment/refund, customer/admin authentication and email through meaningful E2E tests. Fix confirmed defects without changing Piotr's visual style.
- Keep implementation small; preserve money, ownership and resource checks.

## Milestones
1. **Corpus** — 41/100 downloaded; 34 3MF and 7 STL, 41 unique payload hashes. Complete remaining downloads and seek original STEP. Store locally outside Git; do not fabricate formats. Verify manifest, archive bounds, source and filenames. MakerWorld CAPTCHA requires user completion before more downloads.
2. **Model/pricing robustness** — add an opt-in corpus runner using real upload/preview/quote APIs. Report each result, resource limits, failures and expected oversized rejections; distinguish unsupported geometry, manual quotes and defects. Test quantities and measured pricing invariants, then fix defects with focused regressions.
3. **Frontend assets/interactions** — verify all rendered images, examples, desktop/mobile and both themes. Keep existing controls and style. Add missing assertions instead of duplicating existing suites.
4. **Accounts and commerce** — execute isolated integration suites for auth/MFA, customer/admin ownership, messages/mail, shipping and Stripe test/refunds. Never count mocked providers as live evidence. Record external blockers and fix actual product failures.
5. **Report** — concise results, commands, test scope, known failures/NOT RUN, source commit and evidence paths. Goal remains active while any explicit requirement lacks proof.

## Review gate
After each implementation step, request a GPT-6-Sol xhigh review across correctness, security/authorization, money integrity, resource limits, error handling, concurrency, test evidence, simplicity, maintainability and visual/contract compatibility. Resolve findings and request another review as needed; never fabricate a 10/10 score. A review score alone does not prove completion.

## Current checkpoint

Branch `codex/corpus-quality`; commands, artifacts, detailed scope and prior results: [report](../corpus-report.md).

- **Downloads:**41/100 distinct sources/hashes;34 3MF,7 STL, no MakerWorld STEP. Originals and manifest remain ignored/private. Remaining59 require human MakerWorld CAPTCHA completion; no bypass.
- **Corpus:**`corpus41-entry64`,30 PASS/11 FAIL/262.2s;16 measured prices,9 manual quotes,5 expected413,11 resource rejections. No skips/flaky. Latest run changes only case9 to measured price; earlier fixes rescued5/22/25. Limits:512 ZIP entries,64 MiB expanded/per entry,1M unique vertices/triangles; worker30s/2GiB unchanged. Do not mask resource failures as successful quotation. A separate bounded192MiB diagnostic of the preceding12 failures identified topology/metadata/triangle/time barriers;192MiB is NOT application policy.
- **Prices:**fixed integer invoice oracles and real slicer quantities1/2/10/100/1000; API checks every money component and batch sums. Fixture purchase cost100 PLN net/kg proves arithmetic, not production profitability. Runtime fingerprint guards launcher/ELF, not all installed libraries/assets.
- **Models/UI:**STL/STEP/3MF private GLB, exact indexing, mirror winding, preserved painting compatibility. Geometry29 PASS/1 optional ignored, Clippy/build PASS. Configurator6 PASS includes178 PLA shade choices and five families; this is not178 independent slicer runs.
- **Appearance:**public/account routes56 captures, populated panels44, showcases20. Both themes/desktop/mobile, fonts/images/static errors/overflow and real canvas interaction checked within those scenarios. Manual screenshots inspected; no claim of every SVG or pixel-perfect future visual baselines. Piotr style preserved.
- **Accounts/commerce:**real local MFA/ownership plus separate iCloud auth, registration(password/virtual passkey), manual offer/email→chat, Stripe sandbox payment/refunds and shipping runs recorded in report. Physical passkey and all possible provider/failure combinations are not proved.
- **Combined core:**source `564977f`, `core.json`:30 PASS/0 FAIL/0 skipped/0 flaky,241.3s. Includes current label/mirror/showcase changes, real local API/slicer/MFA and populated panels; external providers run separately.
- **Latest sandbox:**`sandbox.json`7 PASS/2 FAIL136.9s; both logistics radio-scroll failures fixed in test only. Focused `logistics-current.json`2 PASS73.5s, no provider-blocked annotation: DHL sandbox booking/PDF/pickup/cancel and tracking/handover confirmed. Full9 after fix NOT RUN; do not describe split runs as one9/9. No product/style change.
- **Latest email:**source `8dbdb07`, `email.json`:2 PASS/0 FAIL/0 skipped/0 flaky213.2s. Actual iCloud MFA, manual offer notice, Stripe test payment/terms email and one inbound reply in chat; virtual passkey only. Sole recipient void@sieciowiec.xyz.
- **Large real-model UI:**`corpus-ui-final.json`6 PASS/119.8s,0 skipped/flaky: four real large3MFs plus two authored showcase tests,36 scene captures. True UI upload/private GLB/pixels/rotation, case9 displayed price equals server cents, other three explicitly manual due size. Fixed UTF-8 locale in test browser and ellipsized long preview filenames without losing full text or changing design. Build0/0. Current full core not rerun.
- **Reviews:**GPT-6-Sol xhigh after each implementation; actionable findings fixed, scores honest and still below10/10 for full scope. No review score replaces tests.

## Next

1. Complete100 source downloads once CAPTCHA is cleared; extend manifest and rerun corpus without fabricating formats, dropping difficult cases or hiding failures.
2. Resolve remaining resource-bound models within safe resource policy; keep exclusions and unsupported assemblies explicit.
3. Fill demonstrated interaction/provider/visual gaps with focused tests; use real sandbox evidence and distinguish NOT RUN/blocked from PASS.
4. Keep the full goal open until its requirements have evidence. Upstream low `deepmerge` advisory and production-cost uncertainty remain documented limitations.
