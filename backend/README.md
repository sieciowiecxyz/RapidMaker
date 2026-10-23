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

API: `127.0.0.1:18432`; frontend Vite przekazuje `/api`. Launcher czyta tylko dozwolone klucze z rootowego `.env` (0600, SOPS): sandboxy Stripe/Furgonetki, `DATABASE_URL`, `PRICING_RATES_JSON`, `ADMIN_API_TOKEN`, `MODEL_STORAGE_DIR`.

`DATABASE_URL` uruchamia migracje i idempotentny import katalogu. Bez tej zmiennej stary sandbox działa, nowe endpointy zwracają 503. Nieprawidłowa konfiguracja DB/cennika zatrzymuje start, bez ujawniania poświadczeń.

Lokalny PostgreSQL 16: rootless Podman, kontener i wolumen `rapidmaker-catalog-postgres`, port `127.0.0.1:18434`. Start istniejącego kontenera: `podman start rapidmaker-catalog-postgres`. Dane dostępu są prywatne, nie w README. Po sklonowaniu potrzebujesz własnego PostgreSQL i odpowiedniego `DATABASE_URL`.

Linux + bubblewrap + prlimit są wymagane dla parsera i slicera. Slicer korzysta z lokalnego `/opt/bambustudio-bin` (sprawdzono Bambu Studio 02.08.02.61). Jego zasoby są tylko do odczytu; proces nie ma sieci ani sekretów. Drukarka: P1P hardened steel 0,4 mm, Textured PEI. Profile materiałów wybierane są z zaufanego katalogu: 41 materiałów ma profil P1P 0,4 mm do dalszej kwalifikacji. Materiały podporowe i brakujące profile nie są samodzielną ofertą. Fizyczna zgodność ekstrudera/obudowy i dopuszczenie każdego materiału należą do operatora. Faktyczny slicer sprawdzono dla PLA Basic, PETG HF i ABS; nie potwierdza to jakości rzeczywistego wydruku.

## Nowe API

- `GET /api/catalog/materials`: katalog PostgreSQL, kolory i dostępność. Bez kosztów zakupu i surowych presetów.
- `POST /api/models`: surowy STL, STEP/STP lub 3MF do 32 MiB; `X-Model-Filename` = nazwa zakodowana przez `encodeURIComponent`. Zwraca 201, ID, wymiary i termin wygaśnięcia; ustawia prywatną sesję cookie.
- `GET /api/models/{id}` i `/api/models/{id}/source`: metadane i pobranie oryginału, tylko z cookie właściciela. Brak sesji → 401, obcy/wygasły model → 404.
- `POST /api/quotes/estimate`: JSON `{"model_id":"ID z uploadu","material_id":"GFA00","color_code":"10300","options":{"quantity":1,"quality":"standard","wall_loops":2,"infill_percent":15}}`. Wymaga cookie właściciela. Ilość 1–1000, jakość draft/standard/fine, 2–8 ścian, 5–100% wypełnienia. Wynik: pomiar partii, koszt netto/VAT/brutto i ID niezmiennego snapshotu. Nie uruchamia płatności.

Automatyczna cena wymaga zatwierdzonego materiału, dodatniej ceny zakupu netto/kg i dostępnego koloru w DB oraz `PRICING_RATES_JSON`. Import nigdy ich nie wymyśla ani nie nadpisuje. Zapisy operatora korzystają z chronionego API poniżej. Rola z browser mocka nie daje uprawnień. Cennik w PostgreSQL ma pierwszeństwo przed konfiguracją startową `.env`; zmiana działa dla nowych wycen bez restartu.

`PRICING_RATES_JSON` zawiera: `version`, `setup_grosze`, `handling_per_plate_grosze`, `machine_per_hour_grosze`, `energy_per_kwh_grosze`, `printer_watts`, `loss_bps`, `margin_bps`, `vat_bps`, `minimum_net_grosze`. Stawki pieniężne są netto w groszach; 100 bps = 1%. Marża jest udziałem w cenie netto, a nie narzutem na koszt. Dane muszą zostać zatwierdzone przez operatora; brak konfiguracji oznacza ocenę indywidualną.

