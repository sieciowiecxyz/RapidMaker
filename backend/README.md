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
    ├── payments/        # Stripe test i podpisy webhooków
    ├── shipping/        # adres PL i Furgonetka sandbox
    ├── observability/   # strukturalne logi JSON
    └── orders/          # rezerwa
```

## Start

Wymagane: Rust ≥1.94, Linux, kompilator C++17 i systemowy Open CASCADE **7.9** (nagłówki oraz biblioteki, w tym TKDESTEP). Na Arch pakiet `opencascade`; sprawdzono 7.9.3. STEP używa małego mostu CXX do OCCT, reszta backendu pozostaje w Rust. Build nie pobiera binariów OCCT. Domyślny include: `/usr/include/opencascade`; dla innej instalacji ustaw `OCCT_INCLUDE_DIR` i ewentualnie `OCCT_LIB_DIR` podczas budowania. Biblioteki współdzielone muszą być dostępne także podczas uruchamiania API i workera; sam plik binarny nie wystarczy.

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

- `GET /api/catalog/materials`: PLA/ABS/TPU z PostgreSQL, kolory bez bramki magazynu i dopuszczenie automatycznej wyceny. Bez kosztów zakupu i surowych presetów.
- `POST /api/models`: surowy STL, STEP/STP lub 3MF do 32 MiB; `X-Model-Filename` = nazwa zakodowana przez `encodeURIComponent`. Zwraca 201, ID, wymiary i termin wygaśnięcia; ustawia prywatną sesję cookie.
- `GET /api/models/{id}` i `/api/models/{id}/source`: metadane i pobranie oryginału, tylko z cookie właściciela. Brak sesji → 401, obcy/wygasły model → 404.
- `POST /api/quotes/estimate`: JSON `{"model_id":"ID z uploadu","material_id":"GFA00","color_code":"10300","options":{"quantity":1,"quality":"standard","wall_loops":2,"infill_percent":15}}`. Wymaga cookie właściciela. Ilość 1–1000, jakość draft/standard/fine, 2–8 ścian, 5–100% wypełnienia. Odpowiedź 202: ID i status trwałego zadania; wynik/snapshot pobierany przez `GET /api/quotes/jobs/{id}`. Nie uruchamia płatności.

Automatyczna cena wymaga zatwierdzonego materiału, dodatniej ceny zakupu netto/kg i dostępnego koloru w DB oraz skonfigurowanych stawek (panel operatora lub `PRICING_RATES_JSON`). Import nigdy ich nie wymyśla ani nie nadpisuje. Zapisy operatora korzystają z chronionego API poniżej. Rola z browser mocka nie daje uprawnień. Cennik w PostgreSQL ma pierwszeństwo przed konfiguracją startową `.env`; zmiana działa dla nowych wycen bez restartu.

`PRICING_RATES_JSON` zawiera: `version`, `setup_grosze`, `handling_per_plate_grosze`, `machine_per_hour_grosze`, `energy_per_kwh_grosze`, `printer_watts`, `loss_bps`, `margin_bps`, `vat_bps`, `minimum_net_grosze`. Stawki pieniężne są netto w groszach; 100 bps = 1%. Marża jest udziałem w cenie netto, a nie narzutem na koszt. Dane muszą zostać zatwierdzone przez operatora; brak konfiguracji oznacza ocenę indywidualną.

Zadania i wyniki są trwałe w PostgreSQL, deduplikowane per właściciel i pełne wejście wyceny. API zwraca ID od razu, frontend odpytuje status. Worker ma lease/retry i może działać osobno: [kolejka](../docs/slicer-queue.md). Oryginał i kanoniczna siatka są prywatne przez 24 h i przetrwają restart. `MODEL_STORAGE_DIR` (domyślnie `~/.local/state/rapidmaker/models`) wymaga 0700, pliki mają 0600. Limity łącznie z siatkami i podglądami: 20 modeli/256 MiB na właściciela, 1000/8 GiB globalnie; wygasłe pliki sprzątane przy uploadzie i co minutę przez workera; aktywne zadania chronią model. Wycena oraz snapshot stawek/ceny zakupu zostają w PostgreSQL. Każdy format przechodzi przez izolowaną konwersję do samej siatki STL; ustawienia/G-code klienta nie docierają do slicera. STEP wymaga pojedynczej części z jawnymi jednostkami mm; 3MF zachowuje jednostki i transformacje. Slicer rzeczywiście układa kopie: mierzy pełną płytę, powtarza jej wynik i osobno mierzy resztę. Przygotowanie jest raz na zamówienie, obsługa na faktyczną liczbę płyt. Maks. 512 kopii w pojedynczym układaniu i dwa uruchomienia slicera na zamówienie; daje to wykonalny plan, nie gwarancję optymalnego nesting’u. Opcje jakości/ścian/wypełnienia stosowane są do profili, 100% używa wzoru rectilinear. Kierunek ułożenia oryginału jest zachowany; automatyczne obracanie pod kątem jakości wydruku pozostaje oceną operatora.

## API operatora

Panel operatora i klienta używa wspólnego auth: hasło albo passkey + obowiązkowy email/TOTP. UI logowania i ustawień jest podłączone; rola, pełne MFA, Origin i CSRF są sprawdzane na serwerze. Pierwszy admin i odzyskanie: [auth](../docs/auth.md).

- `GET /api/admin/session`: stan sesji; `POST` z `{token}` tworzy sesję cookie, `DELETE` ją unieważnia. Browser mock nie nadaje uprawnień.
- `GET /api/admin/materials`: pełny katalog z kosztami i metadanymi.
- `PUT /api/admin/materials/{id}`: dodaj/edytuj `name`, `density_g_cm3`, `cost_net_grosze_per_kg` (liczba groszy lub null), `production_enabled`. Włączenie wymaga dodatniej ceny. Własny materiał bez zaufanego profilu nie otrzyma automatycznej wyceny.
- `PUT /api/admin/materials/{id}/colors/{code}`: `code`, `name`, `rgba` (lista `#RRGGBBAA`), `in_stock`.
- `GET /api/admin/pricing`: obowiązujące stawki albo null.
- `PUT /api/admin/pricing`: cały obiekt stawek wymieniony wyżej. Przy zmianie wymagana jest nowa `version`; identyczne retry jest dozwolone, zmiana z tą samą wersją daje 409.

