# Architektura Rapidmaker

## Zasady

Jeden backend Rust i PostgreSQL. Małe konkretne funkcje, jawne SQL, minimum zależności i warstw. `app → api → moduły`; logika obliczeń nie zna HTTP. Nie przenosimy całego kodu do routera. Zewnętrzne dane są niezaufane; granice uprawnień, kwot i zasobów pozostają obowiązkowe w POC.

## Faktyczna mapa

- `backend/crates/app`: start serwera na localhost:18432, start trybu workera geometrii.
- `api`: wspólny router, lokalna ochrona Host/Origin, upload, katalog, wycena slicerem i checkout sandboxów. `cart.rs` sprawdza snapshoty i składa koszt zamówienia. `checkout.rs` wciąż zawiera adaptery Stripe/Furgonetki. `docs.rs` udostępnia `/api` i `/api/routes`; spis metod/ścieżek generuje build.rs z AST rejestracji tras.
- `storage`: SQLx/PostgreSQL; katalog/wyceny/modele oraz `users`, `auth_challenges`, `sessions`, `auth_limits`, `orders`, `order_messages`, `email_outbox`, `passkeys`, `passkey_challenges`, `admin_bootstrap`, `slicer_jobs`, `order_file_holds`, `order_files`. Zmiana zamówienia i odpowiadający email w jednej transakcji. Import zachowuje konfigurację operatora, wyceny pozostają snapshotami.
- `slicer`: Bambu Studio w osobnym procesie bubblewrap/prlimit; P1P hardened 0,4 mm, Textured PEI, standard 0,20 mm; STL po bezpiecznej konwersji z STL/STEP/3MF; profile materiałów z zaufanego katalogu, oddzielne dopuszczenie operatora. Materiały podporowe nie są osobną ofertą. Nasze presety z `data/bambu`, rozwiązywane `inherits`/`include`. Bez sieci i sekretów, RAM 4 GiB, CPU 60 s, timeout 65 s, limit pliku 128 MiB, jedno zadanie naraz. Odczyt rzeczywistego czasu i zużycia filamentu. Slicer układa kopie na płytach; wynik pełnej płyty jest powtarzany, reszta mierzona osobno (maks. dwa przebiegi/512 kopii w układaniu). Kontrolowane draft/standard/fine, 2–8 ścian i 5–100% wypełnienia; automatyczne podpory z zaufanego presetu są wliczone.
- `pricing`: całkowitoliczbowe koszty netto w groszach, energia, maszyna, obsługa płyt, przygotowanie raz, rezerwa strat, marża, minimum i VAT. Stawki wymagają jawnej konfiguracji. Wejście to zmierzone sumy partii i liczba płyt, bez ponownego mnożenia przez ilość. Plan układania jest wykonalny, bez gwarancji globalnego optimum.
- `geometry`: analiza STL/STEP/3MF i GLB. Worker bez sieci/sekretów, 30 s/2 GiB. STEP ograniczony do pojedynczej części w mm. Masa z analizy jest oszacowaniem pełnej bryły, nie wynikiem slicera.
- `packing`: algorytm układania obwiedni z zapasem, 6 obrotów, katalog kartonów; do 2000 sztuk/30 paczek. Nie obiecuje minimum paczek. Furgonetka sandbox waliduje/wycenia usługi i punkty; aplikacja nie nadaje przesyłek.
- `auth`: CSPRNG/HMAC i webauthn-rs, bez HTTP/SQL. API auth/passkeys obsługuje email, sesje, CSRF i WebAuthn UV Required. Admin ma rolę DB + MFA, bez Bearer/starego cookie. Lokalne bootstrap/recovery są w CLI. Production wymaga skonfigurowanego HTTPS Origin/proxy; frontend jeszcze niepodłączony.
- `notifications`: iCloud SMTP (`lettre`/Tokio/rustls), tekstowe wiadomości, disabled/test/live. `api/email.rs` uruchamia jeden worker outboxa, 10 prób/min, lease/CAS/retry i kontrola preferencji. App kończy go po SIGINT/SIGTERM z limitem 25 s. Kody nie trafiają do outboxa. Lokalny `--mail-test` nie wymaga DB/HTTP. Szczegóły: [email](email.md).
- Zamówienia/rozmowy: `api/orders.rs` i `storage/orders.rs`, kontrola własności w SQL, chronione trasy operatora. Potwierdzenie płatności tylko z odpowiedzi Stripe sprawdzonej przez backend. Własność modeli/wycen/zamówień może przejść z gościa na konto, atomowo i bez nadpisywania innych właścicieli. Dedykowane crate’y `orders`, `payments`, `shipping`, `observability` pozostają rezerwą, bez osobnych implementacji.

## Nowa wycena a istniejący checkout

`POST /api/models`: STL/STEP/3MF → prywatny oryginał, izolowana kanoniczna siatka STL i prywatny podgląd GLB bez ustawień/G-code. `POST /api/quotes/estimate` przyjmuje ID własnego modelu, materiał, kolor i opcje → trwałe zadanie PostgreSQL (202) → worker slicera → koszt i atomowy snapshot. `GET /api/quotes/jobs/{id}` zwraca status/wynik właścicielowi. Katalog: `GET /api/catalog/materials`. Deduplikacja w DB obejmuje model, ustawienia, profile, kolor i koszty; bez cache pomiarów w RAM. Brak ceny, magazynu, zatwierdzenia albo poprawnego wyniku nie daje automatycznej oferty.

