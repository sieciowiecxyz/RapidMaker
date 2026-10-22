# Architektura Rapidmaker

## Zasady

Jeden backend Rust i PostgreSQL. Małe konkretne funkcje, jawne SQL, minimum zależności i warstw. `app → api → moduły`; logika obliczeń nie zna HTTP. Nie przenosimy całego kodu do routera. Zewnętrzne dane są niezaufane; granice uprawnień, kwot i zasobów pozostają obowiązkowe w POC.

## Faktyczna mapa

- `backend/crates/app`: start serwera na localhost:18432, start trybu workera geometrii.
- `api`: wspólny router, lokalna ochrona Host/Origin, upload, katalog, nowa wycena i stary checkout sandboxów. `checkout.rs` wciąż zawiera adaptery Stripe/Furgonetki.
- `storage`: SQLx/PostgreSQL; migracja `catalog_materials`, `catalog_colors`, `quotes`. Import transakcyjny zachowuje ceny i dostępność operatora. Snapshot wyniku wyceny jest niezmienny przez API.
- `slicer`: Bambu Studio w osobnym procesie bubblewrap/prlimit; P1P hardened 0,4 mm, Textured PEI, standard 0,20 mm; obecnie STL i profile PLA Basic/PETG HF. Nasze presety z `data/bambu`, rozwiązywane `inherits`/`include`. Bez sieci i sekretów, RAM 4 GiB, CPU 60 s, timeout 65 s, limit pliku 128 MiB, jedno zadanie naraz. Odczyt rzeczywistego czasu i zużycia filamentu.
- `pricing`: całkowitoliczbowe koszty netto w groszach, energia, maszyna, obsługa płyt, przygotowanie raz, rezerwa strat, marża, minimum i VAT. Stawki wymagają jawnej konfiguracji. Obecnie każda kopia ma osobną płytę; brak optymalizacji partii.
- `geometry`: analiza STL/STEP/3MF i GLB. Worker bez sieci/sekretów, 30 s/2 GiB. STEP ograniczony do pojedynczej części w mm. Masa z analizy jest oszacowaniem pełnej bryły, nie wynikiem slicera.
- `packing`: algorytm układania obwiedni z zapasem, 6 obrotów, katalog kartonów; do 2000 sztuk/30 paczek. Nie obiecuje minimum paczek. Furgonetka sandbox waliduje/wycenia usługi i punkty; aplikacja nie nadaje przesyłek.
- `orders`, `auth`, `payments`, `shipping`, `notifications`, `observability`: nadal szkielety bez własnej implementacji. Nie oznaczają gotowych kont, maila, webhooków ani monitoringu.

## Nowa wycena a istniejący checkout

`POST /api/quotes/estimate`: kontrolowany STL → pomiar slicera → koszt z ceny zakupu i skonfigurowanych stawek → snapshot PostgreSQL. Katalog: `GET /api/catalog/materials`. Cache pomiarów: SHA-256 modelu + materiał + wersja/hash profili, 64 wpisy/10 minut. Brak ceny, magazynu, zatwierdzenia albo poprawnego wyniku nie daje automatycznej oferty.

Nowy wynik jest POC i nie jest płatny. Nie ma jeszcze adaptera do checkoutu, wspólnych płyt, wyboru jakości ani siatki produkcyjnej STEP/3MF. Nowy endpoint usuwa oryginał po pomiarze; trwałe prywatne modele/zadania pozostają do wdrożenia.

Stary `/api/checkout/quote` nadal używa taryfy 24 zł/szt. + 15 zł przygotowania i kosztu kartonów. Dane geometrii, oferty i sesje Stripe są w ograniczonym RAM; restart je traci. Stripe jest testowy; kwota/status są sprawdzane na serwerze. Trwałe zamówienia, webhooki i przełączenie na nową wycenę są przed nami.

## Frontend i dane

`frontend/`: Svelte 5/TypeScript/Vite, Tailwind 4, GSAP, Manrope/Oxanium i limonkowe akcenty. `App.svelte` ładuje `storefront/Storefront.svelte` dla `/` i `/wycena`; `Workspace.svelte` dla checkoutu, konta, admina i wyceny zaawansowanej. STL: natywny WebGL i parser w workerze przeglądarki. STEP/3MF analizuje backend; pełny viewer GLB/Three.js pozostaje planem. GLB nie zastępuje oryginału ani siatki dla produkcji.

Frontend korzysta jeszcze ze starego checkoutu; katalog/panele/login/chat są mockami z localStorage. Nowy katalog PostgreSQL i wycena nie zostały jeszcze podłączone do UI. Zapisy admina wymagają prawdziwej autoryzacji backendu; rola z localStorage jej nie zastępuje.

Katalog Bambu pochodzi z lokalnego Studio, wersja 02.08.00.06: 49 materiałów/314 kolorów. Ceny presetów mają niepotwierdzoną walutę/VAT; ceny zakupu netto PLN, magazyn i dopuszczenie operatora pozostają osobne. Sama obecność profilu P1P nie kwalifikuje sprzętu.

## Uruchomienie i granice

[Backend: konfiguracja i kontrakt API](../backend/README.md). Lokalny PostgreSQL 16 na localhost:18434; API:18432, Vite:18430. Jedna `.env`, SOPS i hook przed commitem. Brak Dockerfile/Taskfile/Kubernetes i publicznego wdrożenia. iCloud SMTP/IMAP, trwałe zamówienia, role, logi/metryki Grafany są planem. Kolejność i dowody w [PLAN.md](PLAN.md) oraz [MILESTONES.md](MILESTONES.md).
