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


## 2026-11-08 — defensywny przegląd email → chat

Nie potwierdzono nowej luki w sprawdzonym kodzie. Limit 256 KiB jest przed parserem; po odczycie odrzucane są załączniki, ponad32części i głębokość>8. Sprawdzenie liczby części poprzedza lokalną rekurencję. IMAP ma1slot/30s/20UIDna partię, kontrolę RFC822.SIZE, częściowe BODY.PEEK i8MiB budżetu odczytu. From jest tylko nagłówkiem — dostęp opiera się na podpisanej HMAC referencji oraz aktualnym sprawdzeniu uczestnika, roli, własności, wygaśnięcia i revocation w storage. Dedup i limit10wiadomości/order/min pozostają.

Istniejące offline `replies::tests`: **2 PASS**, artefakty `security-inbound-mail-20261108/`. Główny agent i Solxhigh przejrzeli źródła. Subagent audytowy został zablokowany przez filtr narzędzia; kontynuowano jedynie defensywny odczyt kodu i istniejące testy, bez nowych payloadów, IMAP/SMTP lub prób penetracyjnych. Brak zmian produktu; nie jest to dowód bezpieczeństwa wszystkich gałęzi parsera ani kryptograficzne uwierzytelnienie From.


## 2026-11-08 — zmiana konta w otwartym checkout

Potwierdzono wyciek UI: po logowaniu B w drugiej karcie inline formularz nadal pokazywał kontakt i uwagi A. Po zatwierdzeniu `prepareCheckout` kopiowało uwagi kontaktowe do wspólnego `model.notes` w localStorage; samo zamknięcie formularza nie wystarczało. Nie wykazano dostępu do cudzych zamówień w backendzie.

Poprawka używa istniejącego sygnału auth: zamyka inline checkout, usuwa prywatny orderDraft i blokuje spóźnione kontynuacje przez authEpoch. Uwagi kontaktowe nie są już kopiowane do wspólnego koszyka; pozostają w istniejącym szkicu konta i są przekazywane do ręcznej wyceny przez props/request body. Prawdziwe wymagania modeli i model cart są zachowane. Bez focus watcher, nowej struktury storage, zależności lub zmian stylu. Sześć plików produktu: netto18linii, zamiast nowej warstwy prywatności.

Regresje headless z kontrolowanym HTTP: przed fixem FAIL na pozostawionym formularzu, częściowa poprawka FAIL na uwagach w globalnym cart. Końcowo5/5PASS, build/svelte-check0errors/0warningsPASS, diffPASS; Solxhigh zaakceptował zamrożone źródła. Test używa prawdziwego UI logowania w drugiej karcie i produkcyjnego sygnału auth, sprawdza zatwierdzoną ręczną wycenę/body z oryginalnymi wymaganiami, dane B po ponownym otwarciu oraz reload `/zamowienie` bez uwag A. Dowody `security-checkout-identity-20261108/final-tests.json` i final-source-manifest.json. To nie jest nowy dowód backendu/cookies lub dostawców.

Granice: stare przedfixowe uwagi już wymieszane w model.notes nie są automatycznie usuwane — nie da się ich odróżnić od oryginalnych wymagań modelu. Asynchroniczne edycje współdzielonego model cart nadal nie mają pełnej gwarancji serializacji między kartami; poprawka zamyka przenoszenie nowych prywatnych uwag kontaktowych. Normalne usługi bez restartu, testy wyłącznie headless.

Uzupełnienie rzeczywistym backendem na `d008cfa`: `security-checkout-identity-live-20261108/report.json` — **1 PASS**,0FAIL/SKIP/flaky,13,16s. Bez mockowania HTTP: realne hasło/TOTP/cookies A→B w drugiej karcie i produkcyjny sygnał auth; formularz A znika, B ma własne dane także po reloadzie, oryginalne wymagania modelu zostają. Niezależna sesja A składa ręczną wycenę przez UI; API200, GET zamówienia i SQL potwierdzają dokładne fulfillment.notes oraz awaiting_quote. Uploadcube201, ścieżka manualna przez wymaganie modelu — bez dowodu slicera. Svelte/buildPASS, źródła/binarka/dist zgodne z provenance, migracja24 tylko w prywatnejDB. Brak zewnętrznych żądań/providerów/SMTP, zasoby testowe usunięte, normalnyAPIbezrestartu. Solxhigh potwierdził. To jeden scenariusz, bez testu zawieszonych akcji lub wszystkich wyścigów kart.