Import aktualizuje źródłowe metadane materiału, zachowując konfigurację operatora i dodane kolory. Snapshoty wcześniejszych wycen nie są przeliczane ani nadpisywane.

## Checkout

`POST /api/checkout/quote`: JSON `{parts:[{id,quantity,analysis_id,quote_id}],delivery,contact,manual}`. `analysis_id` to ID prywatnego modelu, `quote_id` to snapshot slicera; `id` identyfikuje pozycję koszyka. Cookie właściciela jest wymagane. Wymiary i masa klienta nie są akceptowane. Cena zakupu, dopuszczenie profilu, wersja stawek/profilu, model i ilość muszą nadal odpowiadać snapshotowi, ważnemu 10 min; inaczej 409 i ponowna wycena. Zmienione ustawienia wymagają nowego ID wyceny.

Koszty części sumowane przed naliczeniem jednej opłaty przygotowania, marży, minimum i VAT. Podział brutto między pozycje zachowuje dokładną sumę groszy. Pakowanie bierze wymiary modelu i masę slicera na sztukę, łącznie z odpadami/podporami (konserwatywny szacunek). Różne modele mają oddzielne partie; brak optymalizacji wspólnej płyty. Nieobsługiwane wymagania/brak snapshotu → ręczna ocena bez płatnej oferty.

Checkout ma trwały szkic, ofertę i próbę płatności w PostgreSQL. Przed Stripe atomowo zapisuje zamówienie, kwotę, wybraną dostawę/paczki i ochronę plików. Worker obsługuje podpisane webhooki, retry i odczyt po restarcie; zapłata i outbox zapisują się raz. Stare trasy są adapterami wspólnego silnika. Nowe DTO, kody błędów, lokalny listener i odzyskiwanie: [kontrakt checkoutu](../docs/checkout.md). Integracja pozostaje testowa; frontend kont/auth jest podłączony.

## Spis API

