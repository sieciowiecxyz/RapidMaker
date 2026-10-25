# Szybka weryfikacja

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

`live-panels.spec.ts` oraz `operator.spec.ts`: kontrakt HTTP interfejsu (kod email/claim, MFA przed odczytem, zamówienia/pliki/chat, preferencje, RGBA i CSRF, ponowienie zwrotu bez nowego request_id). Odpowiedzi API są podstawiane; nie jest to ponowny test rzeczywistego SMTP, passkey/Stripe/Furgonetki ani autoryzacji PostgreSQL. Starszy `flows.spec.ts` nadal wymaga migracji. Używaj `http://localhost:18430` dla rzeczywistego auth; port testów 18431 nie jest skonfigurowanym origin backendu.

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
