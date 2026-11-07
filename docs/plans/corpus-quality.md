# Cel: rzeczywiste modele i wiarygodne E2E

Przeczytaj gdy: wznawiasz goal lub oceniasz jego zamknięcie.
Opis: kryteria, aktualny checkpoint i dalsza praca; historia w [raporcie](../corpus-report.md).

## Wymagania

- 41 prywatnych projektów MakerWorld z pochodzeniem/SHA (34×3MF, 7×STL); użytkownik zakończył pobieranie. STEP z fixtures.
- Rzeczywisty upload, prywatny podgląd, slicer i niezależny rachunek ceny. Ręczna ścieżka/odmowa nie oznacza automatycznej wyceny lub obsługi geometrii.
- Obrazy, opcje i interakcje Piotra bez redesignu: konfigurator, zakup/refundy, dostawa, klient/admin, MFA, chat i rzeczywisty email. [Mapa dowodów](../e2e-coverage.md), [kontrolki](../e2e-controls.md).
- Minimalny Rust, proste API/KISS; zachować kwoty, własność, izolację i limity. Nie zmieniać oczekiwań dla zielonego raportu.
- Po etapie Sol xhigh w dziesięciu kategoriach: poprawność, bezpieczeństwo, kwoty, zasoby, błędy, współbieżność, dowody, prostota, utrzymanie, wygląd. Naprawiać konkretne problemy, nie wymuszać 10/10.

## Checkpoint

| Zakres | Zweryfikowany stan |
| --- | --- |
| Korpus/API | Aktualny69bec69:corpus-api-current-69bec69,41PASS/0FAIL/0SKIP;24 ceny slicera z rachunkiem,12 uzasadnionych ręcznych,4wadliwe,1limit;36GLB,391hashów zgodnych. Dawny corpus12-manual-geometry osobno |
| Korpus/UI | Aktualny40e8a5d:41PASS/0FAIL/0SKIP/0flaky w jednym676s przebiegu;36renderów,4wadliwe i1zasobowa odmowa.144 unikalne ujęcia obejrzane,437hashów zgodnych; ceny UI tylko9/20/29 |
| Większe modele | Bezstratny kompaktowy/spatial GLB i jawne uproszczenie wyłącznie podglądu z zachowaniem pełnego STL. [Budżety i dowody](large-models.md). Zespół 41 nadal ponad limitem |
| Nowsza regresja3519cb9 | Storage25PASS, core49PASS/1 opt-in large SKIP, zeroFAIL/flaky; poprawiona etykieta runnera. Hashe niezmienione; sandbox/email inne osobne dowody |
| Pełny zestaw 78d5802 | 136 PASS/0 FAIL/0 flaky/1 SKIP w siedmiu osobnych grupach. Kod wrappera 2 przez pominięcie opt-in large; ten scenariusz osobno 1 PASS. 312 hashów przed/po zgodnych |
| Naprawione przepływy | Tożsamość line_id/model_id/quote_id, zakup gościa → konto, PI decline/leave/resume. Dwa warianty jednego uploadu z prawidłowymi kwotami w obu panelach; duplikacja dostępna przez UI; osobne core i Stripe po1PASS, last-reference cleanup |
| Osiem klientów | eight-clients-final: 1 PASS/3,8 s. 16 równoczesnych estimate → 8 zadań/8 snapshotów/max prób 1; własne ceny i odmowa obcych zasobów; 4 próbki responsywności, nie szeroki load test |
| Błędy uploadu | upload-failures-final: 2 PASS (istniejąca retencja + nowy scenariusz). Rzeczywisty ENOTDIR → 500, błąd insertu po konwersji → 503, brak nowych plików/rekordów, wcześniejszy model zachowany, następny upload 201. Nie ENOSPC |
| Chwilowo zajęty upload | Pre-acceptance marker+bounded retry,12nowych i5cleanup PASS, review bez blokera. Zwykły API zaktualizowany do7fb2a24; realny smoke HTTP busy/retry/GLB/cena/delete PASS |
| Pobieranie i zawieszony upload | Konsultacja wskazała pełne buforowanie plików i brak deadline. Streaming 64 KiB/4 transfery, timeout tylko body 60 s; 10 testów Rust, HTTP 2 PASS oraz osobno 3 PASS. Bez zmiany uprawnień/wyglądu |
| Trwałość uploadu | Utrwalenie katalogu nadrzędnego przed DB; typowany niepewny COMMIT zachowuje pliki. upload-commit-durability-current 1 PASS; 10 Rust PASS i Clippy. Test błędu przy COMMIT to znany rollback, nie utrata ACK/power loss; osierocone katalogi uzgadnia teraz opisany poniżej worker |
| Profil przy płatności | Hash profilu sprawdzany pod blokadą katalogu w końcowej transakcji; regresja PG 1 PASS oraz HTTP 1 PASS (409 i brak prób płatności/zamówień), review bez blokera. Pliki runtime osobno kontroluje API |
| Retencja orphanów | Root flock + nieblokujący fence DB, 24 h/64 wpisy z kursorem, ochrona zadań/holdów; orphan-gc-storage 23 PASS i upload regression 1 PASS. Bez migracji, nowych zależności lub endpointu; bez rotacji root na żywo |
| Zależności Rust | Pierwotny audyt 74bc144: cargo-machete 0.9.2 bez nieużywanych zależności; 13 crate’ów, 99 deklaracji. Streaming dodaje dwa użyte bezpośrednie helpery już obecne w lockfile. Brak ignorelist i zmian; nie dowodzi to prostoty całego kodu |
| Localhost | API18432 stabilne binarium7fb2a24, Vite18430 aktualny; backup DB i2 katalogów. Upload/GLB/native wycena/delete PASS; niezależny rachunek37,05zł zgodny z obecnymi stawkami POC, rentowność niepotwierdzona |

