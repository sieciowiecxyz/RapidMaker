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
    ├── notifications/   # iCloud SMTP i tekstowe wiadomości
    ├── auth/            # crypto kodów, sesji i CSRF
    └── ...              # rezerwa orders/payments/shipping/observability
```

## Start

```sh
cargo build --locked --manifest-path backend/Cargo.toml
python backend/run.py
```

API: `127.0.0.1:18432`; frontend Vite przekazuje `/api`. Launcher czyta tylko dozwolone klucze z rootowego `.env` (0600, SOPS): sandboxy Stripe/Furgonetki, `DATABASE_URL`, `PRICING_RATES_JSON`, `AUTH_*`, `MODEL_STORAGE_DIR` oraz konfigurację SMTP poniżej.

## Email

`rapidmaker-notifications` udostępnia `Mailer::from_env()` i `send(recipient, Notification)`. API współdzieli jeden Mailer. Kody wysyła bezpośrednio; zdarzenia zamówień/rozmów zapisuje atomowo z outboxem w PostgreSQL. Jeden worker wykonuje maks. 10 prób wysyłki/min, odzyskuje lease po restarcie, honoruje preferencje i ma stały Message-ID. Brak publicznego endpointu wysyłki. Stan kolejki: chronione `GET /api/admin/notifications`.

Backend klienta: `AUTH_MODE=localhost`, `AUTH_CODE_KEY` (32 losowe bajty hex w `.env`/SOPS), opcjonalnie `AUTH_ORIGIN` (domyślnie `http://localhost:18430`). Kod email jest jednorazowy, ważny 10 min; sesja HttpOnly/SameSite=Lax trwa do 30 dni, wygasa po 7 dniach bezczynności. Brak trybu oznacza disabled; production obsługuje origin HTTPS i Secure cookie; wymaga skonfigurowanego reverse proxy TLS. Passkey chroni admina; szczegóły w docs/auth.md. UI kont nadal korzysta z mocków.

Trasy klienta: `/api/auth/{start,verify,me,logout,logout-all}`, `GET/PUT /api/account/notifications`, `/api/account/orders/{id}` i `/messages`, `POST /payment-status`. Mutacje wymagają JSON, dokładnego Origin i CSRF z `/me`; start/verify używają powiązania przeglądarki. Operator obsługuje `/api/admin/orders/{id}/{status,messages}` przez sesję konta z potwierdzonym passkey. Szczegóły kontraktu: [email](../docs/email.md).

Konfiguracja w `.env`: `MAIL_MODE=disabled|test|live` (domyślnie disabled), `MAIL_SMTP_HOST=smtp.mail.me.com`, `MAIL_SMTP_PORT=587`, `MAIL_USER`, hasło aplikacji `MAIL_PASSWORD`, dopuszczony alias `MAIL_FROM`, `MAIL_REPLY_TO`, `MAIL_TEST_TO`. STARTTLS jest wymagane. `test` przyjmuje wyłącznie dokładny adres `MAIL_TEST_TO`; niczego nie przekierowuje. Na tym stanowisku używamy `void@sieciowiec.xyz` w trybie testowym. Dane IMAP nie są potrzebne serwerowi do wysyłki.

Po buildzie jedną wiadomość testową wysyła lokalna komenda:

```sh
MAIL_MODE=test python backend/run.py --mail-test
```

Komenda nie uruchamia HTTP ani nie wymaga DB. Wynik potwierdza przyjęcie przez SMTP, odbiór sprawdź w skrzynce. Błędy zwracają bezpieczną kategorię, bez adresów/treści/sekretów. Limit 20 s obejmuje kolejkę i SMTP; maks. 2 równoległe wysyłki na instancję. Nie ma automatycznego retry. Szczegóły i dalsze integracje: [plan email](../docs/email.md).

`DATABASE_URL` uruchamia migracje i idempotentny import katalogu. Bez tej zmiennej katalog, zapis modeli i automatyczna wycena nie działają (503); nie ma zastępczej taryfy. Nieprawidłowa konfiguracja DB/cennika zatrzymuje start, bez ujawniania poświadczeń.

Lokalny PostgreSQL 16: rootless Podman, kontener i wolumen `rapidmaker-catalog-postgres`, port `127.0.0.1:18434`. Start istniejącego kontenera: `podman start rapidmaker-catalog-postgres`. Dane dostępu są prywatne, nie w README. Po sklonowaniu potrzebujesz własnego PostgreSQL i odpowiedniego `DATABASE_URL`.

