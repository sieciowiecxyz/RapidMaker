# Rapidmaker — rapidmaker.pl

Strona do zamawiania wydruków 3D FDM: dodaj części STL, STEP/STP lub 3MF, skonfiguruj druk, wybierz dostawę i zapłać. Budujemy prosty interfejs i mały backend zgodnie z KISS.

## Podział pracy

| Osoba | Odpowiedzialność |
| --- | --- |
| **Piotr — frontend** | Wygląd, responsywność, konfigurator wielu części, checkout, panel klienta, panel admina, podgląd 3D i obsługa stanów ładowania/błędów. Podłączenie API oraz wizualne sprawdzanie przepływu zakupu w sandboxach. |
| **Kuba — backend i infrastruktura** | API, baza danych, konta i uprawnienia, pliki i geometria, slicer i wycena, algorytm pakowania, integracje Stripe/Furgonetki, chat i email, powiadomienia, bezpieczeństwo, logi, monitoring, backupy i wdrożenie. |

Wspólnie ustalamy formaty danych API i zachowanie ekranów. Wspólny adapter `frontend/src/lib/api.ts` obsługuje checkout i mocki; `frontend/src/storefront/lib/api.ts` łączy z nim nowy konfigurator Piotra. Brakujące funkcje można rozwijać na mockach. Ceny, płatności, uprawnienia operatora i pakowanie rozstrzyga backend.

Checkout: **konfiguracja druku → dane i dostawa → podsumowanie → płatność**. Dane odbiorcy wpisujemy raz. Klient widzi zamówienia i rozmowę w swoim panelu; admin zarządza realizacją i odpowiada klientowi. Liczbę paczek i ich parametry wylicza system z części, ilości, zabezpieczeń i limitów przewoźnika. Sama objętość nie gwarantuje poprawnego pakowania.

## Aktualny stan

To POC. Działają frontend, API analizy modeli, szacowanie paczek oraz testowe integracje Stripe i Furgonetki. Konta klientów, chat i panele zamówień używają mocków i `localStorage`. Katalog, konfigurator, cennik i panel operatora korzystają z API/PostgreSQL.

Konfigurator i checkout liczą cenę z rzeczywistego slicera; koszyk nalicza przygotowanie raz. Prywatne modele i snapshoty są trwałe (24 h/10 min); oferty dostawy i stan płatności API pozostają w RAM. Operator loguje się na `/logowanie?operator=1` kluczem `ADMIN_API_TOKEN` z lokalnego `.env`; klucza nie wklejaj do repo ani czatu. Kalibracja kosztów, trwałe zamówienia, produkcyjne webhooki i email pozostają do wdrożenia. Konfiguracja i kontrakt: [backend/README.md](backend/README.md). Nowy frontend Piotra ma podgląd STL w natywnym WebGL, z parserem w workerze przeglądarki. STEP i 3MF analizuje backend; ich podgląd oraz wspólny viewer GLB/Three.js pozostają do podłączenia. Przykłady bez analizy serwera wymagają ręcznej wyceny.

Nowy landing i konfigurator są pod `/` i `/wycena`. Checkout (`/zamowienie`), logowanie oraz panele klienta i admina zachowują dotychczasową logikę w motywie Piotra. Opcje techniczne druku są pod `/wycena/zaawansowana`; powrót do konfiguratora zachowuje ustawienia koszyka.

## Stack i katalogi

| Miejsce | Technologia i zastosowanie |
| --- | --- |
| `frontend/` | Svelte 5, TypeScript, Vite, Tailwind CSS 4, GSAP i WebGL; motyw Piotra, adaptery API i mocki. |
| `backend/crates/` | Rust/Axum: `app` startuje serwer, `api` zbiera trasy, `storage` obsługuje PostgreSQL, `slicer` mierzy druk, `pricing` liczy cenę, `packing` paczki. |
| `backend/crates/geometry/` | Biblioteka Rust do analizy geometrii; worker izolowany przez bubblewrap i prlimit. |
| `backend/crates/geometry/tests/fixtures/` | Referencyjne modele do sprawdzania parsera. |
| `docs/` | Wizja, plan, milestones, architektura i instrukcje pracy. |
| `secrets/dev.enc.env` | Jeden zaszyfrowany plik konfiguracji SOPS; jawny `.env` zostaje lokalnie. |
| `.githooks/` | Automatyczne szyfrowanie zmienionego `.env` przed commitem. |

