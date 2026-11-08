Aktualny zestaw całej strony: `npm --prefix frontend run test:all`. Grupy i wymagania: [tests/README](../frontend/tests/README.md); wyniki bieżącego etapu: [E2E-REPORT.md](E2E-REPORT.md). Starsze sekcje poniżej opisują historyczne sprawdzenia i nie nadpisują aktualnego raportu.

# Szybka weryfikacja

Testy przeglądarki wyłącznie headless, bez otwierania okien. Obie konfiguracje Playwright mają jawne `headless: true`; nie używaj `--headed`, `--ui` ani `--debug`.

Testy mają pomagać dostarczać kod. Sprawdzaj zmieniony zakres najkrótszą sensowną metodą. Bez obowiązkowej Astry, testujących subagentów, wielokrotnych review i pełnych zestawów przy każdej iteracji. Zielonego wyniku nie powtarzaj bez nowej zmiany, błędu lub konkretnej wątpliwości.

- UI: build i krótki przebieg zmienionego ekranu. Przy zmianie układu obejrzyj komputer i telefon; screenshoty tylko, gdy pomagają ocenić efekt.
- Algorytm: kilka istotnych przypadków granicznych. Dla pakowania: różne części, 1/1000 sztuk, masa, gabaryt i brak danych.
- API: jedna poprawna operacja i najważniejszy błąd. Dla płatności sprawdź serwerową kwotę, ponowienie i brak fałszywego potwierdzenia; dla danych klienta — uprawnienia.
- Docs: przejrzyj zmienione linki i zgodność opisu. Audyt narzędziem tylko, gdy jest potrzebny.

Nie kasuj działających testów bez powodu. Zmieniony kontrakt wymaga dopasowania testów, a wykryty błąd — małej regresji, jeśli jest przydatna. Pełny E2E, fuzzing, pentest, skany i odtwarzanie awarii są osobnymi zadaniami według potrzeb lub przed wydaniem; nie blokują codziennego prototypowania.

## Komendy według potrzeby

- `npm --prefix frontend run build` — Svelte/TypeScript i build.
- `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium npm --prefix frontend run test:e2e -- --grep 'nazwa testu'` — wybrany scenariusz.
- `cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api -p rapidmaker-packing -p rapidmaker-pricing` — zmienione reguły API, pakowania i wyceny.
- `cargo build --locked --manifest-path backend/Cargo.toml`, `python backend/run.py` — lokalne Rust API na porcie 18432.
- `cargo fmt --manifest-path backend/Cargo.toml --all -- --check` — formatowanie Rust.
- `cargo test --locked --manifest-path backend/Cargo.toml --workspace` — API i parser, gdy wymagają tego zmiany.

## Granice obecnego demo

Frontend: `frontend/`, port 18430; preview E2E: 18431. Vite przekazuje `/api` do `backend/` na localhost:18432. API uruchom przed testami integracji. Szczegóły konfiguracji: [README frontendu](../frontend/README.md).

Testy sandboxów wymagają lokalnych sekretów: Stripe test oraz `.env` z `FURGONETKA_SANDBOX_TOKEN` i `FURGONETKA_SENDER_JSON`, uprawnienia 0600, poza Gitem. Konfigurator/checkout używają slicera i PostgreSQL; integracja wymaga osobnej bazy i jawnych stawek testowych, nie potwierdza rentowności. Materiały/cennik i sesja operatora są rzeczywiste; konta klientów, zamówienia/chat i powiadomienia korzystają z API. Ręczna oferta ma własny trwały przebieg i płatność; faktury pozostają poza zakresem. Brak tokena oznacza brak dowodu integracji, a nie poprawny darmowy przewóz. Nie twórz prawdziwych obciążeń ani nadań.

