# E2E — wymagania i dowody

Przeczytaj gdy: oceniasz pokrycie celu lub wybierasz test po zmianie.
Opis: mapa wykonanych przebiegów i ich granic; historia i szczegóły w [raporcie](corpus-report.md).

Prywatne raporty: `frontend/tests/artifacts/`; nazwy poniżej zwykle bez `.json`. Różne HEAD-y i przebiegi nie tworzą wspólnego PASS. [Mapa kontrolek](e2e-controls.md) to spis źródeł, nie pełna macierz kombinacji.

## Najnowsze dowody — 2026-11-07

| Zakres / raport | Wykonany wynik | Granica |
| --- | --- | --- |
| Zbiorczy `baad9d1`, `full-integrated-baad9d1-20261107/` | **179PASS/0FAIL/1opt-inSKIP/0flaky**,441hashy bez zmian; storage25/core65/privacy11/sandbox13/email3/rejestracje1+1/contracts60 | Osiem grup exit0, wrapper2 przez skip; katalog i ceny na jawnych stawkach testowych. Korpusu41 nie powtarzano. |
| Large, ta sama wersja i hashe | Osobny `large.json`: **1PASS/0SKIP**,16,5s, exit0 | Ten sam pominięty przypadek, osobny profil/run; nie sumować do180PASS pełnego zestawu. |
| Pełny zestaw `6addae4`, `full-current-6addae4-20261107/` | **170 PASS / 3 FAIL / 1 opt-in SKIP**, 0 flaky; 443 hashe niezmienione | FAIL: privacy, sandbox i email po jednym; nie zastępować wyniku celowanymi rerunami. |
| Kontrakty w pełnym `6addae4` | 56 PASS, osobna grupa `contracts` | Mocki API/wire fixtures; nie rzeczywiste operacje backendu/providerów. |
| Poprawka `a03f362`, `privacy-final-focus-isolated-20261107` | 11 PASS: same/cross-tab, remote revoke, mutacja w locie, opóźnione/me, iframe click i szybki powrót; własność/publiczny szkic zachowane | Natywny focus iframe w headless; zewnętrzny lifecycle syntetyczny. Bez dowodu natywnego BFCache/push-revoke stale widocznej karty. |
| Providerzy poprawki `a03f362` | `panels-focus-final-20261107`: 1 PASS; `journey-focus-final-20261107`: osobno 1 PASS | Panele/chat/produkcja/refundy oraz manual/email/Stripe/replay dedup; te osobne reruny nie zastępują zbiorczego wyniku. |
| Cache, `5222964`, `private-cache-access-20261107` | 1 PASS: rzeczywiste PG/router, no-store i uprawnienia prywatnych GET, me klienta/admina/anonima | Bez wczesnych Host/Origin, mutations lub dowodu BFCache; 103 GET to diagnostyka, nie 103 przypadki. |
| Worker, `63e5387`, `preview-worker-fixed-20261107/last-run.json` | 2 PASS; po 503 Worker kończony przed zwinięciem, także po ponownym otwarciu | Marker wykonania, brak osobnego raportu JSON/hashów; mock HTTP, rzeczywisty Worker; nie pomiar RSS. |
| Podgląd, `3aed38f`, `private-preview-restore-20261107/fixed.json` | 3 PASS: dwa natywne cykle WebGL restore bez resize, obrót, collapse i failed Worker | Rzeczywiste Worker/parser/GPU, mock HTTP/kanoniczny GLB fixture; nie konwersja backendu. Baseline FAIL zachowany; brak snapshotu hashów. |

## Korpus i przepływy produktu