Pojedynczy snapshot nie otwiera płatności. Checkout przyjmuje jego ID, sprawdza właściciela, termin (10 min), ilość/model, aktualny cennik, koszt zakupu, magazyn i wersje profili. Wspólne płyty, wybór jakości i konwersja STEP/3MF działają w nowym endpointcie; STEP nadal wymaga pojedynczej części w mm. Oryginał, siatka i GLB pozostają prywatne przez 24 h, także po restarcie. Właściciela określa losowe cookie HttpOnly/SameSite; w DB zapisujemy jego hash. Metadane i pobranie wymagają tej sesji. Limity: 20 modeli/256 MiB na właściciela, 1000/8 GiB globalnie; obejmują siatki i GLB. Wygasłe pliki sprzątane przy uploadzie i co minutę przez workera; aktywne zadania chronią pliki, znacznik usuwania blokuje nowe zadania. `MODEL_STORAGE_DIR` wskazuje katalog 0700, pliki mają 0600; domyślnie `~/.local/state/rapidmaker/models`. Trwała kolejka działa: `api/slice_worker.rs`, `storage/jobs.rs`; lease/CAS, retry, jeden aktywny slicer globalnie. Szczegóły i granice: [kolejka](slicer-queue.md).

`POST /api/checkout/quote` przyjmuje części `{id,quantity,analysis_id,quote_id}`, dostawę i kontakt. Sumuje koszty różnych modeli; przygotowanie, rezerwa, marża, minimum i VAT naliczane raz. Pozycje i przygotowanie sumują się dokładnie w groszach. Każdy model ma własne partie; brak wspólnego układania różnych modeli na jednej płycie. Pakowanie używa serwerowych wymiarów i ostrożnej masy na sztukę ze zużycia filamentu (z podporami/purge), nie masy pełnej bryły ani danych przeglądarki. Opakowanie i dostawa osobno.

Oferty i sesje Stripe pozostają w ograniczonym RAM; restart je traci. Płatność/resume/status wymagają cookie właściciela. Przed otwarciem płatności backend ponownie sprawdza snapshoty; Stripe dostaje wyłącznie ustaloną kwotę. Ręczna wycena nie ma ofert płatnych. Przed Stripe trwały hold zabezpiecza własne modele z niezmiennych snapshotów; niepewna płatność i realizacja blokują sprzątanie. `completed` ustawia 90 dni retencji. Powiązania/parametry zostają po usunięciu plików, pobieranie jest przez autoryzowane endpointy zamówienia. Zamówienia i referencje płatności są w PostgreSQL; własny endpoint zamówienia może sprawdzić płatność po restarcie. Produkcyjne webhooki pozostają planem.

## Frontend i dane

`frontend/`: Svelte 5/TypeScript/Vite, Tailwind 4, GSAP, Manrope/Oxanium i limonkowe akcenty. `App.svelte` ładuje `storefront/Storefront.svelte` dla `/` i `/wycena`; `Workspace.svelte` dla checkoutu, konta, admina i wyceny zaawansowanej. STL: natywny WebGL i parser w workerze przeglądarki. STL/STEP/3MF mają backendowy GLB przez chroniony `/api/models/{id}/preview.glb`; pełny viewer GLB/Three.js na froncie pozostaje planem. GLB nie zastępuje oryginału ani siatki dla produkcji.

Konfigurator i checkout używają prawdziwego katalogu i wyniku slicera. Koszyk jest lokalnym szkicem, kwoty należą do serwera; zmiana parametrów unieważnia ofertę. Frontend kont i chatu pozostaje mockiem; panel admina wymaga migracji logowania oraz dodania WebAuthn/CSRF do adaptera. Backend nie przyjmuje już starego tokena ani operator-cookie.

Katalog Bambu pochodzi z lokalnego Studio, wersja 02.08.00.06: 49 materiałów/314 kolorów. Ceny presetów mają niepotwierdzoną walutę/VAT; ceny zakupu netto PLN, magazyn i dopuszczenie operatora pozostają osobne. Sama obecność profilu P1P nie kwalifikuje sprzętu.

## Uruchomienie i granice

[Backend: konfiguracja i kontrakt API](../backend/README.md). Lokalny PostgreSQL 16 na localhost:18434; API:18432, Vite:18430. Jedna `.env`, SOPS i hook przed commitem. Brak Dockerfile/Taskfile/Kubernetes i publicznego wdrożenia. Działają SMTP, klient auth i trwałe zamówienia/rozmowy. IMAP, publiczne wdrożenie oraz logi/metryki Grafany pozostają planem. Kolejność i dowody w [PLAN.md](PLAN.md) oraz [MILESTONES.md](MILESTONES.md).

## Auth

[Auth](auth.md): email klienta, passkey admina, trwałe sesje, tryb HTTPS i własność zasobów przez konto są zaimplementowane. UI i publiczny reverse proxy TLS pozostają do konfiguracji.