W `PLAN.md` zapisz krótko, co sprawdzono i czego nie. Nie wklejaj sekretów, modeli klientów ani dużych logów. Testy bezpieczeństwa kieruj tylko do własnego localhosta; dostawców sprawdzaj zwykłymi wywołaniami sandbox API. Publiczny deployment pozostaje poza zakresem.

## Panel operatora

Wybrana regresja formularza RGBA: z katalogu `frontend`: `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium npx playwright test operator.spec.ts`. Test stubuje granicę HTTP i sprawdza formularz; nie jest dowodem autoryzacji backendu. Pełny przebieg cookie/zapisu w PostgreSQL sprawdzono osobno na tymczasowej bazie, bez zmieniania cen normalnego środowiska.

Starsze `flows.spec.ts` obejmuje mockowe logowanie admina i nie odpowiada już serwerowej sesji ani nowemu panelowi. Wymaga migracji wraz z resztą B5; nie raportuj całego zestawu E2E jako PASS na podstawie jednej regresji.

Auth: `cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-auth -p rapidmaker-api -p rapidmaker-storage`. Ignorowane testy PG wymagają osobnej bazy: `AUTH_TEST_DATABASE_URL` oraz `TEST_DATABASE_URL` (prefiks rapidmaker_test); uruchamiaj sekwencyjnie. Dotychczasowe E2E operatora używające tokena wymagają migracji do wspólnej sesji/passkey.

## Integracja istniejących paneli

`live-panels.spec.ts` oraz `operator.spec.ts`: kontrakt HTTP interfejsu (kod email/claim, MFA przed odczytem, zamówienia/pliki/chat, preferencje, RGBA i CSRF, ponowienie zwrotu bez nowego request_id). Odpowiedzi API są podstawiane; nie jest to ponowny test rzeczywistego SMTP, passkey/Stripe/Furgonetki ani autoryzacji PostgreSQL. Dawne demo `flows.spec.ts` i `shipping.spec.ts` zastąpiono rzeczywistymi scenariuszami z [tests/README](../frontend/tests/README.md). Używaj `http://localhost:18430` dla rzeczywistego auth; port testów 18431 nie jest skonfigurowanym origin backendu.

`operations.spec.ts`: kontrakty formularzy logistyki (paczki/koszt/zgoda/book/retry/PDF/podjazd/anulowanie), częściowego zwrotu z zachowaniem ID po reloadzie i natywnego WebAuthn z wirtualnym uwierzytelniaczem. `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium npx playwright test operations.spec.ts live-panels.spec.ts operator.spec.ts` z `frontend/`. To test UI z podstawionym API, bez połączeń z dostawcami. Screenshoty fixture dostawy: `frontend/tests/artifacts/shipping-desktop.png` i `shipping-mobile.png` (ignorowane przez Git).

## Auth A1 — 2026-10-25

PASS: fmt; Clippy auth/storage/api all-targets z `-D warnings`; build Rust; 5 testów auth; izolowane testy PostgreSQL etapów, limitów, replay i passkey; macierz uprawnień routera na osobnej DB (baza i pliki usunięte). Frontend: svelte-check bez błędów/ostrzeżeń i Vite build. Wybrane 11 scenariuszy Chromium: 10 przeszło od razu, ostatni po przebudowaniu nieaktualnego dist; bez zmiany oczekiwań. Obejmują hasło+email, wirtualny passkey+TOTP, błędny OTP bez utraty formularza, brak email przy TOTP, ustawienia i brak sekretów w browser storage oraz istniejące kontrakty paneli. Wygląd `/logowanie` sprawdzony ze zrzutu.

Pełny zestaw E2E, fizyczny klucz, publiczny HTTPS i nowe doręczenie SMTP: NOT RUN. Chromium używa kontrolowanych odpowiedzi HTTP, więc nie jest dowodem całego przepływu SMTP → przeglądarka → rzeczywisty backend.

