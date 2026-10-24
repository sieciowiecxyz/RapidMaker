# Bezpieczeństwo — stan 2026-10-24

Aktualny Cargo audit: **0 podatności, 0 ostrzeżeń RustSec**, bez ignorowania zgłoszeń. To wynik bazy RustSec dla Cargo.lock, nie certyfikat bezpieczeństwa aplikacji ani systemowych bibliotek C++. Poprzedni npm audit: 0 zgłoszeń; frontend nie był zmieniany w etapie L7.

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