| Wymaganie / raport | Dowód | Granica |
| --- | --- | --- |
| Modele, `corpus-api-current-69bec69` | `69bec69`: 41 PASS, 24 natywne ceny z rachunkiem, 12 ręcznych, 4 wadliwe, 1 zasobowa odmowa; 36 GLB, 391 hashów zgodnych | Model 41 nadal ponad limitem; wadliwe 14/21/32/37 poprawnie odrzucone. Ręczna oferta/odmowa nie oznacza automatycznej obsługi. STEP osobno w `models`. |
| Piotr UI, `corpus41-ui-authoritative-40e8a5d-20261107T004449Z` | `40e8a5d`: 41 PASS, 36 renderów, 144 unikalne obejrzane ujęcia, 437 hashów zgodnych | Ceny UI tylko 9/20/29, szerokości 1440/390 i motywy; [większe modele](plans/large-models.md) zachowują pełny STL. |
| Opcje i wygląd | `configurator`, `showcase`, `public-visual`, `populated-visual`; pięć rodzin, paleta, jakość/wytrzymałość, zaawansowane, viewer i błędy sieci | Zachować [kontrakt Piotra](frontend-contract.md) i [opcje](e2e-configurator-inventory.md); profile wymagające operatora mają ścieżkę ręczną. Bez fizycznego telefonu/formalnego audytu kontrastu. |
| Ceny i przygotowanie raz | `pricing`: 1/10/1000, partie, jednostki/zaokrąglenia i obce/zmienione quote; `cart-minimum-real-20261107`: 2 PASS, minimum raz, kolejność i suma przydziałów | Rzeczywisty slicer/API na jawnych stawkach testowych, bez Stripe w minimum; nie rentowność firmy ani globalne optimum układania. |
| Jeden upload, różne warianty | `same-upload-configs-final`: 1 PASS, kwoty/Stripe/oba panele; `duplicate-variant-same-upload`: przycisk „Powiel wariant”; osobny core 1 PASS | Niezależność/reload/last-reference cleanup; poprawione line_id/model_id/quote_id, nie nowy wspólny przebieg. |
| Konta/MFA | `auth-journey-current`: 2 PASS; obie rejestracje po 1 PASS; `security`: błędne dane, rotacja/recovery, hasło/passkey/TOTP | Wirtualny passkey Chromium, email testowy tylko void@sieciowiec.xyz. |
| Zakup/dostawa/PI | `sandbox-current`: 9 PASS; `payment-resume-provider-fixed`: 1 PASS, decline/leave/resume, ten sam PI/kwota, reload i obcy gość, niezależny Stripe GET | Stripe/Furgonetka sandbox, paczkomat/kurier/pickup/adres/faktura; nie produkcyjna taryfa, fizyczna dostawa lub każda metoda/redirect. |
| Gość → konto | `guest-claim-fixed-final`: 1 PASS, płatność/login/MFA, trwała wyłączna własność, pliki/historia/ponowienie/obce konto | Istniejące konto; nowe rejestracje osobno. |
| Panele/chat/email/refundy | Najnowsze provider-reruny powyżej; [mapa paneli](e2e-panels-inventory.md). `journey-email-replay-20261107`: dwa identyczne Message-ID → imported+duplicate, jeden chat/reload | Jeden sender/mailbox; admin chat przez API, bez niezależnego Stripe GET w journey. Wyłączone kontrolki nie są funkcjami biznesowymi. |
| Niezależne kwoty zwrotów | `stripe-provider-proof-current`: 1 PASS; `order-identity-manual-compat`: 2500 gr, zwroty 500/1500/500; role/nadmierny zwrot odrzucone w panels | Stripe test, nie rozliczenie bankowe. |
| Logistyka | `logistics`: nadanie/PDF/tracking/anulowanie/przekazanie i podjazd DHL | Dawny `carrierConnectionError` DPD zachowany; bez gwarancji każdego przewoźnika. |

## Błędy, trwałość i zasoby