Rust: 24795 → 26219 (+1424) linii kodu w `backend/crates/**/*.rs`, łącznie z testami inline i nowymi plikami; bez komentarzy/pustych/generowanych. To pomiar łączny, nie rozdział production/test. Nowe zależności bezpośrednie: argon2 (hasła), totp-rs (TOTP), chacha20poly1305 (szyfrowanie sekretów); brak nowych zależności frontendu. Audyt zależności wykonany w tej zmianie: brak zgłoszeń w bieżącej bazie.

## Finalizacja F0–F6: rzeczywisty backend

Po buildzie backendu/frontendu: `python frontend/tests/run-integration.py`. Runner tworzy osobną DB `rapidmaker_test_browser_*` i prywatny katalog tymczasowy, uruchamia API:18436 i frontend:18437; nie dotyka normalnych uploadów. Wymaga lokalnego PostgreSQL/Podman oraz Chromium. Nie ładuje sekretów Stripe/mail/Furgonetki; bazowy zestaw testuje rzeczywiste uploady/geometrię/GLB i własność, nie sandboxy. Konfiguracja: playwright.integration.config.ts. Logi bez trace/cookies; błędy dają FAIL. DB i pliki są usuwane po zakończeniu runnera. Zestaw bazowy obejmuje też prawdziwy slicer; fixture kosztów istnieje wyłącznie w tej bazie. Wyniki sandbox/mail zapisujemy oddzielnie, bez deklaracji PASS z mocków.

Sandboxy (jawnie, tylko test): `python frontend/tests/run-integration.py --sandbox auth.spec.ts delivery.spec.ts checkout.spec.ts journey.spec.ts abandonment.spec.ts`. Runner ładuje wyłącznie potrzebne klucze dostawców z lokalnego .env, wymusza Stripe test i iCloud test do void@sieciowiec.xyz. Tworzy prywatny fixture kont przez przykład Rust `browser_fixture`, który odmawia działania poza DB rapidmaker_test_browser_*. Kody email/TOTP i klucze sesji nie są zapisywane w raportach; auth/płatność mają wyłączone trace/screenshots. Czytnik iCloud używa readonly/BODY.PEEK i ogranicza się do świeżych testowych wiadomości. Baza, fixture i uploady są kasowane przy wyjściu; nie oznacza to automatycznego usuwania historii sandboxów ani wiadomości użytkownika.

Podstawowy rzeczywisty zestaw uploadów i wyceny: 6 PASS na osobnej bazie, obejrzane screenshoty produkcyjne i poprawiony błąd ładowania CSS. Pełny `--sandbox journey.spec.ts` ponownie sprawdzony 2026-10-25: 1 PASS (2,4 min), z rzeczywistym iCloud → Stripe test → chat → iCloud → odpowiedź w UI. Szczegóły i wcześniejsze FAIL doręczeń: MILESTONES.md. Sam kod testu nie jest dowodem PASS.

Do diagnozy niezależnej od emailowego logowania: `... --sandbox --client-totp journey.spec.ts`. Tworzy testowego klienta z prawdziwym TOTP (bez obchodzenia auth); odmawia uruchomienia tak `auth.spec.ts`. Test nadal wymaga doręczenia oferty i odpowiedzi email, więc awaria iCloud pozostaje FAIL, nawet gdy płatność przejdzie.


## Dokumenty i akceptacja — 2026-10-25

PASS: Rust build/fmt; Clippy api/storage/notifications all-targets z `-D warnings`; 11 testów notifications, w tym pełna wersjonowana kopia MIME i limity. Trzy rzeczywiste regresje na osobnej, usuniętej bazie PostgreSQL: konkurencyjna rejestracja zapisuje jedną akceptację/kopię; checkout zachowuje wersję/czas i jeden kontrakt po ponowieniach zapłaty; ręczna oferta zapisuje warunki przed płatnością. Brak zmian w normalnych kontach.