## Dalsza praca i granice

Aktywne lease ma teraz osobny dowód: queue-recovery-20261107-first1PASS/193s (naturalne180s+5s,attempts2,jeden snapshot). Końcowy cleanup po naprawie zweryfikowany oddzielnie fault2PASS i queued1PASS; nie awaria hosta lub dowód startu Bambu przed kill.

1. Ulepszać konkretnie wskazane problemy, nie uruchamiać ponownie pełnych zestawów bez nowej przyczyny. Po zmianie tylko potrzebna regresja i scoped review.
2. Zbędnych bezpośrednich zależności nie wykryto. Pozostałe słabiej udowodnione obszary to ogólna prostota kodu, szersze obciążenie i zużycie zasobów. Osiem klientów nie wyznacza przepustowości produkcyjnej.
3. Jawne ograniczenia: wadliwe/ponadlimitowe źródła, koszty fixture zamiast rentowności firmy, wirtualny passkey i emulowany viewport. Cały goal nadal otwarty; review nie potwierdziło 10/10.

## Uruchamianie

```sh
python frontend/tests/run-all.py --group core --no-build
E2E_REPORT_NAME=eight-clients-current python frontend/tests/run-integration.py concurrency.spec.ts
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-large-current python frontend/tests/run-integration.py --large-models corpus.spec.ts
```

Używaj nowej nazwy raportu, by zachować wcześniejsze wyniki. Konfiguracja w backend/README.md; sekrety poza raportami. Pełne zestawy i testy providerów tylko celowo. Historia etapów, błędów harnessu i review jest w corpus-report.md; starsze checkpointy także w Git.

Dalsze działania zgodnie z poleceniem właściciela konsultować z reviewerem. Poprzednie blocked oparte na samych ocenach nie było techniczną przeszkodą. Konsultacja wskazała dwa konkretne problemy zasobów; oba poprawione i ponownie sprawdzone. Nie czekać na definicję oceny, gdy reviewer potrafi wskazać wykonalną poprawkę. Nie deklarować 10/10 bez dowodów ani powtarzać zestawów tylko dla wyższej liczby.
