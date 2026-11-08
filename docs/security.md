# Bezpieczeństwo — stan 2026-10-24

Audyt 2026-11-05 na `8bd6fcc`: Cargo **0 podatności,1 ostrzeżenie o utrzymaniu** — [RUSTSEC-2026-0441, pin-utils](https://rustsec.org/advisories/RUSTSEC-2026-0441.html), pośrednio przez async-imap0.12.0. To nie zgłoszona luka wykonania kodu; brak nowszej wydanej wersji async-imap usuwającej ten pakiet. Nie wyciszano zgłoszenia ani nie dodano forka. npm:3 wpisy low opisujące tę samą zależność deepmerge/pluginy Svelte ([GHSA-gcqx-w48m-w2h4](https://github.com/advisories/GHSA-gcqx-w48m-w2h4)), bez dostępnej poprawki według audytu. Raporty prywatne: `frontend/tests/artifacts/dependency-audit/`. Nie jest to certyfikat bezpieczeństwa aplikacji ani systemowych bibliotek C++.

## Zależności i poprawki

- WebAuthn 0.5.3→0.5.5: poprawiona walidacja domen według [GHSA-22w3-693w-x895](https://github.com/advisories/GHSA-22w3-693w-x895). Jawnie wyłączono subdomeny. Poprzednia konfiguracja też ich nie włączała; nie używano podatnego wariantu. To zgłoszenie GitHuba nie pojawiało się w raporcie RustSec.
- SQLx 0.8→0.9 z PostgreSQL/Tokio/rustls-ring; zachowane migracje w binarce. Nowy podział features usuwa opcjonalne RSA z lockfile. W testach rollbacku używamy literalnego SQL zamiast interpolowanego DDL; nie obchodzimy nowej kontroli `SqlSafeStr`.
- Truck, ruststep, `cgmath` i `proc-macro-error` usunięte. STEP obsługuje systemowy Open CASCADE 7.9 przez mały most CXX; OBJ parser `tobj` bez odczytu zewnętrznych materiałów. STL/3MF zachowują dotychczasowe parsery, w tym zaktualizowany `lib3mf` i bezpieczne wersje XML/ZIP.
- Native STEP: wejście ze strumienia pamięci, wyłącznie pojedyncza bryła i jawne mm; limity ekstrakcji trójkątów/wierzchołków, tłumaczenie wyjątków, poprawne transformacje i kierunek ścian. Konwersja nadal działa w osobnym procesie bubblewrap/prlimit bez sieci/sekretów/zapisu plików. OCCT nie zapisuje diagnostyki do protokołu JSON.
- Lockfile L7: 419→365 pakietów. Nie ma już RSA, cgmath, proc-macro-error ani Truck. Nowe biblioteki: CXX/CXX-build oraz tobj; OCCT jest zależnością systemową. [Wymagania budowania](../backend/README.md#start).
- Poprzedni etap: ścisłe, niejednoznaczne cookie próby logowania jest odrzucane; katalog roboczy od razu ma 0700. CSV cytuje/escapuje pola i poprzedza potencjalne formuły tabulatorem wewnątrz cytowanego pola. To format do oglądania w arkuszu; ponowny import/zapis wymaga ostrożności. [Ograniczenia CSV według OWASP](https://community.owasp.org/attacks/CSV_Injection).

## Uprawnienia — rzeczywisty router i PostgreSQL

Test `backend/crates/api/tests/access.rs` używa dwóch kont klientów, osobnego gościa oraz sesji administratora bez/z MFA. Role i stan sesji są fixture'ami DB; ten test nie zastępuje testu samej ceremonii WebAuthn.

| Granica | Potwierdzone zachowanie |
|---|---|
| Model, oryginał i GLB | właściciel 200; drugi klient/gość 404; bez sesji 401 |
| Zamówienia, rozmowy i pliki | listy nie zawierają cudzych zamówień; odczyt i zapis cudzych zasobów 404 |
| Plik innego zamówienia | podmiana modelu przy własnym zamówieniu nie daje pliku; mesh produkcyjny tylko dla admina |
| Operacje admina | klient i admin przed MFA 403; potwierdzony admin ma dostęp; refund niedozwolonej roli nie trafia do DB |
| CSRF i Origin | brak tokena lub obcy Origin blokuje zapis, również adminowi |
| Claim gościa | wymaga konta, CSRF i oryginalnego cookie; po claim stary gość traci dostęp |
| Sesje | wygasła lub odwołana przez wyłączenie konta daje 401; błędna sesja konta nie przełącza na gościa |

**Wykryty i poprawiony błąd:** admin po kodzie email, ale przed passkey, mógł używać części ścieżek klienta do własnych zamówień oraz claim gościa. Wspólne `orders::scope` wymaga teraz MFA także dla tych operacji; claim korzysta z tej samej kontroli. Test najpierw odtworzył błędne 200 zamiast 403, po poprawce przechodzi. Nie stwierdzono odczytu cudzych danych w sprawdzonych scenariuszach.

## Powtórzenie testu dostępu

Przygotuj osobną bazę `rapidmaker_test_access_*` i pusty katalog tymczasowy `rapidmaker-access-*` w systemowym temp, z prawami 0700. Ustaw `ACCESS_TEST_DATABASE_URL` i `DATABASE_URL` na tę samą bazę, `AUTH_MODE=localhost`, `AUTH_ORIGIN=http://localhost:18430`, jednorazowy `AUTH_CODE_KEY` (32 losowe bajty hex), `MODEL_STORAGE_DIR` na ten katalog, `MAIL_MODE=disabled`, `SLICER_WORKER_MODE=external`. Usuń z procesu konfigurację Stripe, Furgonetki, IMAP i metryk. Nie wczytuj całego produkcyjnego `.env`.

```sh
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api --test access -- --ignored
```

Test sprawdza nazwę bazy/katalogu i konfigurację przed utworzeniem fixtures. Po zakończeniu usuń wyłącznie własną bazę/katalog testowy. Nie wysyła emaili, nie wywołuje Stripe ani Furgonetki.

## Dowody i ograniczenia

- PASS: 17 testów geometrii, 24 jednostkowe API/storage/auth, macierz uprawnień HTTP na izolowanej bazie oraz 2 testy PostgreSQL (kody logowania, atomowy rollback statusu/outboxa). Pozostałych ignorowanych testów PG nie uruchamiano.
- PASS: fmt, Clippy zmienionych pakietów/all-targets `-D warnings`, locked build. GCC emituje `maybe-uninitialized` w wygenerowanym konstruktorze wektora CXX; ostrzeżenie pozostaje widoczne, nie jest tłumione.
- PASS: rzeczywisty worker w read-only sandboxie dla STL/STEP/3MF, kanoniczny STL/GLB i JSON; działający API po migracji SQLx: OpenAPI/STL/STEP 200, admin bez sesji 401, 3MF path traversal 422.
- Lokalne raporty: `/tmp/rapidmaker-access-{before-fix,tests}.log`, `/tmp/rapidmaker-security2-{audit-final.json,clippy.log,unit.log,pg.log,build.log,smoke.json}` oraz `/tmp/rapidmaker-{geometry-occt,webauthn}*.log`. Baza testowa i pliki fixtures zostały usunięte po testach.
- Status GitHuba przy pierwszym pushu L7 wciąż pokazywał wcześniejsze alerty lz4_flex/WebAuthn; kod/lockfile usuwa lub aktualizuje wskazane wersje. Nie zamykano alertów ręcznym wyjątkiem ani nie zakładano odświeżenia indeksu.
- Nadal potrzebne przed produkcją: wdrożenie HTTPS/proxy, podłączenie autoryzacji frontendu i utrzymanie aktualnych bibliotek systemowych OCCT. To ograniczony przegląd, bez pełnego pentestu i testów atakujących zewnętrzne usługi.

## Nagłówki lokalnego frontendu

Vite dev i preview ustawiają CSP `frame-ancestors 'none'; object-src 'none'; base-uri 'self'`, `X-Frame-Options: DENY` i `X-Content-Type-Options: nosniff`. Aplikacja nie może być osadzana w ramce; własne iframe Stripe/Maps, workery i skrypty nie dostały nowej polityki źródeł.

Nagłówki nie są częścią plików `dist`. Przyszły hosting musi wystawić je sam; nie jest to konfiguracja wdrożenia produkcyjnego. Test headless sprawdza cztery dokumenty HTML i blokadę cross-origin iframe przez łączną politykę CSP/XFO; nie dowodzi uprzywilejowanego ataku na localhost.

## Pamięć tymczasowa workerów

Aktualny kod uruchamia geometrię i Bambu z `/dev` tylko do odczytu oraz osobnym, zapisywalnym `/dev/shm` o limicie 128 MiB. `/dev/null` i `/dev/urandom` działają. Slicer zachowuje osobne 128 MiB w `/tmp`.

Natywny probe korzysta z konstruktorów produkcyjnych poleceń; zastępuje tylko końcowy program zaufanym testem jednego bajtu. Sprawdza RO urządzeń, rozmiar i zapis w shared memory. Nie wyczerpuje zasobów ani nie wykonuje exploita z modelu. Wcześniej tmpfs `/dev` oferował około 45,9 GiB zapisu; nadal ma taki rozmiar, lecz zwykłe zapisy kończą się EROFS.

AS i CPU pozostają limitami pojedynczego procesu. Ta zmiana nie zapewnia łącznego limitu RAM/CPU potomków ani nowego limitu zawartości `/out` slicera. Sprawdzono profile geometrii standard/large i reprezentatywne cięcie Bambu. Zmianę uruchomiono w testach izolowanych; zwykły lokalny serwer zachował poprzednią binarkę.

Regresje wymagają Linux/bubblewrap/Bambu i są domyślnie ignored. Jawne uruchomienie: `cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api -p rapidmaker-slicer native_device_mounts_are_readonly_with_bounded_writable_shm -- --ignored --nocapture`.


## Anulowanie slicera

Natywna regresja `cancelled_plate_kills_descendants_and_removes_workspace` bierze produkcyjny prefiks sandboxa, zastępując wyłącznie Bambu zaufanym Pythonem z jednym potomkiem. Po anulowaniu future oba procesy zwalniają blokady w ciągu 2 s, a `Workspace` usuwa katalog. Jawnie uruchomiony test PASS; nie jest to dowód odporności na awarię hosta ani pomiar rzeczywistego Bambu.

```sh
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-slicer cancelled_plate_kills_descendants_and_removes_workspace -- --ignored --nocapture
```

Nadal brak łącznych limitów RAM/CPU/liczby potomków oraz liczby plików/inodów. Per-process rlimits i timeout nie zastępują tych limitów. Bajty `/out` ograniczono osobno poniżej; nie jest to łączny limit pamięci sandboxa.


## Pliki wyjściowe slicera

`/out` to tmpfs maks.512 MiB. Host udostępnia wyłącznie prywatny `result.json` (create_new/0600); G-code i pliki postprocessingu znikają po zamknięciu sandboxa. Istniejący FSIZE128 MiB ogranicza pojedynczy plik, w tym eksportowany JSON; parser nadal przyjmuje maks.1 MiB. Dane `/out` wraz z eksportem mają więc górną granicę640 MiB. To nie obejmuje osobnych `/tmp` i `/dev/shm` po128 MiB, metadanych/inodów ani pamięci procesów.

Dobór wynika z pomiaru: większa płyta korpusu010 miała45,5 MB końcowego wyjścia, lecz136,4 MB próbkowanego szczytu;128 MiB spowodowało błąd eksportu,256 MiB przeszło. Produkcyjne512 MiB sprawdzono na czterech płytach cube1/10 i model0101/6. Inne większe zadanie może przekroczyć limit i otrzymać odmowę; nie sprawdzono całego korpusu na tym limicie. Natywna regresja `native_output_tmpfs_exports_only_result_including_missing_and_failed_runs` sprawdza limit i izolację hosta oraz poprawny/brakujący/błędny wynik. Dowody: `frontend/tests/artifacts/slicer-output-20261107/`.

Walidacja 3MF: normalizator może wcześniej odrzucić potwierdzony kanoniczny root z trzecim wystąpieniem krawędzi. Ograniczona mapa524288wpisów; niejednoznaczność/limit wyłącza wskazówkę, pozostałe modele nadal waliduje SDK. Części pomocnicze i alternatywny root nie dostają tej dodatkowej odmowy. Model37exit67 skrócono20,9→~1,9s bez podnoszenia budżetów; to odmowa wadliwej geometrii, nie jej obsługa/naprawa.


## Zagnieżdżone przestrzenie użytkownika

Geometria i slicer wymagają jawnego `--unshare-user --disable-userns` po `--unshare-all`. Stare prefiksy z zerowymi capabilities i NoNewPrivs nadal pozwalały na nowe user/mount namespace, dające możliwość tworzenia kolejnych tmpfs. Kontrolowany `unshare --user --map-root-user --mount true` działał przed poprawką; po niej odmawia w geometrii standard/large i slicerze. Nie zmieniono ustawień hosta.

Bubblewrap musi obsługiwać oba argumenty, a system dopuszczać początkową przestrzeń użytkownika; brak wsparcia kończy się odmową, bez słabszego fallbacku. Natywne probes wymagają także `/usr/bin/unshare`. To blokada tworzenia kolejnych user namespaces, nie kwota inodów ani łączny limit RAM/CPU/PID. Istniejące tmpfs mają nadal około12mln inodów; Bubblewrap0.13.1 nie ma opcji `nr_inodes`. Rzeczywisty twardy limit wymaga osobnego rozwiązania mount/deployment — polling i RLIMIT_NOFILE go nie zastępują.


## 2026-11-08 — koszt auth i prywatność logów

Ograniczony przegląd źródeł nie wykazał nowej potwierdzonej luki. Limity długości i admission IP/email/global poprzedzają Argon2; dwa sloty są trzymane wewnątrz spawn_blocking również po anulowaniu HTTP. Ustawienie hasła ma osobną kontrolę admission. Weryfikacja parametrów hasha, dummy hash nieznanego konta i limity/zużycie challenge pozostają. Nie jest to test odporności całego API/DB na zalew połączeń.

Logi HTTP używają szablonu trasy i serwerowego UUID, bez surowego URI/body/headers. Filtr targetów i panic hook ograniczają przypadkowe wypisanie sekretów; MailError nie zawiera tekstu dostawcy. Dwa istniejące testy wykonane ponownie: `logging::tests::logs_are_correlated_json_without_request_or_response_secrets` i `json_filter_and_panic_hook_protect_process_output` — po1PASS. Dowody: `frontend/tests/artifacts/security-auth-cost-20261108/`. Reviewer niezależnie sprawdził źródła. Bez patcha, nowych zależności, wywołań dostawców lub zmian normalnego serwera; nie obejmuje wszystkich gałęzi błędów ani pełnego pentestu.


## Naprawione w kodzie: retencja wygasłych wycen

Problem odtworzono na rzeczywistych metodach Storage: wycena gościa pozostawała po usunięciu modelu i zadania. To trwała retencja poza limitem zadań, nie dowód szybkiego DoS lub ujawnienia danych. Baseline: `frontend/tests/artifacts/security-public-quota-20261108/result.json`.

Istniejący cleanup modeli (minutowy oraz przy uploadzie) uruchamia osobną transakcję GC: do 100 wygasłych, niepowiązanych wycen. Zachowuje zadania, wszystkie `order_files`, historyczne `orders.lines` oraz aktywne/locked drafty w obu formatach referencji. Martwy expired-unlocked draft nie chroni wyceny. Zapis referencji draftu i GC mają ten sam queue lock; ponowne sprawdzenie istnienia wycen w transakcji blokuje zapis po ich usunięciu. Historycznych cen nie przepisujemy.

Migracja `0024_quote_retention.sql` dodaje indeks expiry, indeks FK i trzy GIN dopasowane do zapytań; legacy expiry bez liczbowej wartości pozostaje. Lock timeout 100 ms i statement timeout 1 s ustawiane przed blokadą pozwalają pominąć sweep, a cleanup modeli działa dalej. Partia 100 nie jest limitem całej bazy ani gwarancją postępu przy stałych timeoutach; protected business records nadal rosną zgodnie z retencją historii.

Pięć celowanych PG testów przeszło przed końcową korektą expired-unlocked; po niej ponowiono zmienioną regresję — 1 PASS. Obejmuje orphan/live/draft/history/legacy/job, stale draft writes, wyścig, 101 rekordów w partiach 100+1 i timeout blokady. Osobno 10 unit PASS; fmt/diff PASS. EXPLAIN potwierdza dopasowanie indeksów, nie wydajność produkcyjnej bazy. Dowody i końcowe hashe: `security-public-quota-20261108/final-hashes.json`. Reviewer zaakceptował zamrożony patch. Normalny serwer i jego schemat nie były aktualizowane; migracje wykonano wyłącznie na usuniętych już bazach testowych.

Post-GC smoke rzeczywistego API na `b467e1b`/binarka `85c24cb5`: upload STL i Bambu, `POST /api/checkout/drafts` 201, dwie aktualizacje PATCH 200, dwie oferty pickup 200 oraz GET 200. Wersje1–5, oferta2997gr, zapisane referencje wejścia/wyniku zgodne z prawdziwą wyceną; PATCH unieważnia wcześniejszą ofertę. Migracja24 sprawdzona tylko na jednorazowej bazie. Hashy źródeł/binarki/dist bez zmian, API/DB/root usunięte, normalnyAPI nietknięty. Dowody: `security-quote-gc-api-20261108/result.json` i routes.json; Solxhigh potwierdził. To pozytywna zgodność HTTP, nie nowy atak stale-write (osobny PG), provider/payment/email lub test skali. Początkowa błędna konfiguracja localhost-origin próby została zachowana, poprawiony fixture przeszedł.


## 2026-11-08 — aktualny audyt zależności

Cargo audit: 0 podatności, informacyjne [RUSTSEC-2026-0441](https://rustsec.org/advisories/RUSTSEC-2026-0441.html) o utrzymaniu `pin-utils 0.1.1` przez `async-imap 0.12.0`. Ta ścieżka jest kompilowana w aplikacji, choć IMAP domyślnie disabled. Świeża baza RustSec: `309392b18f6b17022f82c701f300433126901068`. Drzewo normal/build zawiera OpenSSL przez WebAuthn, nie zawiera RSA/sqlx-mysql; wpis w Cargo.lock sam nie dowodzi kompilowania modułu.

npm audit: 3 wpisy LOW, jeden źródłowy [GHSA-gcqx-w48m-w2h4](https://github.com/advisories/GHSA-gcqx-w48m-w2h4) dla `deepmerge 4.3.1`, `fixAvailable:false`; dwa pluginy Svelte dziedziczą ostrzeżenie. Wszystkie oznaczone dev, plugin używa merge konfiguracji kompilatora. Nie znaleziono ścieżki od danych klienta; `npm audit --omit=dev` daje 0, co nie jest dowodem bezpieczeństwa całego bundla. Nie wdrożono bezcelowej aktualizacji major/override ani wyciszenia ostrzeżenia.

Surowe raporty, kody wyjścia, drzewo i niezmienione sumy manifestów/locków: `frontend/tests/artifacts/security-dependencies-20261108/`. Sol xhigh potwierdził klasyfikację; dokładna deklaracja najnowszej wersji pluginu nie jest podstawą wniosku. Audyt nie obejmuje bibliotek systemowych lub wszystkich podatności aplikacji. Bez zmian kodu/zależności, dostawców i usług.