| Zakres / raport | Dowód | Granica |
| --- | --- | --- |
| Ośmiu klientów, `eight-clients-resources-20261107` | 1 PASS: 16 estimate → 8 rzeczywistych zadań, max running 1; próbkowana suma RSS drzewa 187,2 MiB, max 3 potomków, recovery 0 potomków | Uploady sekwencyjne; równoległe wyceny/odczyty, mała kohorta API, nie osiem przeglądarek/load test/SLA/górny limit pamięci. Starszy `eight-clients-final` bez RSS pozostaje w raporcie. |
| Upload dwóch klientów, `two-client-upload-20261107b` | 1 PASS: 429 przed drugim body, responsywny katalog, oba 201 po retry, własne GLB/ceny, obce 404 | Jeden parser/upload naraz — celowy limit RAM, nie równoległe konwersje. |
| Błędy uploadu, `upload-failures-final` | 2 PASS: retencja + ENOTDIR 500/insert po konwersji 503, brak pozostałości, wcześniejszy model bez zmian, recovery 201 | Izolowane filesystem/DB fault injection; nie ENOSPC/awaria procesu/UI. |
| Timeout/streaming | `files::tests`: 10 PASS; `upload-timeout-http-backend-final`: 2 PASS, 408 po 60 s/recovery; `private-transfer-http-final`: 3 PASS, admin mesh/odmowa klientowi | 64 KiB/4 transfery; unit RSS +516 KiB dla czterech strumieni 200 MiB to nie HTTP benchmark. |
| Lease i restart | `queue-recovery-20261107-first`: 1 PASS, SIGKILL po claim, naturalne 180 s+5 s, attempts 2/jeden snapshot; `queue-restart-final`: 1 PASS, queued→nowy PID→Bambu/attempts 1 | Recovery nie dowodzi startu Bambu przed kill; cleanup osobno fault 2 PASS + queued 1 PASS. Restart graceful; bez awarii hosta/power loss. |
| COMMIT i orphan | `upload-commit-durability-current`: 1 PASS, parent fsync przed DB/ochrona niepewnego COMMIT; `orphan-gc-storage`: 23 PASS + upload regression 1 PASS | Deferred błąd to znany rollback, nie utracony ACK. GC: 24 h/64 wpisy, root flock/DB fence, zadania/holdy chronione; bez live root rotation, restart resetuje kursor, błąd może opóźnić retry. |
| Drift profilu przy płatności | PG 1 PASS + HTTP 1 PASS: hash pod blokadą katalogu, 409 bez próby płatności/zamówienia | Pliki runtime osobno kontroluje API; stare sandbox FAIL i celowane 2 PASS opisane w raporcie. |
| Prostota/zależności | Badge `e1b10c0`: 71 usuniętych linii/3 pliki (31 komponentu), 3 bezpośrednie devDependencies; clsx pozostaje przechodnio | Machete `74bc144` bez nieużywanych zależności, streaming dodał dwa użyte helpery; nie dowód prostoty całości. npm audit: 3 LOW, jedno GHSA deepmerge, bez dostępnej poprawki. |
| Lokalny runtime | Ostatni opisany smoke `7fb2a24`: upload/GLB/cena/delete; wcześniej `smoke-fixed` modelu 8: 78,4 MB, canvas/DELETE 204, profil large/proxy 96 MiB | Historyczne localhost i backup, nie obecny proces/produkcja; smoke większego modelu nie powtarzał cen/płatności/email. |

Historia `78d5802`, `3519cb9`, `bb67c8a`, wcześniejsze FAIL harnessu i opt-in large pozostają w [corpus-report](corpus-report.md), wraz ze szczegółami własności/retencji. Nie sumować tych wyników ani usuniętych linii z nowszymi. Pełna macierz, szerszy load/pentest, fizyczne urządzenia, rentowność i ogólna prostota pozostają niepotwierdzone; review nie oznacza 10/10 ani zamknięcia celu.

Uruchamianie: [plan](plans/corpus-quality.md) i [testing](testing.md). Wyłącznie headless, bez `--headed`/`--ui`/`--debug`; wspólny Chromium Playwright, inny executable tylko jawnie. Wrapper `core` obejmuje `privacy`, a `all` wszystkie grupy. Spis komend nie oznacza ich wykonania; archiwizuj raporty przed ponowieniem.
