# Testy strony

Pełny zestaw: `npm --prefix frontend run test:all`. Pierwszy krok buduje Rust i frontend, dobiera przeglądarkę do lokalnej wersji Playwright; kolejne uruchamiają oddzielne grupy. Wymagane lokalne PostgreSQL/Podman i skonfigurowana .env. Uruchom istniejący kontener: `podman start rapidmaker-catalog-postgres`. Sandbox/email wymagają testowych kluczy Stripe, Furgonetki i poczty; live jest odrzucane.

| Grupa | Komenda po buildzie | Co naprawdę sprawdza |
| --- | --- | --- |
| core | `python frontend/tests/run-all.py --group core --no-build` | Prawdziwe API/DB/parser/slicer, wszystkie opcje UI, modele, granice, ustawienia bezpieczeństwa; bez dostawców |
| sandbox | `python frontend/tests/run-all.py --group sandbox --no-build` | Stripe test, Furgonetka sandbox, kurier/paczkomaty, panele/refundy/logistyka, porzucona płatność |
| email | `python frontend/tests/run-all.py --group email --no-build` | Hasło/passkey + email, admin TOTP, oferta → płatność → warunki/email → odpowiedź chat |
| registration | `python frontend/tests/run-all.py --group registration --no-build` | Osobne bazy dla pierwszego hasła i pierwszego passkey, prawdziwe potwierdzenie emaila |
| contracts | `python frontend/tests/run-all.py --group contracts --no-build` | 20 szybkich kontraktów UI, w większości kontrolowane API; nie dowodzą działania dostawców |

Runner tworzy i usuwa wyłącznie własną bazę rapidmaker_test_browser_*, pliki i konta; API:18436, frontend:18437. Nie uruchamiaj dwóch integracji naraz na tych portach. Normalny localhost i jego zamówienia pozostają poza testem. Logowanie TOTP w fixture jest prawdziwe; nie obchodzimy 2FA. Wirtualny WebAuthn nie zastępuje testu fizycznego klucza. Email tylko void@sieciowiec.xyz. Nie podawaj sekretów w argumentach CLI.

Raporty JSON, screeny i diagnostyka są ignorowane przez Git i prywatne w tests/artifacts (0700; nowe pliki 0600). Nie publikuj surowych artefaktów: błędy narzędzi mogą zawierać stan formularza. Formularze z kodami/kluczami/kartą nie zapisują trace ani screenshotów; wybrane podsumowania i panele z syntetycznymi danymi mają prywatne zrzuty. Automatyczna kopia DOM wyłączona. Zrzuty/diagnostyka mają osobne katalogi grup w artifacts/integration, więc kolejna grupa nie usuwa poprzedniej. Podsumowanie grup: tests/artifacts/summary.json; opis pokrycia i wyników: docs/E2E-REPORT.md. Kod wyjścia: 0 = sprawdzone grupy bez ograniczeń, 1 = błąd, 2 = pominięcie lub jawna blokada dostawcy. Zaznaczenie provider-blocked nie jest potwierdzeniem wykonania podjazdu.

Dla konkretnej regresji: `python frontend/tests/run-integration.py --local-accounts security.spec.ts`, albo `--sandbox --client-totp panels.spec.ts`. Emailowy auth wymaga `--sandbox auth.spec.ts`; rejestracja `--sandbox --registration registration.spec.ts` (E2E_REGISTRATION_METHOD=password/passkey). `--local-accounts` nie ładuje sekretów dostawców. Cennik i dopuszczenie trzech rodzin materiałów w fixture dotyczą wyłącznie nowej bazy testowej.

Inwentaryzacja: `node frontend/tests/inventory-controls.mjs > docs/e2e-controls.md`. Lista szablonów nie jest wynikiem testu ani obietnicą wszystkich kombinacji. Wartości dynamiczne i wyłączone funkcje są opisane w docs/e2e-*-inventory.md. Usunięto stare flows.spec.ts i shipping.spec.ts zależne od nieistniejącego logowania demonstracyjnego; ich aktualne zachowania zastępują realne configurator/models/boundaries/checkout/shipping-checkout/panels/journey oraz kontrakty operations. Nie przywracamy fikcyjnych danych, żeby stare scenariusze przechodziły.