Zwroty Stripe test: `POST /api/admin/orders/{id}/refunds` z `request_id` UUID i `amount_grosze`; `GET` pod tym samym adresem zwraca statusy. Admin/MFA/CSRF, pełne lub częściowe kwoty, trwała idempotencja i uzgadnianie w tle. `202` oznacza zapis zlecenia. [Kontrakt](../docs/checkout.md#zwroty-płatności).

Otwórz [Swagger UI](http://localhost:18430/api). Specyfikacja OpenAPI: `/api/openapi.json`. UI i zasoby są serwowane lokalnie przez Rust, bez CDN. `utoipa` generuje schematy z typów, a `utoipa-axum` rejestruje jednocześnie trasę i jej dokumentację przez `routes!`; nie utrzymujemy osobnego spisu endpointów.

Rozwiń endpoint, aby zobaczyć parametry, body i odpowiedzi. `Try it out` wykonuje rzeczywiste żądanie. Logowanie ustawia cookie HttpOnly; dla operacji sesyjnych wymagających CSRF podaj token z `/api/auth/me` w `Authorize`. Admin wymaga również drugiego składnika. Używaj hosta `localhost` zgodnego z `AUTH_ORIGIN`. Jawne DTO mają wygenerowane schematy; odpowiedzi zwracane jako dynamiczny JSON pozostają ogólne.

## Podgląd GLB

Upload STL/STEP/3MF tworzy oryginał, `canonical.stl` i `preview.glb` w jednym prywatnym katalogu. Siatka i GLB powstają z jednego odczytu źródła w izolowanym procesie. GLB zawiera wyłącznie geometrię i własny BIN, bez materiałów, tekstur ani zewnętrznych URL. Podgląd ma metry/Y-up (transformacja węzła), siatka produkcyjna i `dimensions_mm` pozostają w mm/Z-up.

`POST /api/models` oraz `GET /api/models/{id}` zwracają `preview_url` i `preview_bytes`. Właściciel pobiera binarny podgląd przez `GET /api/models/{id}/preview.glb`: `model/gltf-binary`, `no-store`, `nosniff`. Obcy/wygasły model: 404; starszy upload bez podglądu: 409 i prośba o ponowne przesłanie (`preview_url:null`). Brak/uszkodzenie pliku nie powoduje zwrócenia zastępczego modelu. Pobranie nie uruchamia konwersji.

GLB maks. 25 MiB; rozmiar oryginału + siatki + GLB liczy się do wspólnych limitów. Zapis i pobieranie są prywatne, retencja i ochrona aktywnych zadań wspólne dla całego katalogu. Oryginał i STL nadal służą produkcji, GLB wyłącznie podglądowi. Frontend odczytuje wyłącznie nasz GLB przez ograniczony parser w workerze i natywny WebGL; biały podgląd nie wymaga koloru.

## Pliki zamówień

Przed zewnętrznym wywołaniem Stripe backend trwale wiąże zamówienie z własnymi snapshotami wycen i ich modelami. Bez ochrony plików nie utworzy zamówienia. Powtórzenie wymaga identycznego właściciela i zestawu snapshotów; cudze, wygasłe, zmienione lub już usuwane modele są odrzucane. Maks. 20 snapshotów/zamówienie i 30 nowych zabezpieczeń/właściciela/dobę.

Ochrona obejmuje oryginał, STL i GLB: przed płatnością, przy niepewnym wyniku i przez realizację. Przejście do `completed` ustawia 90 dni retencji w tej samej transakcji co status/email. Inne zamówienie tego samego modelu może nadal go chronić. Pliki nadal liczą się do limitów dysku. Niepewność Stripe nie zwalnia miejsca; automatyczne rozstrzyganie porzuconych prób wymaga kolejnego etapu webhooków/uzgadniania płatności.

- `GET /api/account/orders/{id}/files`: własne pliki, materiał/kolor/ilość/opcje i adresy pobrania. Pobranie `/files/{model_id}/source` lub `/preview.glb` działa niezależnie od roboczych 24 h; zasady dostępu do zamówienia pozostają wymagane.
- `GET /api/admin/orders/{id}/files`: to samo dla admina po MFA; dodatkowo `/files/{model_id}/mesh` daje STL do produkcji. Klient nie ma dostępu do tej trasy siatki.

Po zakończeniu retencji sprzątanie może usunąć pliki, ale zachowuje snapshoty/parametry/nazwy w historii zamówienia (`available:false`, pobranie 410). Restart nie usuwa powiązań. Starsze zamówienia bez wiarygodnego powiązania nie są automatycznie przypisywane do plików; nie odgadujemy go z identyfikatorów UI. Fizyczne katalogi nadal są prywatne; endpointy stosują tę samą kontrolę rozmiaru, typów i dowiązań co pobieranie modelu.

## Krótkie sprawdzenie

```sh
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-api -p rapidmaker-pricing
cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-slicer -- --include-ignored
```

Test PostgreSQL w `storage` jest ignorowany domyślnie; uruchamiaj wyłącznie na bazie testowej przez `TEST_DATABASE_URL`, z `--include-ignored`. Test zmienia i przywraca wskazane fixture’y katalogu; nie jest przeznaczony dla bazy produkcyjnej.

## Rozmowy i email

[Kontrakt i diagnostyka](../docs/chat-email.md): wspólna historia przez API/SMTP/IMAP, kopie treści, grupowanie 60 s, paginacja i read cursor. `MAIL_INBOUND_MODE=test`, `MAIL_IMAP_MAILBOX=void@sieciowiec.xyz`, osobny `MAIL_REPLY_KEY` w SOPS. Domyślnie odbiór disabled; worker startuje razem z API, odczytuje folder bez zmiany flag/usuwania. CLI `--mail-receive` odbiera jedną partię, `--mail-operator EMAIL on|off` nadaje/cofa wąską zgodę istniejącemu operatorowi. Frontend rozmów używa tego API; ostatni stan rzeczywistego doręczenia opisuje [checkpoint](../docs/MILESTONES.md#checkpoint-2026-10-25).


## Nadawanie przesyłek

Backend sandbox, bez nowego panelu. Podjazdy i anulowanie obsługuje backend opisany poniżej. Po opłaceniu i `ready`: GET/PUT `/api/admin/orders/{id}/shipment`, potem POST `/quote` i `/book` pod tym adresem (version/request_id UUID/quote_id). Wymaga sesji admina po pełnym MFA oraz CSRF. `202` potwierdza zapis zlecenia. Worker korzysta z PostgreSQL; GET zwraca bieżący stan/last_error, etykiety są prywatne. `/handover` potwierdza fizyczne przekazanie; klient czyta `/api/account/orders/{id}/shipment`. Niepewne wyniki nie są ponawiane w ciemno. Recovery i ponowna akceptacja kosztu: [kontrakt S0–S4](../docs/PLAN.md#s0s4--nadawanie-przesyłek).

Używa istniejących FURGONETKA_SANDBOX_TOKEN/FURGONETKA_SENDER_JSON; bez nowych sekretów. DPD sandbox sprawdzony: dwa kartony/dwa numery w jednej przesyłce, PDF, restart i ponowienie bez duplikatu, przekazanie, jedna wysyłka SMTP oraz symulowane doręczenie. Właściciel zaakceptował regulamin DPD i zweryfikował telefon konta; kod nie zapisuje zgód. Test używał opłaconego zamówienia/sesji admina z fixtures i void@sieciowiec.xyz.


### Podjazdy i anulowanie przesyłek

Wymagają admin/MFA + CSRF. Używaj lokalnych ID przesyłek z GET `/api/admin/orders/{id}/shipment`, aktualnej `version` i nowego `request_id` UUID dla nowej decyzji.

| Metoda i końcówka pod `/api/admin/orders/{id}/shipment` | Dane / wynik |
| --- | --- |
| POST `/pickup/proposals` | `{package_ids,ready_date:"YYYY-MM-DD",repeat:false}` → terminy Furgonetki |
| POST `/pickup` | `{version,request_id,package_ids,pickup_date:{date,min_time,max_time},repeat:false}` → 202 i trwała operacja |
| POST `/cancel` | `{version,request_id,package_ids}` → 202 i trwała operacja |
| GET `/actions` | Historia, wynik, numer podjazdu lub błąd przewoźnika |
| POST `/actions/{action_id}/reconcile` | `{version}` → ponowne sprawdzenie niepewnego wyniku, bez nowego zlecenia |

Godziny kopiuj z dostępnej propozycji wspólnej dla wszystkich wybranych przesyłek. `repeat:true` służy do ponowienia nieudanego odbioru, jeśli Furgonetka na to pozwala. Jedna operacja obejmuje jedno zamówienie i maks. 30 przesyłek Furgonetki; pojedyncza przesyłka może zawierać wiele kartonów. Podjazd nie oznacza przekazania kurierowi. Anulowanie nie zwraca pieniędzy w Stripe. Przekazanych lub anulowanych lokalnie paczek nie można ponownie zlecić tymi endpointami. Kontrakt i ograniczenia: [L1–L2](../docs/PLAN.md#l1l2--podjazdy-i-anulowanie).


### Logi

Backend Rust emituje JSON na stderr, domyślnie `LOG_LEVEL=info`. Udane odczyty i szczegóły dostawców: `debug`; błędy pozostają widoczne na poziomie domyślnym. Każda odpowiedź HTTP ma serwerowy `x-request-id`; logi mają szablon trasy i kontekst zadania/zamówienia. Bez sekretów, body, adresów, treści wiadomości i surowych błędów. Stdout CLI/workera pozostaje bez zmian. [Konfiguracja i granice](../docs/logging.md).

### Wielu przewoźników — sandbox

- Kurier: DPD, DHL, InPost, GLS, UPS i FedEx.
- Automaty: Paczkomaty InPost, ORLEN Paczka, DPD i DHL BOX. API zwraca identyfikator punktu ze wskazaniem sieci; przekaż go bez zmian jako `delivery.point_id`. Stare kody InPost nadal działają.
- Dla ORLEN ustaw w lokalnym `.env` `FURGONETKA_SENDER_POINTS_JSON='{"orlen":"KOD_PUNKTU_NADANIA"}'`. To punkt nadawcy z mapy Furgonetki, oddzielny od punktu klienta. Launcher przekazuje go do Rust. ORLEN może zwrócić pusty kod pocztowy punktu; identyfikację potwierdza API po jego ID.
- Cena sumuje wszystkie zlecenia: automaty i kurier UPS/InPost po jednym kartonie, DPD/DHL/GLS/FedEx do 15; ograniczenia sprawdza Furgonetka. Powtarzalne paczki nie mnożą identycznych wywołań API. Limit 20 obliczeń ceny/30 walidacji/30 s; przekroczenie daje ręczną wycenę.
- Zgody usług zostały zaakceptowane w sandboxie za zgodą właściciela, bez marketingu. Kod nie akceptuje regulaminów. Produkcja wymaga osobnej konfiguracji i zgód; ceny sandboxa nie są cennikiem produkcyjnym.

Sprawdzono rzeczywisty adapter Rust: sześciu kurierów i cztery sieci automatów, po jednym i dwóch kartonach 1 kg; wycena, walidacja i zgodność sum checkoutu z przygotowaniem nadania. Nowych nadań ani podjazdów dla tych sieci nie wykonano. [Dowody](../docs/MILESTONES.md#l4--wielu-przewoźników--pass-sandbox-2026-10-24).

### Metryki

`METRICS_TOKEN` w `.env`/SOPS włącza prywatne `127.0.0.1:18435/metrics` z Bearer auth; brak zmiennej wyłącza zbieranie. HTTP: liczniki/klasy statusu, histogram opóźnienia, in-flight i uptime; bez danych klienta i surowych URL. Osobny listener, bez trasy w publicznym API. [Konfiguracja Prometheusa/Grafany i granice](../docs/metrics.md).

Uprawnienia i aktualne wyniki audytu: [security.md](../docs/security.md). Izolowany test API obejmuje dwóch klientów, gościa, admina/MFA, CSRF i własność plików.

## Ręczna oferta

`api/manual_quotes.rs` orkiestruje `storage/manual_quotes.rs` (migracja 0019), istniejące płatności i outbox. Zalogowany klient: `POST /api/account/manual-quotes`; admin: `POST /api/admin/orders/{id}/quote`; klient: `POST /api/account/orders/{id}/payment`. Kontrakty i typy w OpenAPI `/api/`. Oferta i status są w `manual_offer` własnego zamówienia.

Brak automatycznego profilu/ceny, przekroczenie 500 mm lub niepasująca P1P kierują do ręcznej oceny. Admin podaje całkowite brutto i zawartą w nim dostawę, zakres oraz uzgodnioną metodę; nie udajemy automatycznej oferty Furgonetki. Prośba ma 14 dni, oferta 7 dni. Jedna próba płatności na zamówienie; po rozpoczęciu nie można zmienić ceny, po definitywnym wygaśnięciu potrzebne nowe zgłoszenie. Publikacja/email i zapłata/email są atomowe.

Worker uzgadnia próby ze Stripe. Po 24 h zamyka tylko potwierdzone nieopłacone Session/Intent; processing, succeeded i requires_capture zachowują ochronę plików. Awaria Stripe nie zwalnia hold. Sprzątanie usuwa cały katalog modelu dopiero po utracie wszystkich aktywnych hold.
