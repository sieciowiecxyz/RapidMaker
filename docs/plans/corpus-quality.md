# Real-model and end-to-end reliability

## Goal
- Use the accepted corpus of 41 distinct MakerWorld downloads, retain provenance, and test actual model processing and price calculation. Further downloads cancelled by the user on 2026-11-05.
- Verify images, configuration, checkout/payment/refund, customer/admin authentication and email through meaningful E2E tests. Fix confirmed defects without changing Piotr's visual style.
- Keep implementation small; preserve money, ownership and resource checks.

## Milestones
1. **Corpus — complete:**41 accepted downloads;34 3MF,7 STL,41 unique payload hashes. Keep locally ignored/private with provenance and unusual filenames. No further downloads or MakerWorld STEP search required; STEP remains covered by repository fixtures.
2. **Model/pricing robustness** — add an opt-in corpus runner using real upload/preview/quote APIs. Report each result, resource limits, failures and expected oversized rejections; distinguish unsupported geometry, manual quotes and defects. Test quantities and measured pricing invariants, then fix defects with focused regressions.
3. **Frontend assets/interactions** — verify all rendered images, examples, desktop/mobile and both themes. Keep existing controls and style. Add missing assertions instead of duplicating existing suites.
4. **Accounts and commerce** — execute isolated integration suites for auth/MFA, customer/admin ownership, messages/mail, shipping and Stripe test/refunds. Never count mocked providers as live evidence. Record external blockers and fix actual product failures.
5. **Report** — concise results, commands, test scope, known failures/NOT RUN, source commit and evidence paths. Goal remains unfinished while any explicit requirement lacks proof.

## Review gate
After each implementation step, request a GPT-6-Sol xhigh review across correctness, security/authorization, money integrity, resource limits, error handling, concurrency, test evidence, simplicity, maintainability and visual/contract compatibility. Resolve findings and request another review as needed; never fabricate a 10/10 score. A review score alone does not prove completion.

## Current checkpoint

The user accepted41 models and cancelled further downloads on 2026-11-05. CAPTCHA is no longer a task blocker or required user action. Existing test results and remaining limitations below are unchanged; only the download milestone is complete.

Latest96 MiB API check: `expanded96-optimized`,4 PASS/3 FAIL,165.2s;7/13/38 newly priced,31 manual;24/39 resource limit and32 invalid metadata still rejected. Geometry29 PASS/1 optional ignored; build/Clippy/fmt PASS. UI `expanded96-ui`:7 PASS/194.8s (four renders, three explicit rejections, not seven prices); tightened invalid32 rejection assertion:1 PASS/23.5s (`expanded96-invalid32`), final review found no remaining concrete blocker in this slice. Full41 baseline below remains historical64 MiB. Sol xhigh found no blocking code defect; doc/evidence findings corrected; grades7–9/10.

Branch `codex/corpus-quality`; commands, artifacts, detailed scope and prior results: [report](../corpus-report.md).

