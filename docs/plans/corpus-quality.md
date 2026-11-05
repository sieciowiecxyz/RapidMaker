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

## Progress
- Branch `codex/corpus-quality`; detailed verified outcomes and remaining limits: [checkpoint report](../corpus-report.md).
- Corpus41/100 (34 3MF,7 STL,41 distinct hashes). MakerWorld CAPTCHA is visible; user handoff requested and pending. No MakerWorld STEP yet.
- Latest41-case run:15 measured prices,6 manual,5 expected413,15 resource-limit failures. No unclassified decode/proxy failures remain in this run;15 positive-pipeline assertions remain FAIL. Do not describe PASS totals as automatic prices.
- Fixed known Bambu triangle metadata, slicer CPU parallelism (same budgets), actionable geometry errors, embedded delivery CSS/warning and EPIPE oversize status. Normal author style preserved.
- Real auth/security/mail, checkout/shipping sandbox, partial/full refunds and selected desktop/mobile reviewed; report records exact scope. No production-profitability or all-assets claim.
- Geometry21 PASS; slicer7 PASS plus native real model; build0errors/0warnings. Audits:Rust0 vulnerabilities/warnings; one underlying low development deepmerge advisory without upstream patch.
- Sol xhigh reviews identify no blocker for targeted fixes but remain below10/10 for full scope. Keep full goal active.
- Next: complete downloads when available, resolve corpus acceptance/policy for resource-bound models without hiding failures, verify production cost inputs, full asset and remaining E2E coverage.
- Pricing checkpoint:6 Rust tests PASS with five fixed invoice oracles; real slicer q1/2/10/100/1000 physical and batch-aggregation checks PASS. API q1/10/1000 full-component oracle1 PASS7.9s. This is arithmetic under explicit fixture costs, not proven production profitability.
- Configurator6 PASS130s (real local API, injected network faults only), including178 PLA shades/UI state, five families, advanced controls, desktop/mobile WebGL zoom pixels. Sol reviews preserve limits of evidence; full asset/auth/concurrency scope remains separate.

- Runtime checkpoint: launcher+ELF SHA-256 persisted/deduplicated and checked by worker/cart. Fresh build and API regression1 PASS8.3s (fresh/missing/wrong/restored identity plus batch money oracles). Shared libraries/assets and in-flight update races remain outside this guard; restart after changes. Manual quote/iCloud/Stripe/email→chat journey1 PASS176.7s.

- Corpus preparation is now reproducible offline: prepare-corpus.py plus5 synthetic regressions and41-case read-only validation PASS. Exact vertex indexing preserves triangles while rescuing cases3/6(prices) and36(preview/manual); geometry23PASS, freshbuildPASS, full41 rerun26PASS15FAIL withzero new failures, realformatUI1PASS. Sol review8–9/10; large-container policy,100 downloads and broader coverage remain open.

- Visual checkpoint: public/account/admin theme reset reproduced and fixed through one small preference helper, noCSS redesign.5 realAPI/localMFA tests PASS38.2s:14 routevariants x2themes x2viewports=56screenshots; images/fonts/staticasseterrors/overflow, explicit headings, admin toggle/reload, deniedstoragefallback.4 screenshots manuallyviewed. Empty fixture panels only; populatedorder/dialog/Stripe and allSVG coverage remain open. Sol scopedreview8–9/10.

- Concurrent quote checkpoint:4guests/8requests→4ownedjobs/4snapshots, cache reuse, real runningworker + responsivecatalog, foreign completed matchingcheckout409/owned exactmoney200. Focused1PASS3.4s; counts scoped to own validatedmodelIDs. Wholecore27PASS195.9s after dedicated negativeTOTPfixture; latest oracle edits targeted-retested only. UniquevertexlimitUI fixture corrected/PASS. New concurrency+visual suites registered in core. Sol8–9/10, no productionauth/visual changes.

- Populated/registration checkpoint: real iCloud firstpassword+virtualpasskey1PASS each; unpaidmanualorder+chat/privateGLB2themes2widths1PASS10s/44captures. Fixedone statuscontrast1.32→6.66light/13.38dark throughsharedtoken, no layoutredesign. Fullcore28PASS245.1s. RunnerTERMcleanupfaults addressed and manuallyinjected; finalcleanuprevision normal1PASS1.3s (fullcoreprecededlastcleanup tweak). Sol7–9 scoped, no blocker; raw material/colorlabels remain a UI readability gap.
