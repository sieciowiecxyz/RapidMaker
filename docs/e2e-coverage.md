# E2E — wymagania i dowody

Przeczytaj gdy: oceniasz pokrycie celu lub wybierasz test po zmianie.
Opis: faktyczne przebiegi i granice; szczegóły w [raporcie](corpus-report.md).

Raporty JSON są prywatne w `frontend/tests/artifacts/`. Nazwy poniżej pomijają `.json`. To osobne przebiegi, nie jeden wspólny wynik. Aktualny spis źródeł: 445 kontrolek w 44 plikach, [mapa](e2e-controls.md). Powtarzalny szablon nie oznacza sprawdzenia każdej możliwej kombinacji.

| Wymaganie | Test i wykonany dowód | Granica |
| --- | --- | --- |
| Różne modele i nazwy | `corpus.spec`: aktualny corpus-api-current-69bec69,41PASS/0FAIL/0SKIP;24 ceny slicera z pełnym rachunkiem,12 geometrycznie uzasadnionych ręcznych,4wadliwe,1limit;391hashów zgodnych | 36 podglądów; odmowa nie jest obsługą modelu. STEP dodatkowo w models |
| Podgląd, obrazy i Piotr UI | `corpus-ui`: aktualny corpus41-ui-authoritative-40e8a5d-20261107T004449Z —41PASS/0FAIL/0SKIP,36renderów+5jawnych odmów;144 unikalne ujęcia obejrzane,437hashów zgodnych. `public-visual`, `populated-visual`, `showcase`, `configurator` w core-current | Jeden aktualny przebieg UI; ceny tylko9/20/29,1440/390 i motywy; nie fizyczny telefon ani formalny audyt kontrastu |
| Ceny, ilości, przygotowanie raz | `pricing` w core-current: rzeczywiste 1/10/1000 sztuk, partie, jednostki, zaokrąglenia, rachunek fixture, zmienione/obce quote ID; order-identity-pricing po poprawce identyfikatorów | Jawne stawki testowe, bez dowodu rzeczywistych kosztów firmy lub globalnego optimum układania |
| Jedna siatka, różne konfiguracje | `same-upload-configs-final` 1 PASS: dwie wyceny/ilości/kolory/parametry, przypisanie kwot, Stripe, oba panele/pliki/reload | Aktualizacja: duplicate-variant-same-upload1PASS używa przycisku „Powiel wariant”; osobny core1PASS potwierdza niezależność, reload i last-reference cleanup |
| Wszystkie oferowane opcje | `configurator` w core-current: pięć rodzin, paleta, jakość/wytrzymałość, zaawansowane, viewer, nawigacja i kontrolowane awarie sieci | [Mapa opcji](e2e-configurator-inventory.md); wymagania operatora mają ręczną ścieżkę |
| Rejestracja i logowanie | auth-journey-current 2 PASS; registration-password-current i registration-passkey-current po 1 PASS; `security` w core-current: błędne dane, rotacja, recovery, hasło/passkey/TOTP | Wirtualny passkey Chromium, nie fizyczne urządzenie; testy email tylko void@sieciowiec.xyz |
| Zakup, adres i dostawa | sandbox-current 9 PASS: checkout/shipping-checkout/delivery; prawdziwe Stripe/Furgonetka, paczkomat/kurier/pickup, adres/faktura/walidacja | Sandbox, nie produkcyjne ceny lub fizyczna dostawa |
| Wznowienie płatności | payment-resume-provider-fixed 1 PASS: odrzucona karta, opuszczenie/powrót, ten sam PI/kwota, paid po reloadzie, obcy gość odrzucony; niezależny Stripe GET | Jeden rzeczywisty scenariusz odzyskania, nie każda metoda/redirect Stripe |
| Gość → dobrowolne konto | guest-claim-fixed-final 1 PASS: płatność, link konta, login/MFA, trwałe wyłączne przypisanie, pliki/historia, ponowienie i obce konto | Istniejące konto; rejestracje nowych kont sprawdzone osobno |
| Klient/admin, chat i produkcja | `panels` w sandbox-current; order-identity-manual-compat po poprawce tożsamości; oba panele również same-upload-configs-final | [Mapa paneli](e2e-panels-inventory.md); celowo wyłączone kontrolki nie są funkcjami biznesowymi |
| Ręczna oferta, powiadomienia i email → chat | `journey` w auth-journey-current: zgłoszenie, admin cena, rzeczywisty iCloud, płatność, refund i odpowiedź email widoczna raz w chat | Odbiór osobisty; część wiadomości admina wysłana API, nie każdy krok przez UI |
| Zwroty i zgodność kwot | stripe-provider-proof-current 1 PASS oraz order-identity-manual-compat: 2500 gr pobrane, zwroty 500/1500/500, dokładne ID/suma; role i nadmierny zwrot odrzucone w panels | Niezależne odczyty Stripe test; nie rozliczenie bankowe |
| Nadanie i podjazd | `logistics` w sandbox-current: nadanie, PDF, tracking, anulowanie/przekazanie, podjazd DHL | Dawny carrierConnectionError DPD nie oznacza awarii DHL ani gwarancji każdego przewoźnika |
| Prywatność, retencja, równoległość | models/concurrency/boundaries/security/abandonment w core/sandbox; identity-storage-current 22 PASS; final-large-removal-78d5802 1 PASS; eight-clients-final 1 PASS | Równoległość ograniczona testami, nie szeroki load test/pentest lub dowód braku wszelkich luk |
| Operacyjne błędy uploadu | `upload-failures-final`: 2 PASS we wspólnej bazie (retencja + nowy scenariusz); ENOTDIR 500 i odrzucenie insertu po konwersji 503, brak pozostałości, oryginalne pliki bez zmian, recovery 201 | Rzeczywiste filesystem/DB fault injection tylko w izolacji; nie ENOSPC, awaria procesu lub UI. Nowy spec w core, pełnego zestawu nie powtarzano |
| Zawieszony upload i pobieranie | `files::tests` 10 PASS; upload-timeout-http-backend-final 2 PASS (408 po 60,005 s i recovery); private-transfer-http-final 3 PASS (w tym admin mesh i odmowa klientowi) | Osobne przebiegi po poprawce, nie pełny zestaw. RSS +516 KiB dla czterech interleaved strumieni 200 MiB to warstwa unit, nie HTTP benchmark |
| Trwałość uploadu | `upload-commit-durability-current` 1 PASS: przed INSERT błąd sprząta pliki, deferred błąd przy COMMIT zachowuje komplet, baseline SHA niezmienione i recovery 201 | Znany rollback klasyfikowany konserwatywnie, nie odtworzony utracony ACK lub power loss. Rodzic fsync sprawdzony w kodzie; sweeper dodany osobnym etapem poniżej |
| Retencja niezarejestrowanych katalogów | orphan-gc-storage 23 PASS (w tym nowy realny PG/filesystem test): stary orphan usunięty, świeży/zarejestrowany/zadanie/hold chronione, locki/query error/fair cursor/root inode/symlinki; upload regression 1 PASS | Lokalne blokady cooperating writers; bez live root rotation. Okresowo po 24 h, restart resetuje kursor, częściowy błąd może opóźnić retry |
| Faktyczny localhost | Profil large, proxy 96 MiB; smoke-fixed modelu 8: 78,4 MB → prywatny GLB i widoczny canvas, DELETE 204; kopia przed migracjami | Ten smoke nie powtarza cen/płatności/email. Wspólny target, profil tylko w środowisku procesu; [szczegóły](plans/large-models.md) |

