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

Strona: http://localhost:18430 (musi odpowiadać `AUTH_ORIGIN`, także dla passkey). Rust API: `127.0.0.1:18432`, dostępne przez proxy `/api`. Backend czyta jeden lokalny `.env`; przygotowanie SOPS i hooka opisuje [README projektu](../README.md) i [instrukcja sekretów](../docs/secrets.md). Panele wymagają działającego API i sesji serwerowej; nie mają fikcyjnego fallbacku.

## Mapa kodu i ekranów

- `src/App.svelte` — dynamicznie ładuje storefront albo workspace.
- `admin.html`, `src/admin/` — panel Piotra: rzeczywiste zamówienia, dozwolone statusy, chat, pliki, CSV i odczyt płatności. Wymaga hasła albo passkey oraz email/TOTP. [Zakres](src/admin/README.md).
- `src/storefront/Storefront.svelte` — landing i konfigurator Piotra na `/` i `/wycena`: wiele części, materiał/kolor, jakość, wytrzymałość, liczba sztuk i animacja przejścia do zamówienia.
- `src/storefront/lib/api.ts` — łączy nowy konfigurator z istniejącym adapterem `src/lib/api.ts`, analizą serwera, wyceną i koszykiem. Katalog pokazuje PLA/ABS/TPU oraz ich kolory, niezależnie od magazynu.
- `src/Workspace.svelte`, `src/pages/`, `src/components/` — checkout `/zamowienie`, logowanie, konto, admin oraz konfiguracja techniczna `/wycena/zaawansowana`. Powrót `/wycena?restore=cart` zachowuje opcje koszyka.
- `src/lib/types.ts`, `api.ts`, `mock.ts` — wspólne dane, adapter i lokalne szkice koszyka. `session.ts`/`http.ts` obsługują logowanie/CSRF, `orders.ts` trwałe zamówienia, pliki, chat i powiadomienia.
- `src/storefront/lib/stl.worker.ts`, `stlParser.ts`, `partViewer.ts` — parser STL w workerze przeglądarki i podgląd w natywnym WebGL. STL/STEP/3MF analizuje serwer; ograniczony `glbParser.ts` czyta jego prywatny GLB w tym samym workerze. Biały podgląd działa przed wyborem koloru; `ModelPreview.svelte` pokazuje pliki zamówienia.
- `src/storefront/theme.css` — tokeny projektu Piotra; `src/theme.css` mapuje je na checkout i panele. `storefront/app.css` / `transitions.css` i `app.css` / `workspace.css` odpowiadają za układ.

## Granice POC

Przepływ: **Druk → dane i dostawa → podsumowanie → płatność**. Adres i kontakt wpisujemy raz. Stripe Address Element udostępnia podpowiedzi, Payment Element płatność kartą testową; Payment Element działa na naszej stronie dla kuriera, punktu i odbioru osobistego; adres domowy jest potrzebny tylko kurierowi. Oferty Furgonetki sandbox i punkty odbioru pochodzą z API. Brak oferty nie oznacza darmowej dostawy ani automatycznie błędnego adresu. Panel `/admin.html` ma zakładki Dostawa i Rozliczenie: paczki, koszt, nadanie, etykiety, podjazd/anulowanie oraz zwroty częściowe i pełne. Operacje wymagają jawnego działania operatora, a niepewne wyniki zachowują ten sam request_id.

Analiza serwera obsługuje STL/STEP/3MF do 32 MiB, z `bubblewrap` i `prlimit`. STL w mm; STEP pojedynczej części w mm, bez złożeń. Wymiary pochodzą z geometrii, masę i czas mierzy Bambu Studio; pakowanie dostaje ostrożny szacunek z całego zużycia filamentu na sztukę. Niepewne analizy i przykłady bez analizy wymagają ręcznej oceny; nie otrzymują automatycznej płatności.