## 2026-11-08 — fałszywy sukces wylogowania podczas rotacji

Potwierdzony lokalnym API/PG wyścig: zmiana zabezpieczeń rotowałaS1→S2, a równoczesny logout usuwał0wierszyS1 i zwracał200; S2 nadal działała. Poprawka sprawdza rows_affected: zero daje401 ze stałym komunikatem przed budową odpowiedzi/cookie, jeden nadal200 i czyszczenie cookie. UI może zgłosić błąd zamiast fałszywego sukcesu. Netto14linii produktu w2plikach, bez zależności lub zmiany schematu.

Końcowy kontrolowany test rzeczywistego API: rotacja200, starylogout401 bezSet-Cookie, S2/me200; zwykłe ponowienie logoutS2 daje200, potem/me401 i0sesji. Wcześniejszy200 zachowany jako baseline; osobno kontrola logout-all. PGtest1PASS i2lokalnetestycookiePASS (dotychczasowyPGresendignored), build/fmt/diffPASS. Dowody i końcowe hashe: `security-session-logout-20261108/final-hashes.json`, after-result.json. Solxhigh zaakceptował.

Zakres: prawidłowe zgłoszenie nieudanego wylogowania, nie unieważnianie rodziny sesji. Zero usunięć może oznaczać też równoczesne powtórzenie poprawnego logout; nie dowodzi istnienia następcy. Brak dowodu kolejności cookie w przeglądarce; test był HTTP/DB. Normalny serwer nietknięty, prywatne zasoby usunięte, bez providerów. Pełna kontrola rodziny rotacji pozostaje osobnym zagadnieniem.


## 2026-11-08 — atomowe wylogowanie rodziny sesji

Dalsza poprawka zamyka odtworzony wyścig dla obserwowanej rodziny. Migracja0025 dodaje prywatne family_id i indeks; security/MFA/passkey rotacje dziedziczą rodzinę, nowe primary logowanie tworzy osobną. Bootstrap zachowuje rodzinę mimo triggera usuwającego starą sesję. API przekazuje tożsamość odczytaną przed rotacją; logout i logout-all biorą istniejącą blokadę auth, potem świeży DELETE w READ COMMITTED. Brak nowych zależności, cookie-formatu lub publicznego pola; netto41linii produktu+SQL.

Cztery kontrolowane rzeczywiste API/PG próby PASS: factor-first/log-out-second unieważniaS2, niezależnaS3działa; pierwotny TOTP-disable race unieważniaS2; logout-first odrzuca starą rotację, S3działa; logout-all po rotacji unieważniaS2iS3. Nowy niezależny primary-login po logout działa poza starą rodziną. Osobno3PGauth/credentials/passkeysPASS,2testycookiePASS (istniejącyPGresendNOTRUN), build/fmt/diffPASS. Realny headless virtualWebAuthn sprawdza UVbootstrap/trigger i nowyroot/loginfinish/MFA; nie fizycznyklucz. Migracja24→25 zachowuje dwa stare aktywne tokeny, oddzielne rodziny i nie ujawnia pola wJSON.

Solxhigh potwierdził zamrożone hashe źródeł i raportów: `security-session-family-20261108/final-hashes.json`, plan `plans/session-family-logout.md`. Wszystkie prywatne procesy/bazy/katalogi sprzątnięte, normalneAPI/schemat bezzmian. Granice: brak odzyskania już usuniętej lineage sprzed migracji; późniejsze niezależne logowanie nie należy do odwołanej rodziny; żądanie rozpoczynające się dopiero po zniknięciu staregoSID może dostać401. Nie jest to globalna gwarancja wszystkich przeglądarek lub wdrożenie produkcyjne.


## 2026-11-08 — faktyczne granice odczytu modeli

