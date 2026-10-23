# Rapidmaker — frontend

Svelte 5 + TypeScript + Vite, Tailwind CSS 4, GSAP i ikony Tabler. Motyw Piotra: Manrope/Oxanium, limonkowe akcenty, jasny/ciemny wariant konfiguratora. Fonty są dostarczane przez npm, bez pobierania ich z Google.

## Uruchomienie

Z katalogu repo:

```sh
npm --prefix frontend ci
cargo build --locked --manifest-path backend/Cargo.toml
python backend/run.py
```

W drugim terminalu:

```sh
npm --prefix frontend run dev
```

Strona: http://127.0.0.1:18430. Rust API: `127.0.0.1:18432`, dostępne przez proxy `/api`. Backend czyta jeden lokalny `.env`; przygotowanie SOPS i hooka opisuje [README projektu](../README.md) i [instrukcja sekretów](../docs/secrets.md). Panele z mockami można rozwijać bez API; analiza, wycena i checkout wymagają serwera i danych sandboxów.

## Mapa kodu i ekranów

- `src/App.svelte` — dynamicznie ładuje storefront albo workspace.
- `src/storefront/Storefront.svelte` — landing i konfigurator Piotra na `/` i `/wycena`: wiele części, materiał/kolor, jakość, wytrzymałość, liczba sztuk i animacja przejścia do zamówienia.
- `src/storefront/lib/api.ts` — łączy nowy konfigurator z istniejącym adapterem `src/lib/api.ts`, analizą serwera, wyceną i koszykiem. Katalog zachowuje zgodność materiałów i dostępność kolorów.
- `src/Workspace.svelte`, `src/pages/`, `src/components/` — checkout `/zamowienie`, logowanie, konto, admin oraz konfiguracja techniczna `/wycena/zaawansowana`. Powrót `/wycena?restore=cart` zachowuje opcje koszyka.
- `src/lib/types.ts`, `api.ts`, `mock.ts` — wspólne dane, adapter i lokalne szkice koszyka oraz symulacje kont, chatu i zamówień.
- `src/storefront/lib/stl.worker.ts`, `stlParser.ts`, `partViewer.ts` — parser STL w workerze przeglądarki i podgląd w natywnym WebGL. STEP/3MF analizuje serwer; ich podgląd oraz viewer GLB/Three.js pozostają planem.
- `src/storefront/theme.css` — tokeny projektu Piotra; `src/theme.css` mapuje je na checkout i panele. `storefront/app.css` / `transitions.css` i `app.css` / `workspace.css` odpowiadają za układ.

## Granice POC

Przepływ: **Druk → dane i dostawa → podsumowanie → płatność**. Adres i kontakt wpisujemy raz. Stripe Address Element udostępnia podpowiedzi, Payment Element płatność kartą testową; pozostałe ścieżki używają hosted Checkout. Oferty Furgonetki sandbox i punkty odbioru pochodzą z API. Brak oferty nie oznacza darmowej dostawy ani automatycznie błędnego adresu. Aplikacja nie nadaje przesyłek.

Analiza serwera obsługuje STL/STEP/3MF do 32 MiB, z `bubblewrap` i `prlimit`. STL w mm; STEP pojedynczej części w mm, bez złożeń. Wymiary pochodzą z geometrii, masę i czas mierzy Bambu Studio; pakowanie dostaje ostrożny szacunek z całego zużycia filamentu na sztukę. Niepewne analizy i przykłady bez analizy wymagają ręcznej oceny; nie otrzymują automatycznej płatności.

Katalog i kolory pochodzą z PostgreSQL. Ceny ustala slicer i cennik operatora; przygotowanie raz na koszyk, opakowanie i dostawa osobno. Brak kosztu zakupu, dopuszczenia lub magazynu oznacza ręczną ocenę. Panel materiałów i cennika zapisuje API, z osobną sesją operatora. Konta klientów, chat i panele zamówień nadal są prototypami. Nie ma skalibrowanej oferty sprzedażowej.

Kontakt pozostaje w `sessionStorage`, koszyk i zamówienia w lokalnym stanie przeglądarki. Powrót ze Stripe i „Sprawdź płatność” porównują status, ID wyceny i kwotę z API. PostgreSQL zachowuje modele/metadane i snapshoty; prywatne pliki wygasają po 24 h, wyceny po 10 min. Restart usuwa ofertę dostawy i płatniczy stan RAM, wymaga nowej oferty. Produkcyjnych webhooków/trwałych zamówień jeszcze nie ma. Nie używaj kluczy live ani danych klientów. Stan prac: [PLAN.md](../docs/PLAN.md).

## Krótkie sprawdzenie

`npm --prefix frontend run build` uruchamia svelte-check i Vite build. Po zmianie wyglądu obejrzyj zmieniony ekran na komputerze i telefonie. POC: bez obowiązkowego pełnego E2E przy każdej iteracji.

Wybrane E2E: `npm --prefix frontend run test:e2e`, Playwright/Chromium na osobnym porcie 18431. Chromium: `npx playwright install chromium` z tego katalogu albo `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium`. Wyniki: ignorowane `tests/artifacts/`. Formatowanie: `npm --prefix frontend run format:check`. Narzędzia scenariuszy workspace są dostępne przez `?devtools=1`.