## Aktualność i pozostałe ograniczenia

Aktualne kontrakty UI na c8d8514:54PASS/0FAIL/0SKIP/0flaky,12s (contracts-c8d8514/current-contracts.json). Mocki API/wire fixtures, nie rzeczywiste operacje backendu/providerów; Sol xhigh potwierdził zakres.

Nowszy sandbox1ae9ed6:12PASS/1FAIL testu profile-drift (błędne założenie pustej wspólnej bazy), zero provider-blocked. Po zawężeniu liczników do własnego checkoutu celowane „zakup → profile-drift”2PASS/18s. Nie jeden czysty13-scenariuszowy przebieg; pełny nieudany raport zachowany.

Nowsza wspólna regresja na3519cb9: storage25PASS po poprawce etykiety runnera (pierwsza próba24PASS/1harnessFAIL); core49PASS/0FAIL/0flaky/1SKIP,392,4s. Pominięty tylko opt-in large removal. Źródła/binarium/dist niezmienione podczas core; Sol xhigh zweryfikował raporty. Pominięty large removal uruchomiono osobno na7961feb:1PASS/0SKIP,16,7s (large-removal-current-7961feb), bez zmiany wyniku core. Nie powtarzano sandbox/email/registrations/contracts. Szczegóły i prywatne nazwy raportów w corpus-report.md.