Nie potwierdzono brakującego limitu odczytu 3MF/STL. Gate archiwum poprzedza SDK i ogranicza wpisy/ścieżki/nagłówki oraz rozpakowane rozmiary96/512MiB. W używanym zip9.0.5 zwykły ZipArchive ma size_unknown=false; Crc32Reader czyta najwyżej zadeklarowany rozmiar+1 i odrzuca przekroczenie. Samo `read_to_string` w geometry_package nie jest więc dowodem nieograniczonej dekompresji. Central directory jest jednak parsowany przed limitem512wpisów — ten etap chronią rozmiar wejścia i limity procesu, nie późniejszy licznik.

Axum ogranicza rzeczywiste ramki body32/96MiB, z admission przed ekstrakcją i timeoutem60s; worker ma AS2GiB/CPU25s/wall30s. Cztery istniejące testy lokalne: granice3MF,ZIP64,limitySTL,uploadbody — po1PASS. Dowody `security-model-read-bounds-20261108/result.json`; Solxhigh niezależnie sprawdził źródła/bibliotekę. Bez patcha, nowych plików wejściowych lub ruchu sieciowego. Nie jest to formalny dowód szczytowej pamięci normalizacji/SDK, łącznych zasobów potomków lub inodów.


## 2026-11-08 — warianty jednego uploadu w ręcznej wycenie

Potwierdzono błąd kontraktu: widoczny „Powiel wariant” zachowuje ID uploadu, a model-only guard odrzucał drugi wariant nawet z innym materiałem. API i storage porównują teraz całe kanoniczne części po normalizacji ilości; każda nowa pozycja ma osobne `line_id=quote_id`. Limity/własność/replay/14-dniowy hold pozostają, identyczne pozycje nadal odrzucane. Netto 0 linii produktu, bez migracji/DTO/deps lub zmian stylu.

Osobno PG 1 PASS i native UI 1 PASS/12,7 s: jeden upload, dwa lokalne warianty (PLA ×1/PETG ×2), rzeczywisty POST/panel/GET, dwie tożsamości/snapshoty/aktywne holds i poprawne opcje. Kwota/oferta/płatność NULL, zero prób płatności dla zamówienia. Test 300 mm używa prechecku manual bez slicera; sprawdzono też odmowę identycznych części, odrzucenie cudzego modelu bez osieroconych wycen i ponowienie zgłoszenia. Pierwotny PG FAIL przez dodatkowy fixture przed starym GC assert zachowany; usunięto własny fixture, nie osłabiano assert. Build/fmt PASS, Sol xhigh bez blokera, źródła/binarka/dist zamrożone przez UI, własne zasoby usunięte, normalne API niezmienione. Dowody: `security-manual-multipart-20261108/implementation-result.json` i `ui-run1/manual-variants-evidence.json`. Nie sprawdzano tu edycji postępu produkcji w panelu admina lub dostawców; identyczne klony pozostają poza zakresem.

Następny krok uzupełnił postęp: istniejący test PG z kontrolowanym opłaconym rekordem zapisuje/odczytuje [1,0], zmienia tylko pierwszy wariant i odczytuje [2,0] po odrębnych line_id. Rzeczywisty admin UI nieopłaconego zgłoszenia pokazuje dwie poprawne pozycje i wyłączone liczniki; PUT z ważnym Origin/CSRF dostaje konkretny 422 za brak płatności, odczyty produkcji/zamówienia pozostają niezmienione. Osobno PG 1 PASS i UI 1 PASS/13,2 s, wyłącznie zmiany testowe; bez błędów prób. Opłacenie w PG jest syntetycznym fixture, nie Stripe; nie testowano udanej edycji opłaconego zamówienia w przeglądarce. Sol xhigh bez blokera, źródła produktu/binarka/dist i normalny API niezmienione, własne zasoby usunięte. Dowody: `security-manual-line-progress-20261108/implementation-result.json` i `ui-run1/manual-admin-unpaid-evidence.json`.

## 2026-11-08 — przerwane cięcie bez kosztownego retry

