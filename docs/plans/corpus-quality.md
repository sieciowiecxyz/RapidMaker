# Real-model and end-to-end reliability

## Goal
- Finish 100 distinct MakerWorld source downloads, retain provenance, and test actual model processing and price calculation.
- Verify images, configuration, checkout/payment/refund, customer/admin authentication and email through meaningful E2E tests. Fix confirmed defects without changing Piotr's visual style.
- Keep implementation small; preserve money, ownership and resource checks.

## Milestones
1. **Corpus** — 41/100 downloaded; 34 3MF and 7 STL, 41 unique payload hashes. Complete remaining downloads and seek original STEP. Store locally outside Git; do not fabricate formats. Verify manifest, archive bounds, source and filenames. Current browser downloads time out; cause unconfirmed.
2. **Model/pricing robustness** — add an opt-in corpus runner using real upload/preview/quote APIs. Report each result, resource limits, failures and expected oversized rejections; distinguish unsupported geometry, manual quotes and defects. Test quantities and measured pricing invariants, then fix defects with focused regressions.
3. **Frontend assets/interactions** — verify all rendered images, examples, desktop/mobile and both themes. Keep existing controls and style. Add missing assertions instead of duplicating existing suites.
4. **Accounts and commerce** — execute isolated integration suites for auth/MFA, customer/admin ownership, messages/mail, shipping and Stripe test/refunds. Never count mocked providers as live evidence. Record external blockers and fix actual product failures.
5. **Report** — concise results, commands, test scope, known failures/NOT RUN, source commit and evidence paths. Goal remains active while any explicit requirement lacks proof.

## Review gate
After each implementation step, request a GPT-6-Sol xhigh review across correctness, security/authorization, money integrity, resource limits, error handling, concurrency, test evidence, simplicity, maintainability and visual/contract compatibility. Resolve findings and request another review as needed; never fabricate a 10/10 score. A review score alone does not prove completion.

## Progress
- Branch `codex/corpus-quality`; detailed verified outcomes and remaining limits: [checkpoint report](../corpus-report.md).
- Corpus41/100 (34 3MF,7 STL,41 distinct hashes). Download tools still time out while logged in; asynchronous manual-download clarification pending. No MakerWorld STEP yet.
- Final41-case run:13 measured prices,5 manual,5 expected413,18 resource-limit failures. No unclassified decode/proxy failures remain in this run;18 positive-pipeline assertions remain FAIL. Do not describe PASS totals as automatic prices.
- Fixed known Bambu triangle metadata, slicer CPU parallelism (same budgets), actionable geometry errors, embedded delivery CSS/warning and EPIPE oversize status. Normal author style preserved.
- Real auth/security/mail, checkout/shipping sandbox, partial/full refunds and selected desktop/mobile reviewed; report records exact scope. No production-profitability or all-assets claim.
- Geometry21 PASS; slicer7 PASS plus native real model; build0errors/0warnings. Audits:Rust0 vulnerabilities/warnings; one underlying low development deepmerge advisory without upstream patch.
- Sol xhigh reviews identify no blocker for targeted fixes but remain below10/10 for full scope. Keep full goal active.
- Next: complete downloads when available, resolve corpus acceptance/policy for resource-bound models without hiding failures, fingerprint runtime, independent pricing/quantity checks, full asset and remaining E2E coverage.