PASS: svelte-check 0 błędów/ostrzeżeń i Vite build; 2 kontrakty UI `tests/e2e/legal.spec.ts`; `python frontend/tests/run-integration.py --sandbox legal.spec.ts checkout.spec.ts`: 2 PASS, rzeczywiste dokumenty/API oraz upload → slicer → checkout gościa → Stripe test paid. Stara/brakująca wersja rejestracji daje 409. Strony dokumentów nie wysyłają zapytań zewnętrznych; obejrzano desktop/mobile, brak poziomego overflow. Screenshoty w ignorowanym `frontend/tests/artifacts/integration/`.

NOT RUN: nowy test odbioru kopii prawnej przez IMAP, pełny zestaw przeglądarkowy, produkcja, audyt prawny i weryfikacja umów/transferów dostawców. Istniejące ostrzeżenie kompilatora C++ w generowanym mostku OCCT pozostaje; Clippy nie zgłosił błędów. Granice: [legal.md](legal.md).

Rust: 27865 → 28140 (+275) linii kodu w backend/crates, bez pustych/komentarzy/generowanych; łącznie z testami inline i przykładami, bez niezależnego podziału production/test. Bez nowych zależności. Nowe moduły tylko dla wspólnej treści dokumentów i publicznego endpointu; jedna migracja rozszerza istniejące konto, zamówienie i outbox.

## Motyw i zasoby publicznych widoków/paneli

