# Milestones: pełne E2E

Statusy: TODO / IN PROGRESS / PASS / FAIL / BLOCKED / NOT RUN. PASS wymaga zapisanego rzeczywistego wyniku; mock i skip nie potwierdzają integracji. Nie zamykamy celu, jeśli naprawialny błąd lub obowiązkowy test pozostaje bez wykonania. Blokady zewnętrzne nazywamy w raporcie.

| Etap | Zakres | Kryterium zakończenia | Stan |
| --- | --- | --- | --- |
| E0 | Inwentaryzacja i plan | Mapa wszystkich kontrolek źródłowych, podział aktywne/dynamiczne/wyłączone/martwe, mapowanie tras i testów | PASS |
| E1 | Środowisko i runner | Powtarzalna izolowana baza/pliki/konta, bez live; rozdzielone realne sandboxy i kontrakty; wyniki do raportu | PASS |
| E2 | Upload i konfiguracja | Każda oferowana opcja i powrót/odtworzenie sprawdzone z efektem; wszystkie formaty i negatywne przypadki; naprawy | PASS |
| E3 | Dane, dostawa i płatność | Rzeczywiste UI: pickup/kurier/paczkomat/punkt, faktura/walidacja, cena/edycja/retry oraz Stripe paid/failure | PASS |
| E4 | Logowanie i ustawienia | Rzeczywiste hasło/passkey + email/TOTP, ochrona tras, zmiany hasła/kluczy/2FA i logout; brak ujawnienia sekretów | PASS |
| E5 | Klient, admin, zwroty, logistyka | Dostępne akcje paneli z efektem serwerowym; partial+remaining refund Stripe, odmowy i dostępna logistyka sandbox; brak fikcyjnych klików | PASS |
| E6 | Integracja i poprawki | Uruchomiony uzgodniony zestaw, błędy naprawione i konkretnie sprawdzone; wybrane screeny desktop/mobile; lista ograniczeń | PASS |
| E7 | Raport MD i przekazanie | Zwięzły E2E-REPORT.md: scenariusze, komendy, faktyczne PASS/FAIL/SKIP/BLOCKED, naprawy, ograniczenia, LOC; zweryfikowany commit/push | PASS |

## Checkpoint

2026-10-26: wykonano E0–E7. [Raport](E2E-REPORT.md): **31 unikalnych integracji/E2E + 20 kontraktów PASS**, bez liczenia ponownych uruchomień jako nowych scenariuszy. Inwentarz: 368 deklaracji w 42 plikach i trzy mapy pokrycia. Prawdziwe STL/STEP/3MF, slicer, checkout, Stripe/refundy, auth, panele, rejestracja i email → chat sprawdzone; naprawy w raporcie.

Logistyka DHL: nadanie dwóch paczek w jednym zleceniu, PDF, propozycje i rezerwacja podjazdu, anulowanie; osobno przekazanie → shipped: 2 PASS. **DPD BLOCKED:** dwie próby nadania odrzucone carrierValidationError (timeout API DPD), brak potwierdzonego nadania w tych próbach. E5 obejmuje działającą ścieżkę DHL; nie oznacza sprawności każdego przewoźnika.

Runner izoluje DB/pliki/konta i odróżnia realne integracje od mocków; obsługuje przerwanie i prywatne artefakty per grupa. Obejrzano desktop/mobile; testy sprawdzają wybrane granice overflow. Komenda całości: npm --prefix frontend run test:all. W tym etapie wykonano grupy i powtórki naprawionych regresji; nie deklarujemy jednego nieprzerwanego przebiegu całego polecenia po ostatniej poprawce.

Rust build/fmt/Clippy oraz wskazane testy modułów PASS; frontend check/build, Python i celowane formatowanie PASS. Granice: fizyczny passkey, load/pentest, produkcja i niewdrożone funkcje nie są zaliczone. Cudze AGENTS.md/result.json pozostawione poza commitem. Kolejny krok dla DPD: powtórzyć nadanie po ustaniu timeoutów sandboxa; nie usuwać zabezpieczeń ponawiania i niepewnych operacji.