Katalog i kolory pochodzą z PostgreSQL. Ceny ustala slicer i cennik operatora; przygotowanie raz na koszyk, opakowanie i dostawa osobno. Brak kosztu zakupu lub dopuszczonego profilu oznacza ręczną ocenę; stan magazynu nie blokuje wyboru ani wyceny. Panel materiałów i cennika zapisuje API, ze wspólną sesją hasło/passkey + email/TOTP i CSRF. Konta klientów, chat i panele zamówień korzystają z trwałego API. Nie ma skalibrowanej oferty sprzedażowej.

Kontakt checkoutu pozostaje szkicem w `sessionStorage`, koszyk lokalnie; panel konta i admina czyta PostgreSQL. Lokalna karta powrotu Stripe nie jest źródłem listy zamówień ani uprawnień. Potwierdzenie płatności należy do serwera. Oferty, próby płatności i zamówienia są trwałe; oryginały plików podlegają retencji backendu. POC używa tylko sandboxów. Ręczne zgłoszenia i oferty mają trwałe API: admin ustala całość i zakres, klient dostaje email i płaci w swoim zamówieniu. Po rozpoczęciu płatności cena jest nieedytowalna. Nieobsługiwane faktury i konto/adresy nie zapisują fikcyjnych zmian. Stan: [PLAN.md](../docs/PLAN.md).

## Krótkie sprawdzenie

`npm --prefix frontend run build` uruchamia svelte-check i Vite build. Po zmianie wyglądu obejrzyj zmieniony ekran na komputerze i telefonie. POC: bez obowiązkowego pełnego E2E przy każdej iteracji.

Wybrane E2E: `npm --prefix frontend run test:e2e`, Playwright/Chromium na osobnym porcie 18431. Chromium: `npx playwright install chromium` z tego katalogu albo `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium`. Wyniki: ignorowane `tests/artifacts/`. Formatowanie: `npm --prefix frontend run format:check`. Panel scenariuszy demo usunięto z workspace.

## Nowe formularze operatora

`src/admin/Shipment.svelte` + `lib/shipping.ts`: odczyt snapshotu, rzeczywiste paczki, aktualna oferta z jawną akceptacją kosztu, nadanie, PDF, potwierdzenie przekazania, wspólny termin podjazdu i anulowanie zaznaczonych przesyłek. Brak automatycznego nadawania/podjazdów. Refresh pokazuje zapisany stan; 202 oznacza przyjęte zlecenie, nie sukces przewoźnika. Niepewne komendy zachowują request_id/payload w sessionStorage (bez adresów/sekretów). Stany techniczne manual_review/reconcile/reopen wymagają nadal procedury operatora.

`src/admin/Refunds.svelte` + `lib/refunds.ts`: kwoty w groszach, częściowy/pozostały zwrot, rezerwacje i lista stanów. Niepewne zlecenie blokuje zmianę kwoty i zachowuje request_id po przeładowaniu. Bieżące zwroty odświeżają się co 30 s, najwyżej 10 razy podczas otwartego widoku; dalej ręczne Odśwież.

`components/Passkeys.svelte` + `lib/passkeys.ts`: natywny WebAuthn dla klienta/admina, zapasowe klucze, usuwanie i świeże MFA. Ostatni klucz można usunąć tylko mając hasło. Lokalny bootstrap jest potrzebny do nadania pierwszej roli admina, nie do zwykłego klucza klienta. `AccountSecurity.svelte` obsługuje hasło, TOTP, jednorazowe kody odzyskiwania i logout-all; `Login.svelte` wymaga drugiego składnika po haśle/passkey. Dane zabezpieczeń nie trafiają do URL ani browser storage. [Kontrakt](../docs/auth.md).

Rzeczywiste testy całej strony: [instrukcja](../docs/testing.md#finalizacja-f0f6-rzeczywisty-backend). `tests/run-integration.py` tworzy odizolowaną bazę i pliki; `--sandbox` jawnie włącza Stripe test, Furgonetkę sandbox i email do void. Nigdy nie używa normalnej bazy do fixture.
