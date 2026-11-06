# Cel: rzeczywiste modele i wiarygodne E2E

Przeczytaj gdy: kontynuujesz cel lub oceniasz jego zamknięcie.
Opis: aktualne kryteria i następna praca; szczegóły dowodów w [raporcie](../corpus-report.md).

## Wymagania

- 41 różnych projektów MakerWorld, lokalnie i prywatnie, z pochodzeniem/SHA; 34×3MF, 7×STL. Użytkownik zakończył pobieranie; STEP z fixtures.
- Rzeczywisty upload, prywatny podgląd, slicer, ilości i niezależny rachunek ceny. Ręczna ścieżka lub odmowa nie oznacza automatycznej wyceny ani obsłużonej geometrii.
- Obrazy/przykłady, desktop/mobile, oba motywy i działające interakcje z [mapy kontrolek](../e2e-controls.md): konfigurator, zakup/refundy, dostawa, klient/admin, MFA, chat i rzeczywiste emaile.
- Zachować wygląd i interakcje Piotra; minimalny Rust/proste API/KISS. Nie usuwać kontroli kwot, własności ani zasobów dla zielonego raportu.
- Po etapie review Sol xhigh: poprawność, bezpieczeństwo, kwoty, zasoby, błędy, współbieżność, dowody, prostota, utrzymanie, wygląd. Naprawiać konkretne problemy; nie wymuszać ocen ani deklarować nieudowodnionego 10/10.

## Checkpoint

| Etap | Stan i dowód |
| --- | --- |
| C1 Źródła | 41 unikalnych projektów/hashów, ignorowane oryginały i manifest |
| C2 API/wycena | `corpus41-large-current.json`: 41 scenariuszy PASS / 599,7 s na produkcie df8760d; 24 ceny ze slicera z pełnym niezależnym rachunkiem, 12 ręcznych, 4 wadliwe pliki, 1 odmowa zasobów. 36 prywatnych GLB; brak skipped/flaky. Profil large tylko izolowany opt-in |
| C2 Kontrola ręcznej ścieżki | `corpus12-manual-geometry.json`: 12 PASS / 68,7 s po wzmocnieniu testu; surowe pozycje GLB uzasadniają wymiary/powód i wykluczają błędną ręczną wycenę części mieszczącej się w P1P. Nie przypisujemy tej nowej kontroli wcześniejszemu przebiegowi 41 |
| C3 Obrazy/UI | UI41: 37 PASS / 4 FAIL przez błąd zakresu zmiennej w teście; poprawiony zakres 4 PASS w osobnym przebiegu. 36 modeli × 4 warianty = 144 ujęcia obejrzane na 6 zestawieniach; 5 odmów sprawdzonych. Źródło pozostałych 37 scenariuszy identyczne; nie jest to pojedynczy czysty przebieg 41 |
| C4 Konta/zakup | Core 45 PASS / 1 SKIP, duże usuwanie osobno 1 PASS. Rzeczywisty auth/email/manualny zakup/refund/chat: 2 PASS; pełny sandbox: 9 PASS; rejestracja hasło/passkey po 1 PASS; niezależny odczyt płatności/3 zwrotów Stripe: 1 PASS. Osobne zakresy i raporty, bez ich sumowania w jeden przebieg |
| C5 Zamknięcie | Otwarte: złożony zakup gościa → konto/claim, wznowienie istniejącej płatności po opuszczeniu strony, mapa kontrolek i zwykły runtime localhost. Oceny etapów nie są oceną całego celu |

Większe modele: [plan i ograniczenia](large-models.md). Wdrożone: bezstratny kompaktowy/spatial GLB, jawny podgląd uproszczony modelu 19 z pełnym STL, chronione usuwanie modeli oraz wcześniejsza odmowa zbyt dużego zespołu 41. Profil large nie został włączony w .env użytkownika. Pełna semantyczna poprawność modelu 41 nie jest potwierdzona.

## Następne działania

1. Obrazy korpusu sprawdzone w powyższym zakresie. Małe rozproszone zespoły wymagają zoomu, szczególnie w ciemnym motywie; bez twierdzenia o zgodności kontrastu lub fizycznym telefonie. Dalsza mapa kontrolek nie jest zastąpiona samym korpusem.
2. Core sprawdzony na oddzielnej bazie w opisanym zakresie; stare raporty zachowane, bieżący ma unikalną nazwę core-current. Oddzielne duże usuwanie również przeszło. Nie powtarzać całego zestawu bez nowego powodu.
3. Rzeczywiste auth/email/zakup/panele/dostawa i rejestracja mają nowe dowody w raporcie. Uzupełnić wykryte luki złożone: opłacony zakup gościa → dobrowolne konto/claim/późniejszy login oraz RetryPayment → ten sam PaymentIntent. Bez ponawiania całego sandboxa bez powodu.
4. Uzupełnić mapę wymaganie → test/dowód → ograniczenie. Zwykły localhost:18430 nadal obsługuje main f5bdb5e i starszy proces API z 3 listopada (nowy endpoint limitów 404), a raporty dotyczą gałęzi roboczej i izolacji. Przed aktualizacją runtime sprawdzić istniejącą bazę/konfigurację i zabezpieczyć dane; nie nadpisywać zmian użytkownika w głównym checkoutcie. Nie deklarować każdego stanu/SVG lub fizycznego telefonu na podstawie viewportu.

## Komendy

```sh
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-large-current python frontend/tests/run-integration.py --large-models corpus.spec.ts
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-ui-current python frontend/tests/run-integration.py --large-models corpus-ui.spec.ts
python frontend/tests/run-all.py --group core --no-build
```

Runtime i runner wymagają lokalnej konfiguracji zgodnie z backend/README.md; sekrety pozostają poza raportami. Starsze checkpointy są w historii Git df8760d i w raporcie; zachowano prywatne wyniki i modele.
