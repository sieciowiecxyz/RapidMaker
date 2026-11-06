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
| C5 Zamknięcie | Zakup gościa → konto/claim i historia przeszły nowy E2E po poprawce powiązań pozycji/pliku/wyceny. RetryPayment po opuszczeniu płatności ma nowy osobny dowód. Nadal otwarta: mapa kontrolek. Zwykły localhost zaktualizowany osobno 6 listopada; dowód upload/podgląd/usunięcie poniżej. Dwie konfiguracje jednego uploadu: osobny rzeczywisty Stripe/client/admin E2E PASS; utworzenie drugiej pozycji używa jawnego szwu lokalnego szkicu, bo UI nie ma przycisku duplikacji |

Większe modele: [plan i ograniczenia](large-models.md). Wdrożone: bezstratny kompaktowy/spatial GLB, jawny podgląd uproszczony modelu 19 z pełnym STL, chronione usuwanie modeli oraz wcześniejsza odmowa zbyt dużego zespołu 41. Profil large uruchomiono na localhost 6 listopada tylko w środowisku procesu; .env pozostaje bez zmian. Pełna semantyczna poprawność modelu 41 nie jest potwierdzona.

## Następne działania

1. Obrazy korpusu sprawdzone w powyższym zakresie. Małe rozproszone zespoły wymagają zoomu, szczególnie w ciemnym motywie; bez twierdzenia o zgodności kontrastu lub fizycznym telefonie. Dalsza mapa kontrolek nie jest zastąpiona samym korpusem.
2. Core sprawdzony na oddzielnej bazie w opisanym zakresie; stare raporty zachowane, bieżący ma unikalną nazwę core-current. Oddzielne duże usuwanie również przeszło. Nie powtarzać całego zestawu bez nowego powodu.
3. Rzeczywiste auth/email/zakup/panele/dostawa i rejestracja mają nowe dowody w raporcie. Zakup gościa → konto/claim/późniejszy login ma już nowy dowód. RetryPayment → ten sam PaymentIntent potwierdzono osobnym testem payment-resume-provider-fixed. Bez ponawiania całego sandboxa bez powodu.
4. Uzupełnić mapę wymaganie → test/dowód → ograniczenie. Zwykły localhost:18430 nadal obsługuje main f5bdb5e i starszy proces API z 3 listopada (nowy endpoint limitów 404), a raporty dotyczą gałęzi roboczej i izolacji. Przed aktualizacją runtime sprawdzić istniejącą bazę/konfigurację i zabezpieczyć dane; nie nadpisywać zmian użytkownika w głównym checkoutcie. Nie deklarować każdego stanu/SVG lub fizycznego telefonu na podstawie viewportu.

## Komendy

```sh
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-large-current python frontend/tests/run-integration.py --large-models corpus.spec.ts
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-ui-current python frontend/tests/run-integration.py --large-models corpus-ui.spec.ts
python frontend/tests/run-all.py --group core --no-build
```

Runtime i runner wymagają lokalnej konfiguracji zgodnie z backend/README.md; sekrety pozostają poza raportami. Starsze checkpointy są w historii Git df8760d i w raporcie; zachowano prywatne wyniki i modele.

Bieżąca poprawka kontraktu: zamówienie zapisuje line_id (pozycja), model_id (upload), quote_id (konfiguracja), bez zmiany cen i lokalnych kluczy checkoutu. Nowy guest-claim dodany do sandboxa. Dowody po poprawce: guest-claim-fixed-final 1 PASS/22,9 s, multi-model pricing 1 PASS/5,0 s, ręczne zamówienie/panele/Stripe 1 PASS/39,2 s, 22 izolowane DB/API PASS. Nie przypisujemy tych zmian wcześniejszemu pełnemu core/sandboxowi; zakres dobrano do zmiany.


Wznowienie płatności: test odtworzył brak formularza po odrzuconej karcie i opuszczeniu strony (`payment-resume-current.json`, 1 FAIL). Frontend rozpoznaje teraz faktyczne stany PaymentIntent wymagające ponowienia; nie proponuje płatności opłaconej/przetwarzanej/anulowanej. `payment-resume-provider-fixed.json`: 1 PASS / 18,1 s, prawdziwy Stripe test, ten sam PI/zamówienie/kwota, jedno utworzenie, opłacenie po wznowieniu, ponowne odczyty, odmowa obcemu gościowi. Dwa niezależne GET Stripe potwierdziły succeeded, PLN, kwotę pobraną równą serwerowej i brak zwrotów. Test dodany do sandboxa; wcześniejsze 9 PASS nie obejmowało go. Build/check:integration/format PASS; scoped Sol xhigh bez blokera, bez oceny całego celu. Zwykły localhost nadal bez aktualizacji.


Aktualizacja lokalnego runtime 6 listopada: frontend 18430 z tej gałęzi, API 18432/large. Kopia bazy/plików/poprzedniej binarki przed migracjami 22–23; główny checkout i .env bez edycji. Rzeczywisty formularz przyjął model 8 o rozmiarze 78,4 MB, wygenerował prywatny podgląd i pozwolił usunąć upload (204). Obraz obejrzany. Szczegóły, rollback początkowego checku backupu, pierwszy nieudany harness i współdzielony target: [large-models.md](large-models.md). To nie nowy pełny przebieg core/sandboxa ani zakończenie całego celu. Starsze wzmianki o nieaktualnym localhost opisują stan sprzed tego uruchomienia.


Dwie konfiguracje jednego uploadu: `same-upload-configs-final.json` 1 PASS / 19,9 s, bez skipped/flaky. Jeden prawdziwy upload; dwie pozycje/quote ID, 2 czarne standardowe i 3 białe fine/wzmocnione, rzeczywisty slicer, niezależny rachunek oraz podział każdej kwoty i jednego przygotowania. Stripe niezależnie potwierdził pobranie 4698 gr PLN. Oba panele zachowują właściwe parametry, kwoty i pliki po reloadzie. Brak przycisku duplikacji: test duplikuje tylko lokalny wiersz szkicu, dalsze ustawienia/checkout są rzeczywiste; nie jest to w pełni dostępna użytkownikowi operacja powielenia. Dodano do sandboxa. Wcześniejsze cztery błędy harnessu zachowane; pierwsze udane rerun4 osobno, final po uzupełnieniu kontroli przypisania kwot. Build/typecheck/format PASS; scoped Sol xhigh potwierdził zamknięcie uwagi o kwotach. Bez zmian produktu i stylów.