- **Downloads:**41/41 accepted distinct sources/hashes;34 3MF,7 STL. Originals and manifest remain ignored/private. Further downloads and CAPTCHA handling cancelled by the user.
- **Corpus:**`corpus41-entry64`,30 PASS/11 FAIL/262.2s;16 measured prices,9 manual quotes,5 expected413,11 resource rejections. No skips/flaky. Latest run changes only case9 to measured price; earlier fixes rescued5/22/25. Limits:512 ZIP entries,64 MiB expanded/per entry,1M unique vertices/triangles; worker30s/2GiB unchanged. Do not mask resource failures as successful quotation. A separate bounded192MiB diagnostic of the preceding12 failures identified topology/metadata/triangle/time barriers;192MiB is NOT application policy.
- **Multi-model invoice:**new real API test two uploads/q1+10, independent aggregate invoice/one setup/exact allocation, tampered and foreign snapshots rejected. `multi-model-pricing`:1 PASS4.6s, final Sol review no remaining concrete issue in diff; current `core.json`:31 PASS/0 FAIL/0 skip/flaky234.6s, basec13dfa2+new test; external providers not rerun.
- **Prices:**fixed integer invoice oracles and real slicer quantities1/2/10/100/1000; API checks every money component and batch sums. Fixture purchase cost100 PLN net/kg proves arithmetic, not production profitability. Runtime fingerprint guards launcher/ELF, not all installed libraries/assets.
- **Models/UI:**STL/STEP/3MF private GLB, exact indexing, mirror winding, preserved painting compatibility. Geometry29 PASS/1 optional ignored, Clippy/build PASS. Configurator6 PASS includes178 PLA shade choices and five families; this is not178 independent slicer runs.
- **Appearance:**public/account routes56 captures, populated panels44, showcases20. Both themes/desktop/mobile, fonts/images/static errors/overflow and real canvas interaction checked within those scenarios. Manual screenshots inspected; no claim of every SVG or pixel-perfect future visual baselines. Piotr style preserved.
- **Accounts/commerce:**real local MFA/ownership plus separate iCloud auth, registration(password/virtual passkey), manual offer/email→chat, Stripe sandbox payment/refunds and shipping runs recorded in report. Physical passkey and all possible provider/failure combinations are not proved.
- **Combined core:**latest `core.json` on basec13dfa2 plus multi-model test:31 PASS/0 FAIL/0 skipped/0 flaky,234.6s. Real local API/DB/slicer/MFA, configurator, quotes, ownership/retention and populated panels; external providers run separately. Previous564977f:30 PASS241.3s.
- **Latest sandbox:**`sandbox.json` onf60b381:9 PASS168.0s0skip/flaky/no providerblocked. Strengthened selected-carrier/amount/intent/persistedorder checks separately: `selected-carrier-payment.json`2 PASS35.3s; real Stripe/Furgonetka, no product/style change. Review found/fixed assertion gap; finalscope grades7–9/N/A, not10. Full9 precedes strengthened2; no combined-run claim.
- **Concurrent payment start:**extended unpaid scenario:4parallel same-key POSTs→one attempt/order/provider ID, conflict checks and retained expiry cleanup. `concurrent-unpaid-payment`:1 PASS5.9s; source title only clarified afterward. Real Stripe test/no charge, final Sol review no blocking finding, scores7–8/N/A.
- **Latest email:**source `8dbdb07`, `email.json`:2 PASS/0 FAIL/0 skipped/0 flaky213.2s. Actual iCloud MFA, manual offer notice, Stripe test payment/terms email and one inbound reply in chat; virtual passkey only. Sole recipient void@sieciowiec.xyz.
- **Large real-model UI:**`corpus-ui-final.json`6 PASS/119.8s,0 skipped/flaky: four real large3MFs plus two authored showcase tests,36 scene captures, now all inspected in four scaled overview sheets (not pixel-perfect review). True UI upload/private GLB/pixels/rotation, case9 displayed price equals server cents, other three explicitly manual due size. Fixed UTF-8 locale in test browser and ellipsized long preview filenames without losing full text or changing design. Build0/0. Current full core not rerun.
- **All accepted files through UI:**`corpus-ui-all41-final.json`41 PASS235.0s:25 rendered models/100 scene captures,9 server-resource UI errors,7 form-size rejections. All100 frames inspected as scaled overview sheets. Stronger settled rejection/fully selected settings checks targeted-retested9 PASS5.4s after the full run. Separate API result stays30 PASS/11 FAIL; UI negative handling is not successful pricing. Product/limits unchanged in that run. User approved larger-model support on2026-11-05; the controlled96MiB expansion below is implemented and focused-verified.
- **Reviews:**GPT-6-Sol xhigh after each implementation; actionable findings fixed, scores honest and still below10/10 for full scope. No review score replaces tests.

## Larger-model expansion (approved2026-11-05, implemented)

1. Raise only decompressed ZIP total/per-entry allowance64→96 MiB; keep upload,512 entries,1M mesh,GLB,CPU/RAM/wall limits. Test exact byte boundaries.
2. Optimize geometry/lib3mf in local dev builds instead of extending process time; preserve debug assertions/overflow checks. Verify real candidate imports and measured prices.
3. Keep invalid metadata/topology and over-complex meshes rejected; no guessed namespace repair or validator bypass.
4. Run actual API/UI regression, report changed outcomes and review with Sol xhigh. No frontend redesign.

## Next

1. Resolve remaining resource-bound models within safe resource policy; keep exclusions and unsupported assemblies explicit.
2. Fill demonstrated interaction/provider/visual gaps with focused tests; use real sandbox evidence and distinguish NOT RUN/blocked from PASS.
3. Keep the full goal open until its requirements have evidence. Upstream low `deepmerge` advisory and production-cost uncertainty remain documented limitations.
