# Backend Rapidmaker

Jeden Cargo workspace, jedna aplikacja. Zależności: `app → api → moduły`; bez cykli.

```text
backend/
├── Cargo.toml / Cargo.lock
├── run.py
├── data/bambu/           # katalog i zaufane presety
└── crates/
    ├── app/             # start serwera i procesu geometrii
    ├── api/             # router, HTTP, orkiestracja
    ├── storage/         # PostgreSQL/SQLx i migracje
    ├── slicer/          # izolowany Bambu Studio
    ├── pricing/         # czysta kalkulacja kosztów
    ├── geometry/        # analiza STL/STEP/3MF, fixtures
    ├── packing/         # rozmieszczenie w paczkach
    └── ...              # przyszłe orders/auth/payments/shipping/notifications/observability
```

## Start

```sh
cargo build --locked --manifest-path backend/Cargo.toml
python backend/run.py
```

API: `127.0.0.1:18432`; frontend Vite przekazuje `/api`. Launcher czyta tylko dozwolone klucze z rootowego `.env` (0600, SOPS): sandboxy Stripe/Furgonetki, `DATABASE_URL`, `PRICING_RATES_JSON`.

`DATABASE_URL` uruchamia migracje i idempotentny import katalogu. Bez tej zmiennej stary sandbox działa, nowe endpointy zwracają 503. Nieprawidłowa konfiguracja DB/cennika zatrzymuje start, bez ujawniania poświadczeń.

Lokalny PostgreSQL 16: rootless Podman, kontener i wolumen `rapidmaker-catalog-postgres`, port `127.0.0.1:18434`. Start istniejącego kontenera: `podman start rapidmaker-catalog-postgres`. Dane dostępu są prywatne, nie w README. Po sklonowaniu potrzebujesz własnego PostgreSQL i odpowiedniego `DATABASE_URL`.

Linux + bubblewrap + prlimit są wymagane dla parsera i slicera. Slicer korzysta z lokalnego `/opt/bambustudio-bin` (sprawdzono Bambu Studio 02.08.02.61). Jego zasoby są tylko do odczytu; proces nie ma sieci ani sekretów. Dostępny profil: P1P hardened steel 0,4 mm, Textured PEI, standard 0,20 mm, PLA Basic (`GFA00`) i PETG HF (`GFG02`). To konfiguracja POC do kalibracji, nie certyfikacja sprzętu.

## Nowe API

- `GET /api/catalog/materials`: katalog PostgreSQL, kolory i dostępność. Bez kosztów zakupu i surowych presetów.
- `POST /api/quotes/estimate`: body = surowy STL do 32 MiB. Nagłówki: `X-Model-Filename`, `X-Material-Id`, `X-Color-Code`, `X-Quantity` (1–1000). Wynik: pomiar jednej sztuki, liczba płyt, koszt netto/VAT/brutto, wersje i ID zapisanego wyniku. Nie uruchamia płatności.

Automatyczna cena wymaga zatwierdzonego materiału, dodatniej ceny zakupu netto/kg i dostępnego koloru w DB oraz `PRICING_RATES_JSON`. Import nigdy ich nie wymyśla ani nie nadpisuje. Zapisy operatora przez admin API są dopiero w B5; obecnie lokalne SQL, bez publicznego endpointu zapisu.

`PRICING_RATES_JSON` zawiera: `version`, `setup_grosze`, `handling_per_plate_grosze`, `machine_per_hour_grosze`, `energy_per_kwh_grosze`, `printer_watts`, `loss_bps`, `margin_bps`, `vat_bps`, `minimum_net_grosze`. Stawki pieniężne są netto w groszach; 100 bps = 1%. Marża jest udziałem w cenie netto, a nie narzutem na koszt. Dane muszą zostać zatwierdzone przez operatora; brak konfiguracji oznacza ocenę indywidualną.

Pomiar slicera jest cache’owany po modelu/materiale/profilach: maks. 64 wpisy, 10 minut. Oryginał jest tymczasowy i usuwany. Wycena oraz snapshot stawek/ceny zakupu zostają w PostgreSQL. Każda kopia zajmuje obecnie osobną płytę; przygotowanie zamówienia liczone jest raz. Wspólne płyty, inne ustawienia jakości i bezpieczna siatka STEP/3MF są kolejnymi krokami.

## Istniejący checkout

`api/checkout.rs` nadal obsługuje Stripe/Furgonetkę i starą taryfę demonstracyjną 24 zł/szt. + 15 zł przygotowania. Nie korzysta jeszcze z nowych wyników slicera. Jego analizy/oferty/sesje pozostają w RAM; restart je usuwa. Panele/logowanie/chat nadal mają mocki. Nie traktuj nowego endpointu jako gotowej sprzedaży.

## Krótkie sprawdzenie

```sh
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api -p rapidmaker-pricing
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-slicer -- --include-ignored
```

Test PostgreSQL w `storage` jest ignorowany domyślnie; uruchamiaj wyłącznie na bazie testowej przez `TEST_DATABASE_URL`, z `--include-ignored`. Test zmienia i przywraca wskazane fixture’y katalogu; nie jest przeznaczony dla bazy produkcyjnej.