Próba wariantów na modelu korpusu 29 ujawniła exit 137 po około 69 s i automatyczne ponowienie. Large miało prawidłowe 120 s CPU/125 s czasu na płytę; przy dwóch CPU wynik pasuje do limitu CPU, ale przyczyny nie ustalono. Nowe `Stopped` obejmuje timeout płyty i zewnętrzne kody 137/152/153; bez retry, z bezpiecznym komunikatem o ręcznej wycenie. Sprawdzenie znanego `return_code=-6` nadal poprzedza klasyfikację; Busy/Failed pozostają retryable. Adapter mapuje wyłącznie odebrany terminalny stan zadania na istniejącą ścieżkę manual, bez nowych kontrolek lub zależności.

Osobne dowody: 2 unit PASS, 1 PG PASS (poprawny lease, failed/attempts=1/lease NULL/brak kolejnego claim), 1 mock HTTP UI PASS (CTA, brak ceny), build Rust/Svelte/Vite/fmt PASS. Jedna rzeczywista próba tego samego fine ×3 ponownie zakończyła slicer kodem 137: tym razem failed po jednej próbie i nadal bez retry po 7 s. Zalogowany klient zapisał prawdziwe ręczne zgłoszenie z modelem/ilością/opcjami; awaiting_quote, kwota/oferta/płatność NULL, zero prób płatności, hold pliku. GC nie uruchamiano; UI CTA i zapis API sprawdzono osobno. Sol xhigh bez blokera; prywatne zasoby usunięte, normalny API niezmieniony. Dowody: `security-slicer-stopped-20261108/extension-result.json` i `stopped-complex-native-20261108/result.json`. Wcześniejsze dwa FAIL i błędy prób kompilacji/asercji zachowano. Nie jest to automatyczna cena fine ×3, pełna macierz pięciu wariantów ani dowód rentowności.

Później domknięto jeden pełny native UI test bez mocków: hasło/TOTP → upload cienkiego STL → jedno żądanie wyceny → terminalne failed po trzech szybkich exit156 → formularz → prawdziwy POST200/panel/own GET200. DB potwierdza dokładny model/materiał/kolor/opcje, awaiting_quote, kwotę/ofertę/próbę płatności NULL, zero prób dla tego zamówienia i hold na 14 dni. To odrębna ścieżka po wyczerpaniu retry, nie klasyfikacja 137 ani diagnoza 156. Test 1 PASS/25,5 s, źródło zamrożone przed/po; Sol xhigh bez blokera. Cztery historyczne FAIL wynikały z założeń harnessu (odczyt evicted body, liczba prób, normalizacja opcji, retencja), pozostają zachowane. Dowody: `full-manual-ui-native-verified-20261108/result.json`, native-manual-evidence.json i cleanup.json. Produkt/normalny API bez zmian, własne zasoby usunięte; bez testu płatności/emaila/dostawców lub faktycznego GC.

## 2026-11-08 — ponawianie nieodwracalnych operacji

Nie znaleziono potwierdzonego duplikowania operacji lub akceptowania zmienionej treści pod tym samym kluczem w badanych ścieżkach płatności, zwrotów, nadań, podjazdów, anulowań i publikacji ofert. Zapis submitting/creating/ordering z kontrolą lease poprzedza wywołanie przewoźnika; niepewny wynik jest uzgadniany odczytem, bez ślepego ponowienia. Awaria po tym zapisie, ale przed HTTP, może zostawić niewykonaną operację w manual_review: to ochrona przed duplikatem, nie gwarancja exactly-once ani wykonania. Dwa istniejące testy offline klasyfikacji odpowiedzi PASS; blokady/unikalność/crash windows tylko sprawdzono źródłowo, bez ponawiania PG lub dostawców. Sol xhigh bez blokera, kod/testy bez zmian. Dowody: `security-mutation-retry-20261108/result.json`, operation-inventory.json i tests.json.

## 2026-11-08 — bramki tras administracyjnych