Końcowy zestaw na **78d5802**: storage 22 PASS, core 45 PASS/1 SKIP, sandbox 12 PASS, email 2 PASS, rejestracja hasło 1 PASS, rejestracja passkey 1 PASS, contracts 53 PASS. Łącznie **136 PASS/0 FAIL/0 flaky/1 SKIP**. Wrapper zakończył się kodem 2 przez opcjonalny test large pominięty przy standardzie; ten sam scenariusz uruchomiony osobno na tej samej wersji z large dał **1 PASS**, bez skip. To oddzielny przebieg, nie zmiana wyniku wrappera na zielony. Brak adnotacji provider-blocked.

Prywatna kopia raportów: final-suite-78d5802/. Przed startem zapisano czysty HEAD i 312 hashów kodu/binarki; wszystkie niezmienione po zestawie i osobnym large. Komenda: `python frontend/tests/run-all.py --group all --no-build`; build/check były już wykonane dla tej samej wersji produktu. Siedem grup używa osobnych baz/scenariuszy, nie jednego wspólnego zamówienia. Wcześniejsze wyniki z tabeli zachowują opisany zakres; nowy zestaw nie powtarza całego korpusu.

Pozostają granice: nieprzetworzone wadliwe/ponadlimitowe źródła, fizyczny telefon/passkey, pełna macierz kombinacji, formalny kontrast, szeroki load test/pentest oraz rentowność na rzeczywistych kosztach firmy. Nie są to dowody nowych błędów. Osiem izolowanych klientów API sprawdzono osobno: 16 równoczesnych żądań → 8 prawdziwych zadań, poprawne ceny i odmowy obcych zasobów. To mała kohorta, nie osiem przeglądarek ani test przepustowości. Scoped review nie potwierdza 10/10 całego produktu.

Runner ma wspólny Chromium Playwright w preflight i testach; własny executable wymaga jawnej zmiennej. Po poprawce przeszły sprawdzenie obu ścieżek wyboru, start przeglądarki i pełny zestaw powyżej. Z ostatnich 153 zrzutów core obejrzano ponownie mobilny jasny widok klienta oraz ciemny desktop admina; cały korpus miał wcześniejszy osobny przegląd 144 ujęć.


`eight-clients-final` (po pełnym zestawie, zmieniony tylko test): 1 PASS / 3,8 s; przyjęcie 16 żądań 90 ms, wszystkie wyniki po 2611 ms, katalog HTTP 200 w czterech próbkach 13/8/8/8 ms, aktywne zadania 1/1/1/0. Uploady były sekwencyjne; współbieżność dotyczy wycen i odczytów. Maksimum jest próbkowane, nie ciągłe; brak pomiaru RSS lub produkcyjnego SLA.
