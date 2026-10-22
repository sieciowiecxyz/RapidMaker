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

API: `127.0.0.1:18432`; frontend Vite przekazuje `/api`. Launcher czyta tylko dozwolone klucze z rootowego `.env` (0600, SOPS): sandboxy Stripe/Furgonetki, `DATABASE_URL`, `PRICING_RATES_JSON`, `ADMIN_API_TOKEN`.

`DATABASE_URL` uruchamia migracje i idempotentny import katalogu. Bez tej zmiennej stary sandbox działa, nowe endpointy zwracają 503. Nieprawidłowa konfiguracja DB/cennika zatrzymuje start, bez ujawniania poświadczeń.

Lokalny PostgreSQL 16: rootless Podman, kontener i wolumen `rapidmaker-catalog-postgres`, port `127.0.0.1:18434`. Start istniejącego kontenera: `podman start rapidmaker-catalog-postgres`. Dane dostępu są prywatne, nie w README. Po sklonowaniu potrzebujesz własnego PostgreSQL i odpowiedniego `DATABASE_URL`.

Linux + bubblewrap + prlimit są wymagane dla parsera i slicera. Slicer korzysta z lokalnego `/opt/bambustudio-bin` (sprawdzono Bambu Studio 02.08.02.61). Jego zasoby są tylko do odczytu; proces nie ma sieci ani sekretów. Drukarka: P1P hardened steel 0,4 mm, Textured PEI. Profile materiałów wybierane są z zaufanego katalogu: 41 materiałów ma profil P1P 0,4 mm do dalszej kwalifikacji. Materiały podporowe i brakujące profile nie są samodzielną ofertą. Fizyczna zgodność ekstrudera/obudowy i dopuszczenie każdego materiału należą do operatora. Faktyczny slicer sprawdzono dla PLA Basic, PETG HF i ABS; nie potwierdza to jakości rzeczywistego wydruku.

## Nowe API

- `GET /api/catalog/materials`: katalog PostgreSQL, kolory i dostępność. Bez kosztów zakupu i surowych presetów.
- `POST /api/quotes/estimate`: body = surowy STL, STEP/STP lub 3MF do 32 MiB. Nagłówki: `X-Model-Filename`, `X-Material-Id`, `X-Color-Code`, `X-Quantity` (1–1000). Opcjonalnie `X-Print-Quality` = draft/standard/fine, `X-Wall-Loops` = 2–8, `X-Infill-Percent` = 5–100. Wynik: pomiar całego zamówienia (`measurement`) i pełnych/resztkowych płyt, koszt netto/VAT/brutto, ustawienia, wersje i ID zapisanego wyniku. Nie uruchamia płatności.

Automatyczna cena wymaga zatwierdzonego materiału, dodatniej ceny zakupu netto/kg i dostępnego koloru w DB oraz `PRICING_RATES_JSON`. Import nigdy ich nie wymyśla ani nie nadpisuje. Zapisy operatora korzystają z chronionego API poniżej. Rola z browser mocka nie daje uprawnień. Cennik w PostgreSQL ma pierwszeństwo przed konfiguracją startową `.env`; zmiana działa dla nowych wycen bez restartu.

`PRICING_RATES_JSON` zawiera: `version`, `setup_grosze`, `handling_per_plate_grosze`, `machine_per_hour_grosze`, `energy_per_kwh_grosze`, `printer_watts`, `loss_bps`, `margin_bps`, `vat_bps`, `minimum_net_grosze`. Stawki pieniężne są netto w groszach; 100 bps = 1%. Marża jest udziałem w cenie netto, a nie narzutem na koszt. Dane muszą zostać zatwierdzone przez operatora; brak konfiguracji oznacza ocenę indywidualną.

Pomiar slicera jest cache’owany po modelu/formacie/materiale/ustawieniach/ilości/profilach: maks. 64 wpisy, 10 minut. Oryginał jest tymczasowy i usuwany. Wycena oraz snapshot stawek/ceny zakupu zostają w PostgreSQL. Każdy format przechodzi przez izolowaną konwersję do samej siatki STL; ustawienia/G-code klienta nie docierają do slicera. STEP wymaga pojedynczej części z jawnymi jednostkami mm; 3MF zachowuje jednostki i transformacje. Slicer rzeczywiście układa kopie: mierzy pełną płytę, powtarza jej wynik i osobno mierzy resztę. Przygotowanie jest raz na zamówienie, obsługa na faktyczną liczbę płyt. Maks. 512 kopii w pojedynczym układaniu i dwa uruchomienia slicera na zamówienie; daje to wykonalny plan, nie gwarancję optymalnego nesting’u. Opcje jakości/ścian/wypełnienia stosowane są do profili, 100% używa wzoru rectilinear. Kierunek ułożenia oryginału jest zachowany; automatyczne obracanie pod kątem jakości wydruku pozostaje oceną operatora.

## API operatora

Wymaga `Authorization: Bearer <ADMIN_API_TOKEN>`. Token jest losowym sekretem w lokalnym `.env`/SOPS; nie umieszczaj go w kodzie, `VITE_*`, localStorage ani repo jawnym tekstem. Brak konfiguracji wyłącza API operatora, błędny token daje 401. Dostęp jest ograniczony do obecnego localhosta; sesja panelu w przeglądarce pozostaje do podłączenia.

- `GET /api/admin/materials`: pełny katalog z kosztami i metadanymi.
- `PUT /api/admin/materials/{id}`: dodaj/edytuj `name`, `density_g_cm3`, `cost_net_grosze_per_kg` (liczba groszy lub null), `production_enabled`. Włączenie wymaga dodatniej ceny. Własny materiał bez zaufanego profilu nie otrzyma automatycznej wyceny.
- `PUT /api/admin/materials/{id}/colors/{code}`: `code`, `name`, `rgba` (lista `#RRGGBBAA`), `in_stock`.
- `GET /api/admin/pricing`: obowiązujące stawki albo null.
- `PUT /api/admin/pricing`: cały obiekt stawek wymieniony wyżej. Przy zmianie wymagana jest nowa `version`; identyczne retry jest dozwolone, zmiana z tą samą wersją daje 409.

Import aktualizuje źródłowe metadane materiału, zachowując konfigurację operatora i dodane kolory. Snapshoty wcześniejszych wycen nie są przeliczane ani nadpisywane.

## Istniejący checkout

`api/checkout.rs` nadal obsługuje Stripe/Furgonetkę i starą taryfę demonstracyjną 24 zł/szt. + 15 zł przygotowania. Nie korzysta jeszcze z nowych wyników slicera. Jego analizy/oferty/sesje pozostają w RAM; restart je usuwa. Panele/logowanie/chat nadal mają mocki. Nie traktuj nowego endpointu jako gotowej sprzedaży.

## Krótkie sprawdzenie

```sh
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api -p rapidmaker-pricing
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-slicer -- --include-ignored
```

Test PostgreSQL w `storage` jest ignorowany domyślnie; uruchamiaj wyłącznie na bazie testowej przez `TEST_DATABASE_URL`, z `--include-ignored`. Test zmienia i przywraca wskazane fixture’y katalogu; nie jest przeznaczony dla bazy produkcyjnej.