Źródłowo sprawdzono 34 jawne operacje: 33 chronione (20 mutacji/13 odczytów) oraz publiczny GET stanu sesji, bez danych prywatnych przed auth. Montowanie routerów, warstwa authorize i jawne guardy nie wykazały brakującej roli/MFA/Origin/CSRF ani publicznego aliasu operacji. To inwentaryzacja źródeł, nie 34 testy runtime; część metod storage ufa wywołującemu. Dowody: `security-admin-routes-20261108/inventory.json` i result.json.

Usunięto nieaktualne określenie obowiązkowego passkey z komentarza, komunikatu i opisów OpenAPI: faktyczna polityka wymaga admina i pełnego MFA (hasło lub passkey + email/TOTP). Net 0 LoC, bramki bez zmian. Build/fmt i jeden istniejący headless test Swagger PASS; rzeczywisty root opisu oraz osiem opisów operacji odpowiadają źródłom. Watcher miał osiem przejściowych timeoutów przy starcie, później odczyt zakończył się poprawnie bez restartu/rerunu. Dowody: `admin-docs-native-20261108/result.json`, generated-descriptions.json i cleanup.json. Sol xhigh bez blokera; własne zasoby usunięte, normalne API niezmienione. Bez ponawiania macierzy odmów admina lub wywołań dostawców.

## 2026-11-08 — wykonalność wspólnego budżetu workerów

Jednorazowy eksperyment we własnym user scope systemd: przed pracą potwierdzono cgroup z memory.max=6GiB, pids.max=256 i cpu.max=200000/100000. Dwa istniejące testy natywne PASS: pięć poprawnych formatów geometrii (biblioteka bez API sandboxa) i rzeczywisty Bambu przez produkcyjny bwrap/AppRun, m.in. partie 1–1000. Wrapper przed exec i próbki procesów potwierdzają wspólną grupę; zaobserwowano memory.peak około206MiB i pids.peak=11. To zgodność i naliczanie zasobów, nie próba wyczerpania limitów lub ucieczki. CPUQuota ogranicza szybkość CPU, nie sumę CPU-seconds; TasksMax obejmuje wątki. Unit/grupa/procesy usunięte, pozostałe limity i normalne API niezmienione, brak zmian produktu. Dowody: `security-worker-process-budget-20261108/experiment-result.json`, readback-before.json, child-placement.jsonl i unit-after.txt; Sol xhigh potwierdził zakres.

Ochrona agregatowa nadal niewdrożona. Przed zmianą trzeba dobrać budżet cięższych/równoległych zadań, potwierdzać wejście do grupy bez cichego fallbacku i sprawdzić cleanup potomków po timeout/drop. Dla KISS rozważyć limit całego serwisu przed osobnym lifecycle każdej pracy. Zachować bwrap/prlimit; DBus i sekrety poza sandboxem. Sam filtr zakazujący fork zepsułby aktualny launcher AppRun, który używa potoku; brak runtime dowodu zgodności takiego filtra.

Dalsze dwa ograniczone eksperymenty PASS: własny TasksMax=4 przy rodzicu i trzech zwykłych potomkach odrzuca kolejne utworzenie procesu przez EAGAIN, pids.events.max rośnie 0→1. W osobnej grupie jeden potomek z setsid przeżywa SIGKILL rodzica; zatrzymanie dokładnie własnego unit usuwa potomka i grupę. Obie grupy/procesy posprzątane, normalne API/limity przodków/źródła niezmienione. Dowody: `security-worker-process-budget-20261108/pid-cancel-6d3f09e6b7cee99c/experiment-result.json`; Sol xhigh potwierdził. To twardy limit zadań i zachowanie systemu, nie integracja z Tokio, próba MemoryMax lub dowód odporności na ucieczkę. Wspólny unit API ograniczyłby kod, ale awaria jednej pracy mogłaby dotknąć cały serwis; per-job wymaga dodatkowej obsługi anulowania i własności grupy.