Linux + bubblewrap + prlimit są wymagane dla parsera i slicera. Slicer korzysta z lokalnego `/opt/bambustudio-bin` (sprawdzono Bambu Studio 02.08.02.61). Jego zasoby są tylko do odczytu; proces nie ma sieci ani sekretów. Drukarka: P1P hardened steel 0,4 mm, Textured PEI. Profile materiałów wybierane są z zaufanego katalogu: 41 materiałów ma profil P1P 0,4 mm do dalszej kwalifikacji. Materiały podporowe i brakujące profile nie są samodzielną ofertą. Fizyczna zgodność ekstrudera/obudowy i dopuszczenie każdego materiału należą do operatora. Faktyczny slicer sprawdzono dla PLA Basic, PETG HF i ABS; nie potwierdza to jakości rzeczywistego wydruku.

## Nowe API

- `GET /api/catalog/materials`: katalog PostgreSQL, kolory i dostępność. Bez kosztów zakupu i surowych presetów.
- `POST /api/models`: surowy STL, STEP/STP lub 3MF do 32 MiB; `X-Model-Filename` = nazwa zakodowana przez `encodeURIComponent`. Zwraca 201, ID, wymiary i termin wygaśnięcia; ustawia prywatną sesję cookie.
- `GET /api/models/{id}` i `/api/models/{id}/source`: metadane i pobranie oryginału, tylko z cookie właściciela. Brak sesji → 401, obcy/wygasły model → 404.
- `POST /api/quotes/estimate`: JSON `{"model_id":"ID z uploadu","material_id":"GFA00","color_code":"10300","options":{"quantity":1,"quality":"standard","wall_loops":2,"infill_percent":15}}`. Wymaga cookie właściciela. Ilość 1–1000, jakość draft/standard/fine, 2–8 ścian, 5–100% wypełnienia. Odpowiedź 202: ID i status trwałego zadania; wynik/snapshot pobierany przez `GET /api/quotes/jobs/{id}`. Nie uruchamia płatności.

Automatyczna cena wymaga zatwierdzonego materiału, dodatniej ceny zakupu netto/kg i dostępnego koloru w DB oraz skonfigurowanych stawek (panel operatora lub `PRICING_RATES_JSON`). Import nigdy ich nie wymyśla ani nie nadpisuje. Zapisy operatora korzystają z chronionego API poniżej. Rola z browser mocka nie daje uprawnień. Cennik w PostgreSQL ma pierwszeństwo przed konfiguracją startową `.env`; zmiana działa dla nowych wycen bez restartu.

`PRICING_RATES_JSON` zawiera: `version`, `setup_grosze`, `handling_per_plate_grosze`, `machine_per_hour_grosze`, `energy_per_kwh_grosze`, `printer_watts`, `loss_bps`, `margin_bps`, `vat_bps`, `minimum_net_grosze`. Stawki pieniężne są netto w groszach; 100 bps = 1%. Marża jest udziałem w cenie netto, a nie narzutem na koszt. Dane muszą zostać zatwierdzone przez operatora; brak konfiguracji oznacza ocenę indywidualną.

Zadania i wyniki są trwałe w PostgreSQL, deduplikowane per właściciel i pełne wejście wyceny. API zwraca ID od razu, frontend odpytuje status. Worker ma lease/retry i może działać osobno: [kolejka](../docs/slicer-queue.md). Oryginał i kanoniczna siatka są prywatne przez 24 h i przetrwają restart. `MODEL_STORAGE_DIR` (domyślnie `~/.local/state/rapidmaker/models`) wymaga 0700, pliki mają 0600. Limity łącznie z siatkami i podglądami: 20 modeli/256 MiB na właściciela, 1000/8 GiB globalnie; wygasłe pliki sprzątane przy uploadzie i co minutę przez workera; aktywne zadania chronią model. Wycena oraz snapshot stawek/ceny zakupu zostają w PostgreSQL. Każdy format przechodzi przez izolowaną konwersję do samej siatki STL; ustawienia/G-code klienta nie docierają do slicera. STEP wymaga pojedynczej części z jawnymi jednostkami mm; 3MF zachowuje jednostki i transformacje. Slicer rzeczywiście układa kopie: mierzy pełną płytę, powtarza jej wynik i osobno mierzy resztę. Przygotowanie jest raz na zamówienie, obsługa na faktyczną liczbę płyt. Maks. 512 kopii w pojedynczym układaniu i dwa uruchomienia slicera na zamówienie; daje to wykonalny plan, nie gwarancję optymalnego nesting’u. Opcje jakości/ścian/wypełnienia stosowane są do profili, 100% używa wzoru rectilinear. Kierunek ułożenia oryginału jest zachowany; automatyczne obracanie pod kątem jakości wydruku pozostaje oceną operatora.

## API operatora

Panel operatora wymaga teraz wspólnej sesji konta z potwierdzonym passkey. Stary ADMIN_API_TOKEN/Bearer i sesja operatora są wyłączone. Każdy zapis sprawdza rolę/MFA/Origin/CSRF. Pierwszy admin: zweryfikuj email i wykonaj lokalny bootstrap; kroki i kontrakt: [auth](../docs/auth.md). Frontend admina/logowania wymaga migracji do nowych endpointów.