Mapa aktywnych i przyszłych crate’ów: [backend/README.md](backend/README.md).

Docelowa baza: PostgreSQL + SQLx. Płatności: Stripe test; dostawa: Furgonetka sandbox. Email docelowo przez iCloud SMTP/IMAP z hasłem aplikacji. Obecnie pracujemy na localhost.

## Pierwsze uruchomienie

Wymagania: Git, Node.js zgodny z Vite 7, npm, Rust/Cargo, Python 3, SOPS i age. Pełna analiza modeli wymaga Linux, `bubblewrap` (`bwrap`) oraz `prlimit` z util-linux.

```sh
git clone --branch main git@github.com:sieciowiecxyz/RapidMaker.git
cd RapidMaker
git config --local core.hooksPath .githooks
```

Odszyfruj konfigurację swoim kluczem prywatnym SSH uprawnionym w `.sops.yaml`. Zastąp przykładową ścieżkę własną:

```sh
export SOPS_AGE_SSH_PRIVATE_KEY_FILE=/ścieżka/do/klucza
umask 077
sops decrypt --output .env secrets/dev.enc.env
chmod 600 .env
```

Zainstaluj zależności i zbuduj backend:

```sh
npm --prefix frontend ci
cargo build --locked --manifest-path backend/Cargo.toml
```

Terminal 1 — API:

```sh
python backend/run.py
```

Terminal 2 — frontend:

```sh
npm --prefix frontend run dev
```

Otwórz **http://127.0.0.1:18430**. API działa na `127.0.0.1:18432`; Vite przekazuje do niego `/api`. `Ctrl+C` zatrzymuje dany proces. Po zmianie kodu Rust ponów build i restart API; po zmianie `.env` zrestartuj API.

Sam wygląd i mockowane panele można rozwijać z uruchomionym frontendem. Analiza plików i checkout wymagają API; Stripe i Furgonetka wymagają aktualnych danych testowych w `.env`.

## Sekrety i wspólna praca

SOPS szyfruje konfigurację dla dwóch publicznych kluczy SSH z `.sops.yaml`. Każdy odszyfrowuje ją własnym kluczem prywatnym. Po sklonowaniu aktywuj hook powyższą komendą, edytuj `.env` i commituj normalnie: hook odświeża `secrets/dev.enc.env` oraz dodaje go do commita. Jawnego `.env` nie dodawaj do Gita.

Po pobraniu cudzych zmian sekretów odszyfruj aktualny SOPS przed własną edycją `.env`. Szczegóły i dodawanie osób: [sekrety](docs/secrets.md). Pracujemy na testowych płatnościach i sandboxie przewoźnika.

## Krótkie sprawdzenie zmian

```sh
npm --prefix frontend run build
cargo build --locked --manifest-path backend/Cargo.toml
```

Przy zmianie UI obejrzyj zmieniony ekran na komputerze i telefonie. Wybrane testy E2E są w Playwright, testy backendu uruchomisz przez `cargo test --locked --manifest-path backend/Cargo.toml --workspace`. Zakres dobieraj do zmiany; pełny E2E nie jest wymagany przy każdej iteracji POC.

Dokumentacja: [indeks](docs/INDEX.md), [plan i wizja](docs/PLAN.md), [milestones](docs/MILESTONES.md), [architektura](docs/ARCHITECTURE.md), [frontend](frontend/README.md), [walidacja](docs/testing.md). Zasady dla agentów: [AGENTS.md](AGENTS.md).