Pomiar slicera jest cache’owany po modelu/formacie/materiale/ustawieniach/ilości/profilach: maks. 64 wpisy, 10 minut. Oryginał i kanoniczna siatka są prywatne przez 24 h i przetrwają restart. `MODEL_STORAGE_DIR` (domyślnie `~/.local/state/rapidmaker/models`) wymaga 0700, pliki mają 0600. Limity łącznie z siatkami: 20 modeli/256 MiB na właściciela, 1000/8 GiB globalnie; wygasłe pliki sprzątane przy uploadzie. Wycena oraz snapshot stawek/ceny zakupu zostają w PostgreSQL. Każdy format przechodzi przez izolowaną konwersję do samej siatki STL; ustawienia/G-code klienta nie docierają do slicera. STEP wymaga pojedynczej części z jawnymi jednostkami mm; 3MF zachowuje jednostki i transformacje. Slicer rzeczywiście układa kopie: mierzy pełną płytę, powtarza jej wynik i osobno mierzy resztę. Przygotowanie jest raz na zamówienie, obsługa na faktyczną liczbę płyt. Maks. 512 kopii w pojedynczym układaniu i dwa uruchomienia slicera na zamówienie; daje to wykonalny plan, nie gwarancję optymalnego nesting’u. Opcje jakości/ścian/wypełnienia stosowane są do profili, 100% używa wzoru rectilinear. Kierunek ułożenia oryginału jest zachowany; automatyczne obracanie pod kątem jakości wydruku pozostaje oceną operatora.

## API operatora

Skrypty używają `Authorization: Bearer <ADMIN_API_TOKEN>`; panel korzysta z sesji cookie. Token jest losowym sekretem w lokalnym `.env`/SOPS; nie umieszczaj go w kodzie, `VITE_*`, localStorage ani repo jawnym tekstem. Brak konfiguracji wyłącza API operatora, błędny token daje 401. Panel działa na `/admin/materialy` i `/admin/ustawienia`; logowanie `/logowanie?operator=1` używa prywatnego klucza z `.env`. Po logowaniu przeglądarka dostaje losową sesję HttpOnly/SameSite=Strict na 6 godzin. Aplikacja nie zapisuje klucza w localStorage/sessionStorage ani bundle; po logowaniu używa wyłącznie cookie. Sesje są ograniczone do 64, próby logowania do 10/min; restart API unieważnia sesje. Obecne HTTP/cookie bez Secure są przeznaczone tylko dla localhosta. Publiczne wdrożenie wymaga HTTPS/Secure.

- `GET /api/admin/session`: stan sesji; `POST` z `{token}` tworzy sesję cookie, `DELETE` ją unieważnia. Browser mock nie nadaje uprawnień.
- `GET /api/admin/materials`: pełny katalog z kosztami i metadanymi.
- `PUT /api/admin/materials/{id}`: dodaj/edytuj `name`, `density_g_cm3`, `cost_net_grosze_per_kg` (liczba groszy lub null), `production_enabled`. Włączenie wymaga dodatniej ceny. Własny materiał bez zaufanego profilu nie otrzyma automatycznej wyceny.
- `PUT /api/admin/materials/{id}/colors/{code}`: `code`, `name`, `rgba` (lista `#RRGGBBAA`), `in_stock`.
- `GET /api/admin/pricing`: obowiązujące stawki albo null.
- `PUT /api/admin/pricing`: cały obiekt stawek wymieniony wyżej. Przy zmianie wymagana jest nowa `version`; identyczne retry jest dozwolone, zmiana z tą samą wersją daje 409.

Import aktualizuje źródłowe metadane materiału, zachowując konfigurację operatora i dodane kolory. Snapshoty wcześniejszych wycen nie są przeliczane ani nadpisywane.

## Istniejący checkout

`api/checkout.rs` nadal obsługuje Stripe/Furgonetkę i starą taryfę demonstracyjną 24 zł/szt. + 15 zł przygotowania. Nie korzysta jeszcze z nowych wyników slicera. Jego analizy/oferty/sesje pozostają w RAM; restart je usuwa. Panel materiałów/cennika i logowanie operatora korzystają z backendu. Konta klientów, zamówienia, produkcja i chat nadal mają mocki; ich stare widoki admina nie są częścią działającego panelu konfiguracji. Nie traktuj nowego endpointu jako gotowej sprzedaży.

## Krótkie sprawdzenie

```sh
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api -p rapidmaker-pricing
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-slicer -- --include-ignored
```

Test PostgreSQL w `storage` jest ignorowany domyślnie; uruchamiaj wyłącznie na bazie testowej przez `TEST_DATABASE_URL`, z `--include-ignored`. Test zmienia i przywraca wskazane fixture’y katalogu; nie jest przeznaczony dla bazy produkcyjnej.