- `GET /api/admin/session`: stan sesji; `POST` z `{token}` tworzy sesję cookie, `DELETE` ją unieważnia. Browser mock nie nadaje uprawnień.
- `GET /api/admin/materials`: pełny katalog z kosztami i metadanymi.
- `PUT /api/admin/materials/{id}`: dodaj/edytuj `name`, `density_g_cm3`, `cost_net_grosze_per_kg` (liczba groszy lub null), `production_enabled`. Włączenie wymaga dodatniej ceny. Własny materiał bez zaufanego profilu nie otrzyma automatycznej wyceny.
- `PUT /api/admin/materials/{id}/colors/{code}`: `code`, `name`, `rgba` (lista `#RRGGBBAA`), `in_stock`.
- `GET /api/admin/pricing`: obowiązujące stawki albo null.
- `PUT /api/admin/pricing`: cały obiekt stawek wymieniony wyżej. Przy zmianie wymagana jest nowa `version`; identyczne retry jest dozwolone, zmiana z tą samą wersją daje 409.

Import aktualizuje źródłowe metadane materiału, zachowując konfigurację operatora i dodane kolory. Snapshoty wcześniejszych wycen nie są przeliczane ani nadpisywane.

## Checkout

`POST /api/checkout/quote`: JSON `{parts:[{id,quantity,analysis_id,quote_id}],delivery,contact,manual}`. `analysis_id` to ID prywatnego modelu, `quote_id` to snapshot slicera; `id` identyfikuje pozycję koszyka. Cookie właściciela jest wymagane. Wymiary i masa klienta nie są akceptowane. Cena zakupu, dostępność, wersja stawek/profilu, model i ilość muszą nadal odpowiadać snapshotowi, ważnemu 10 min; inaczej 409 i ponowna wycena. Zmienione ustawienia wymagają nowego ID wyceny.

Koszty części sumowane przed naliczeniem jednej opłaty przygotowania, marży, minimum i VAT. Podział brutto między pozycje zachowuje dokładną sumę groszy. Pakowanie bierze wymiary modelu i masę slicera na sztukę, łącznie z odpadami/podporami (konserwatywny szacunek). Różne modele mają oddzielne partie; brak optymalizacji wspólnej płyty. Nieobsługiwane wymagania/brak snapshotu → ręczna ocena bez płatnej oferty.

Stripe test i Furgonetka sandbox używają wyłącznie kwoty/danych serwera. Utworzenie, status i wznowienie płatności sprawdzają właściciela; przed utworzeniem także ważność wyceny i dostępność. Oferty/sekrety wznowienia Stripe nadal są w RAM. Zamówienie, referencja płatności, status, rozmowa i powiadomienia są w PostgreSQL. Własny `/api/account/orders/{id}/payment-status` sprawdza Stripe także po restarcie; potwierdzenie płatności zapisuje z mailem atomowo. Nie ma jeszcze produkcyjnych webhooków ani automatycznego monitorowania Stripe bez odpytywania. Frontend klienta trzeba podłączyć do nowego auth i zamówień.

## Podgląd GLB

Upload STL/STEP/3MF tworzy oryginał, `canonical.stl` i `preview.glb` w jednym prywatnym katalogu. Siatka i GLB powstają z jednego odczytu źródła w izolowanym procesie. GLB zawiera wyłącznie geometrię i własny BIN, bez materiałów, tekstur ani zewnętrznych URL. Podgląd ma metry/Y-up (transformacja węzła), siatka produkcyjna i `dimensions_mm` pozostają w mm/Z-up.

`POST /api/models` oraz `GET /api/models/{id}` zwracają `preview_url` i `preview_bytes`. Właściciel pobiera binarny podgląd przez `GET /api/models/{id}/preview.glb`: `model/gltf-binary`, `no-store`, `nosniff`. Obcy/wygasły model: 404; starszy upload bez podglądu: 409 i prośba o ponowne przesłanie (`preview_url:null`). Brak/uszkodzenie pliku nie powoduje zwrócenia zastępczego modelu. Pobranie nie uruchamia konwersji.

GLB maks. 25 MiB; rozmiar oryginału + siatki + GLB liczy się do wspólnych limitów. Zapis i pobieranie są prywatne, retencja i ochrona aktywnych zadań wspólne dla całego katalogu. Oryginał i STL nadal służą produkcji, GLB wyłącznie podglądowi. Backend jest gotowy; frontend GLTFLoader/Three.js wymaga osobnego podłączenia.

## Krótkie sprawdzenie

```sh
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api -p rapidmaker-pricing
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-slicer -- --include-ignored
```

Test PostgreSQL w `storage` jest ignorowany domyślnie; uruchamiaj wyłącznie na bazie testowej przez `TEST_DATABASE_URL`, z `--include-ignored`. Test zmienia i przywraca wskazane fixture’y katalogu; nie jest przeznaczony dla bazy produkcyjnej.