Po buildzie frontendu: `python frontend/tests/run-integration.py --local-accounts public-visual.spec.ts`. Bez Stripe/iCloud/Furgonetki; sesje i MFA z lokalnego API, osobna baza. Sprawdza14 wariantów tras w dwóch motywach i szerokościach, zapisuje56 prywatnych zrzutów. Zakres i ograniczenia: [raport](corpus-report.md#spójność-motywu-i-widoków).

## Równoległe wyceny

`python frontend/tests/run-integration.py concurrency.spec.ts`:4 sesje/8 żądań, realny slicer, deduplikacja i cache, dostęp do cudzych danych, gotowy checkout i kwoty. Bez dostawców. Należy też do `run-all.py --group core`. Jest to krótka próba asynchronicznego API, nie test maksymalnego obciążenia; progi czasu mogą być wrażliwe na zajęty komputer.

## Wypełnione panele

`python frontend/tests/run-integration.py --local-accounts populated-visual.spec.ts`: rzeczywiste nieopłacone zapytanie/chat i prywatne GLB, oba motywy/szerokości,72 prywatne zrzuty; kontrola etykiety kwoty i kontrastu oraz28 kart błędnej/opublikowanej/wygasłej oferty. Publikacja jest rzeczywista, wygaszenie dotyczy własnej bazy testowej; bez uruchamiania płatności i zewnętrznego emaila. Należy do core. Rejestrację z prawdziwym iCloud uruchamia osobno `python frontend/tests/run-all.py --group registration --no-build` (dwie własne bazy; hasło/wirtualny passkey).

SIGTERM wymaga czasu na sprzątanie: przeglądarka/API do30s każde, operacja psql do15s, rodzic czeka80s. Kontrolowano ponowiony sygnał, CREATE z wstrzymanym potwierdzeniem i błąd kopii logu. To ręczne próby infrastruktury, nie kolejne testy Playwright. Raport i ograniczenia: [checkpoint](corpus-report.md#zatrzymywanie-testów).

`python frontend/tests/run-integration.py --local-accounts manual-publication.spec.ts`: zerwanie połączenia przed/po rzeczywistym zapisie oferty, identyczny retry, wersje i deduplikacja outbox oraz ukrycie danych po logout. Bez Stripe/SMTP; należy do core.

Ten sam plik zawiera pełny native UI fallback: cienki zamknięty STL → terminalny błąd rzeczywistego slicera → formularz ręcznej wyceny → POST/panel/GET/DB. Sprawdza zachowanie modelu i opcji, brak kwoty/płatności i aktywny hold; bez mocków HTTP/dostawców. Filtrowany przebieg na `a0790f2`: 1 PASS/25,5 s, `full-manual-ui-native-verified-20261108/result.json`. Cztery wcześniejsze FAIL testu zachowano; nie zmieniano produktu dla uzyskania PASS.

`public-visual.spec.ts` sprawdza też inline SVG: minimalną widoczną liczbę, geometrię i dla rozmiaru≤600 px wkład pikseli na diagnostycznym tle. Duże SVG: tylko geometria. Kontrole negatywne wykrywają grafikę pustą/ukrytą/brakującą; to nie jest porównanie pełnej zgodności wizualnej ani pomiar kontrastu. Liczniki są w adnotacjach `svg-rendering` raportu JSON.

Profil dużych modeli sprawdzamy osobno: `E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=large-profile-complete python frontend/tests/run-integration.py --large-models large-profile.spec.ts`. Runner ustawia profil tylko własnemu API z tymczasową bazą; `.env`/zwykły serwer bez zmian. To rzeczywisty upload/prywatnyGLB/kanoniczna siatka dla8/20/40 oraz pomiar slicera/niezależny rachunek20, bez dowodu renderowania dużych modeli na stronie. Zwykłe `models.spec.ts` dodaje100Continue/niekompletny body→429 i200 po rozłączeniu. Szczegóły i wcześniejsze FAIL: [plan](plans/large-models.md).

Usuwanie: `E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=model-removal-complete python frontend/tests/run-integration.py --large-models --local-accounts model-removal.spec.ts`. Scenariusze na tymczasowej bazie/storage: realny upload/fizyczne pliki/kwoty, gość/konto/Origin/CSRF, spóźniona odpowiedź i kontrolowane błędy katalogu/DELETE z prawdziwym ponowieniem. Bez flagi duży przypadek jest jawnie pomijany; normalne przypadki są w core. Ochrona kolejki/holdów i fallbacku gościa również w22 izolowanych testach `run-storage.py`. Dowody/granice: plan dużych modeli.

Ten plik sprawdza też wspólny upload dwóch ręcznych wariantów: ostatnia aktywna ochrona zachowuje trzy pliki, a wygaśnięcie obu pozwala fizycznie je usunąć z zachowaniem historii i odpowiedzią 410. Zwykłe uploady uruchamiają realny GC; daty zmieniane wyłącznie we własnych rekordach testowych. Filtrowany przebieg na `fe694bf`: 1 PASS/2,3 s, `security-manual-file-retention-20261108/implementation-result.json`. Bez retencji opłaconych zamówień lub testu zegara rzeczywistego przez 14 dni.

Sprawdzenie współdziałania: `python frontend/tests/run-integration.py --local-accounts manual-publication.spec.ts model-removal.spec.ts --grep manual`. Na `9d7793a` jeden bieg: 4 PASS, 0 FAIL/SKIP/flaky/retry, 104,7 s wraz ze startem/sprzątaniem, jedna prywatna DB/API. Dowody: `manual-flows-combined-20261108/runtime-run1/result.json` i cleanup.json; Sol xhigh potwierdził. Bez nowych zmian produktu, płatności/dostawców lub doręczania emaila.

Model29: `--large-models large-profile.spec.ts --grep 'corpus 29:'` z jawnymE2E_CORPUS_MANIFEST sprawdza wszystkie surowe bity zorientowanych trójkątów prywatnegoGLB kontra kanonicznySTL (multizbiór, także powtórzenia), pełną geometrię/owner i rzeczywistą cenę z niezależnym rachunkiem fixture. `--large-models corpus-ui.spec.ts --grep 'corpus GUI 29:'` dodaje piksele/zoom/4widoki i cenęUI. Nie używa produkcyjnych stawek ani dostawców. Granice/artefakty: plan dużych modeli.

Podgląd uproszczony: `--large-models large-profile.spec.ts --grep 'corpus 19:'` z jawnym E2E_CORPUS_MANIFEST kontroluje pełny STL/source SHA, prywatny ograniczony GLB z metadanymi, brak masy z przybliżenia i ręczną cenę ponad 500 mm. `corpus-ui.spec.ts --grep 'corpus GUI 19:'` dodatkowo sprawdza liczbę rzeczywiście renderowanych trójkątów wobec niezależnego odczytu GLB, piksele/zoom, oba motywy i dwie szerokości oraz ręczną ścieżkę po kompletnym wyborze opcji. Parser ma osobne testy fałszywych/niespójnych metadanych. Dowody i granice w planie dużych modeli.

Budżet zespołu 3MF: `--large-models large-profile.spec.ts --grep 'corpus (41|19|20):'` kontroluje odmowę modelu 41 + odzyskanie workera oraz przyjęcie dwóch dużych modeli, w tym prawdziwy slicer/cenę. `corpus-ui.spec.ts --grep 'corpus GUI 41:'` sprawdza czytelną odmowę w formularzu; nie jest dowodem obsługi modelu. Namespace/ścieżki, instancje/nieużywane części, cykle/limity metadanych i tekstowe referencje XML mają testy w geometry/three_mf_budget.rs. Raport rozróżnia odmowę od przetworzenia i ceny.

Pełny korpus: `--large-models corpus.spec.ts` z jawnym E2E_CORPUS_MANIFEST wymaga 41 unikalnych źródeł/hashów i respektuje limit 96 MiB (bez flagi 32 MiB). Znane odrzucenia są przypięte do niezależnie zdiagnozowanych źródeł; nieoczekiwane błędy nadal FAIL. Raport osobno liczy mierzone ceny, ręczne ścieżki i odmowy. Ręczna ścieżka sprawdza surowe wymiary GLB wobec limitu 500 mm/P1P; mierzone ceny wymagają pełnej niezależnej faktury fixture. `corpus-ui.spec.ts` sprawdza osobno rzeczywistą stronę, obrazy i kontrolki. Przebiegi/kategorie w corpus-report.md; API PASS nie dowodzi obrazu ani rentowności.

Po zmianach E2E uruchom `npm --prefix frontend run check:integration` (Linuxowy runner): sprawdza typy integration specs, których zwykły svelte-check nie obejmuje. Samo zebranie testów Playwright nie wykrywa błędów zakresu zmiennych. Po poprawce testu zachowaj surowy FAIL i osobny wynik ponowienia dotkniętych scenariuszy; nie łącz ich w jeden czysty raport. Wizualny raport rozróżnia istnienie zrzutu, automatyczne piksele/zoom i faktyczne obejrzenie obrazów.

Niezależny dowód Stripe: sandboxowy główny test panels.spec.ts porównuje płatność i wszystkie zwroty także z dwoma stałymi GET do Stripe, nie tylko z API aplikacji. read-test-stripe.py wymaga sk_test_, weryfikuje korelację PaymentIntent/attempt/order/refund i nigdy nie emituje client_secret ani całego payloadu. Testowe klucze pozostają lokalnie. [PaymentIntent GET](https://docs.stripe.com/api/payment_intents/retrieve), [refund list GET](https://docs.stripe.com/api/refunds/list). Rzeczywiste rejestracje uruchamiaj osobno z --sandbox --registration i E2E_REGISTRATION_METHOD=password/passkey; raporty nie zastępują złożonego testu guest checkout → konto/claim.

Zakup gościa → konto: `E2E_REPORT_NAME=guest-claim-current python frontend/tests/run-integration.py --sandbox --client-totp guest-claim.spec.ts`. Sprawdza istniejące konto/MFA po prawdziwej płatności, dodatni/idempotentny claim, pliki/GLB i historię po świeżym logowaniu oraz utratę dostępu starym cookie/obcemu użytkownikowi. W grupie sandbox runnera. Odpowiadające regresje storage dotyczą kanonicznych ID pozycji oraz ręcznych/legacy draftów; testy adapterów i produkcji rozróżniają dwie konfiguracje jednego uploadu.

Proxy uploadu (bez DB i providerów): `npm --prefix frontend run test:proxy`. Test uruchamia prawdziwy Vite na portach tymczasowych, sprawdza EPIPE dla 48/97 MiB i przekazanie upstream 413. Nie zastępuje uploadu do Axum ani testów przeglądarki.

Regresja restartu kolejki: `E2E_REPORT_NAME=queue-restart-current python frontend/tests/run-integration.py --queue-restart`. Wymaga gotowego binarium API i rzeczywistego Bambu; bez przeglądarki/builda frontendu i dostawców. Własne API startuje bez workera, zapisuje upload i queued/attempts=0, kończy się przez SIGTERM (maks.30s), po czym nowy proces z tym samym DB/storage/cookie przetwarza zadanie raz (attempts=1, jeden snapshot, pomiar/cena). Sprawdza hash oryginału i prywatny GLB (200/404); usuwa własne procesy, DB i pliki. Startup30s, HTTP5s/upload40s/estimate30s (zimny odczyt runtime), worker160s. Tryb uruchamiaj samodzielnie; JSON stats/evidence w prywatnym artifacts, domyślnie unikalna nazwa. Nie dowodzi odzyskiwania aktywnego lease ani działania UI.

Regresja aktywnego lease: `E2E_REPORT_NAME=queue-recovery-current python frontend/tests/run-integration.py --queue-recovery` (samodzielnie, bez innych trybów). Własne API pozostaje external; runner uruchamia binarium `rapidmaker-api --slicer-worker` z tym samym izolowanym DB/storage i LOG_LEVEL=info. Dopasowuje JSON stderr `slicer_started` oraz dokładne `span.job_id`, natychmiast zatrzymuje grupę SIGSTOP i potwierdza running/attempts=1, niewygasły lease i brak snapshotu. Ten event dowodzi przejęcia zadania, nie uruchomienia Bambu. Następnie SIGKILL/wait, sprawdzenie i sprzątanie wyłącznie własnych potomków (bwrap może mieć osobną sesję), nowy worker i po7s nadal ten sam aktywny lease. Test czeka na rzeczywiste180s lease +5s retry i Bambu (deadline: odczytany pozostały lease +50s, około230s od przejęcia); nie zmienia czasu/lease w SQL. Sukces wymaga attempts=2, pustego lease, jednego zgodnego snapshotu z pomiarem/ceną, tego samego cookie, hasha i GLB200/obcy404/job404 oraz responsywnego API. Już zakończona pierwsza próba oznacza FAIL. Wymagania i timeouty HTTP/startupu jak powyżej; brak przeglądarki/dostawców, bez dowodu rebootu hosta. Prywatny JSON i logi błędu0600; własne procesy/DB/pliki sprzątane także przy błędzie. Użyj unikalnego E2E_REPORT_NAME, aby zachować wcześniejsze wyniki.

Raport obu trybów kolejki jest końcowy dopiero po sprzątaniu: wymagaj exit0, `stats.unexpected=0` i `cleanup_verified=true`. Każdy błąd sprzątania ustawia FAIL i `cleanup_verified=false`; pozostałe procesy, DB i katalog nadal mają niezależne próby sprzątania. Jawnie ponownie użyta nazwa raportu usuwa poprzedni JSON przed startem, więc błąd startupu nie zachowuje starego PASS. Krótka regresja błędów sprzątania bez usług: `python frontend/tests/test_queue_cleanup.py`.

Raporty w `frontend/tests/artifacts` są prywatne. Vite dev blokuje ten katalog oraz projektowe `frontend/.codex` przez fs.deny z zachowaniem domyślnych blokad; uprawnienia700 same nie chronią przed serwerem działającym pod tym samym UID. Nie publikuj katalogu testów z produkcyjnym hostingiem.