Następnie prywatne API jako transient Type=exec service z KillMode=control-group i tymi samymi limitami wykonało upload201, GLB200 i wycenę202→succeeded. MainPID/wykonywalny plik/grupa/limity potwierdzone przed pracą; nie uzyskano osobnego śladu krótkich native potomków. Osobna mała service automatycznie usunęła potomka setsid po SIGKILL MainPID, bez ręcznego stopu. Pierwotny raport FAIL zachowany: niejednoznaczny check zwykłego bind portu; później brak listenera i poprawny bind z SO_REUSEADDR oraz brak własnych PID/unit/DB/katalogów potwierdzają cleanup. TIME_WAIT jest możliwą przyczyną, nie zachowanym bezpośrednim dowodem. Bez ponowienia eksperymentu, zmian normalnego API lub produktu. Dowody: `security-worker-process-budget-20261108/service-envelope-998d3ab018ca2da9/final-assessment.json` i oryginalny experiment-result.json. Sol xhigh potwierdził zakres; [backend README](../backend/README.md#opcjonalny-wspólny-limit-zasobów) podaje opcjonalny start bez frameworku Rust. Normalny serwer nadal bez tego limitu, brak per-job ochrony i testu wyczerpania pamięci.

## 2026-11-08 — wynik starego hasła po zmianie zabezpieczeń

Przegląd zmian hasła/TOTP/passkey/roli nie wykazał potwierdzonego nadania dostępu przez nieaktualny czynnik. Istniejący test PostgreSQL wzmocniono o faktycznie inne hasło: po jego zapisie wynik wcześniejszej weryfikacji starego hasha nie tworzy sesji ani pending SID; dalsze loginy używają nowego hasha i przechodzą TOTP/recovery. Jeden test PASS, +16 linii testu, produkt bez zmian. Prywatna baza usunięta, normalne API bez zmian, Sol xhigh bez blokera. Dowody: `security-auth-revocation-20261108/regression-result.json`, password-change-regression.json i regression-source-after.sha256. To kontrolowany wynik po commicie, nie równoległy Argon2/HTTP race ani dowód przerwania wszystkich rozpoczętych żądań.

## 2026-11-08 — kwoty koszyka i checkoutu

W badanych ścieżkach nie znaleziono obejścia ceny/ilości/własności/oferty: wyceny są serwerowymi snapshotami, koszyk sprawdza ich właściciela i aktualność, a przygotowanie płatności blokuje właściwą wersję szkicu i bierze kwotę z oferty DB. Ręczna cena wymaga admina; formularz klienta nie przyjmuje kwoty. Cztery istniejące testy offline tożsamości/reuse/formularza/overflow PASS; istniejące scenariusze manipulacji HTTP i blokad PG tylko przejrzano, bez ponownego uruchamiania. Sol xhigh bez blokera, bez zmian kodu/testów. Dowody: `security-cart-amounts-20261108/result.json`, tests.json i source-after.sha256. To nie pełny dowód przepływu płatności, wszystkich wyścigów lub rentowności; bez providerów, zmian normalnego API/DB i nowych cen.

## 2026-11-08 — Origin, Host i CSRF

Przegląd nie potwierdził obejścia. Produkcja wymaga dokładnej authority i Origin przy mutacjach; brak Origin dozwolony tylko dla GET/HEAD. Powielone Host/Origin oraz nieprawidłowy Origin są odrzucane; forwarded headers nie zastępują Host/Origin. Sesje produkcyjne mają __Host/Secure/HttpOnly/Lax; zaufany proxy jest jawny. Upload sprawdza Origin i własność, nie token CSRF — nie deklarować uniwersalnej kontroli tokenu dla każdego POST.

Dodano1test tabelaryczny polityki produkcyjnej:26wariantów/1PASS, netto60linii testu, produkt bez zmian. Wymaga MAIL_MODE=disabled (lub braku tej zmiennej); test nie wysyła emaila. Trzy wcześniejsze testy cookie/CSRF osobnoPASS. Solxhigh bez blokera; dowody `security-origin-csrf-20261108/regression-test.json` i `final-hashes.json`. Nie testowano tu TLS, przeglądarki, middleware HTTP ani konfiguracji proxy; normalny API/DB bez zmian.

## 2026-11-08 — wolne pobieranie prywatnych plików

Pięć istniejących testów ścieżek/nagłówków/odczytu PASS; brak potwierdzonego traversal lub wycieku. Osobny rzeczywisty test TCP wykazał jednak zajęcie wszystkich4slotów: cztery nieczytające połączenia dostają200, piąte503; zamknięcie przywraca200. Fixture używa produkcyjnego odczytu, zastępuje routing/autoryzację — nie dowodzi obejścia dostępu. O_NOFOLLOW zabezpiecza ostatni komponent, nie przodków wobec lokalnego aktora z prawem do prywatnego katalogu.

Poprawka: jeden producent na odczyt, kanał1×64KiB, niezależny deadline300s i anulowanie po porzuceniu odbiorcy. Plik/slot zwalniane przed EOF; przedwczesne zakończenie daje błąd body. Netto35linii produktu, bez zależności lub zmian UI/auth/nagłówków. Siedem końcowych testów PASS: bajty/nagłówki/GLB/duże pliki, cleanup, deadline300s na zatrzymanym zegarze i TCP z krótkim wspólnym deadline. TCP odzyskuje200 bez zamykania czterech starych gniazd.4×200MiB: porcje≤64KiB, zaobserwowany przyrost RSS około3MiB, nie formalny pomiar maksimum.

Solxhigh bez blokera. Dowody `security-file-download-20261108/transport-probe-result.json`, `fix-result.json`, `final-hashes.json`; pierwotny błąd kompilacji zachowany, poprawiony przed testami. Po usunięciu zbędnego testowego SO_LINGER ponowiono tylko test TCP. Normalny API/DB bez restartu lub wdrożenia. Legalny odczyt ponad5min zostanie przerwany; uprawniony klient nadal może ponawiać żądania, więc nie jest to gwarancja fairness ani całkowitej odporności na DoS. Pozostałe połączenia/bufory transportu nie podlegają kwocie tych czterech plików.

Osobno po poprawce: jeden rzeczywisty headless przebieg `--local-accounts models.spec.ts two-client-upload.spec.ts security.spec.ts`,11PASS/0FAIL/SKIP/flaky,20,04s. Potwierdza GLB200/glTF dlaSTL/STEP/3MF, widoczność viewera, obcy401/404, no-store, izolację wycen, upload429/recovery, usunięcie wygasłych plików i lokalne logowanie/TOTP/ustawienia. Nie dowodzi renderu pikseli, pobierania oryginałów/plików zamówień lub deadline300s. Prywatny przypięty API zawiera poprawkę; hashe źródeł/dist niezmienne, własnaDB/procesy/pliki usunięte, normalneAPI niezmienione. Bez providerów/emaila; Solxhigh potwierdził zakres. Dowody `download-security-native-20261108/result.json`, build-manifest.json i cleanup.json.

Oryginały: +17 linii w dwóch istniejących specach; jeden headless przebieg, 9 PASS bez błędów/pominięć/flaky. Siedem własnych pobrań zwraca dokładne bajty; pięć fixture sprawdza też nagłówki długości/attachment/octet-stream/nosniff/no-store i anonimowe 401, dwaj goście dostają 404 dla cudzego źródła. Produkt/dist/API niezmienione; zmieniły się tylko testy, w trakcie przebiegu przywrócono formatowanie obcego fragmentu bez zmiany semantyki. Prywatne zasoby usunięte, normalne API bez zmian, Sol xhigh bez blokera. Dowody: `source-download-native-20261108/result.json`, cleanup.json i final-hashes.json. Zakres nie obejmuje plików zamówień/admin mesh ani deadline 300 s.

## 2026-11-08 — granica zaufania webhooka Stripe

Przegląd źródeł nie wykazał obejścia: podpis surowego body i czas sprawdzane przed zapisem; zdarzenie inicjuje ponowne pobranie obiektu Stripe z weryfikacją ID/kwoty/PLN/metadanych/testmode. Rozliczenie blokuje próbę płatności, stan paid jest końcowy, zapis zamówienia i outbox atomowy. Dwa istniejące testy offline podpisu/korelacji PASS.

Istniejący test PostgreSQL rozszerzono o ten sam ID zdarzenia ze zmienionym typem, rodzajem obiektu, ID obiektu i payloadem: ponowienie odrzucone, lease zachowuje cztery oryginalne pola.1PASS na własnej usuniętej bazie; fmt/diffPASS, Solxhigh bez blokera. Netto17linii testu, produkt bez zmian. Dowody `security-stripe-webhook-20261108/checkout-event-final.json`, `final-hashes.json`. To test deduplikacji storage, nie HTTP/podpisu/providera ani równoległego wyścigu; normalneAPI/DB bez zmian.

## 2026-11-08 — wywołania HTTP płatności i dostawy

W sprawdzonych ścieżkach nie potwierdzono podatności: stałe hosty HTTPS, walidacja ID przed budową ścieżek i wyłączone przekierowania. Etykieta pochodzi z endpointu Furgonetki, nie z URL zwróconego przez dostawcę; dostęp wymaga admina, format a4/a6, limit8MiB i sygnatura PDF. Stripe ogranicza odpowiedź do512KiB; Furgonetka JSON do1/5MiB, błędy64KiB. Limity czasu: Stripe connect5s/request20s, Furgonetka3s/8s.

Pięć istniejących testów offline walidacji ID/URL i kontraktów żądań PASS; redirect/limity transmisji/timeouty sprawdzono źródłowo, bez symulacji transportu. Brak zmian produktu lub nowych testów; bez providerów/DB/przeglądarki. Dowody `security-provider-http-20261108/result.json`, tests.json i niezmienione hashe źródeł. Solxhigh potwierdził zakres i brak konkretnej luki. To nie dowód odporności wszystkich tras na SSRF/DoS ani działania produkcyjnych integracji.

## 2026-11-08 — odpowiedzi zamówień klienta

Przegląd źródeł nie wykazał wycieku prywatnych danych produkcji/faktury, identyfikatorów uczestników rozmowy ani plików innych klientów. Istniejący test routera/PostgreSQL sprawdzał kanarek produkcji; dodano kanarek szkicu faktury, potwierdzenie jego odczytu przez admina i brak obu pól w odpowiedzi klienta. Zmiana tylko testowa: netto5linii, bez kodu produktu/zależności.

Baseline1PASS i końcowy1PASS na osobnych usuniętych bazach; bez providerów, przeglądarki i zmian normalnego API/DB. Dowody `security-order-response-20261108/access-invoice-final.json` i `final-hashes.json`. Solxhigh potwierdził rzeczywisty rozdział serwerowy, bez blokera; zakres to sprawdzone trasy, nie pełny pentest. Aktualne pełnowierszowe SELECT szczegółu zamówienia wymaga nadal uwagi przy dodawaniu kolumn do orders.

## 2026-11-08 — prywatne artefakty a Vite dev

Potwierdzono, że lokalny dev server udostępniał własny niesekretny plik kontrolny z `frontend/tests/artifacts` przez HTTP200 mimo katalogu700/pliku600. Nie odczytywano rzeczywistych raportów lub sekretów. Poprawka `server.fs.deny` zachowuje domyślne reguły Vite (pobierane przez resolveConfig bez pliku konfiguracji/env), dodaje tests/artifacts oraz wyłącznie projektowe frontend/.codex. Nie blokuje całego nadrzędnego ~/.codex, w którym znajduje się worktree.

Końcowy istniejący localhost18430: cztery warianty direct/@fs/raw-import dają403 bez zawartości canary; root i src/main.ts dają200 z nagłówkami. Testproxy2/2PASS, build/svelte0errors/warningsPASS; NODE_ENVproduction i built-in denies zachowane. Pierwszy zbyt ogólny wzorzec powodował403 całej strony, test go wykrył i wzorzec zawężono; historyczna próba zachowana. Vite automatycznie przeładował konfigurację, bez polecenia restartu; brak baselinePID, więc nie twierdzimy że PID pozostał ten sam. Backend/DB bez zmian.

Dowody `security-artifact-serving-20261108/`, Solxhigh potwierdził source/politykę/probe. Netto13linii konfiguracji i31testu, bez nowej zależności. Zakres to Vite dev i wskazane katalogi, nie wszystkie ignorowane pliki, cross-origin exploit lub certyfikacja hostingu produkcyjnego. Preview serwuje dist; jego fs.deny nie zmieniano.
