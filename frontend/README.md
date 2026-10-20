# Rapidmaker — frontend i lokalne demo checkoutu

Z katalogu repo: `npm --prefix frontend ci`. Odszyfruj `secrets/dev.enc.env` do `.env` zgodnie z [instrukcją SOPS](../docs/secrets.md). Następnie `cargo build --locked --manifest-path backend/Cargo.toml`, `python backend/run.py` w jednym terminalu oraz `npm --prefix frontend run dev` w drugim. Frontend działa pod http://127.0.0.1:18430; API tylko pod 127.0.0.1:18432. Build frontendu: `npm --prefix frontend run build`.

Svelte 5 + TypeScript + Vite, Tailwind CSS i wybrany komponent [shadcn-svelte Badge](https://www.shadcn-svelte.com/docs/components/badge) (adaptacja MIT). Wszystkie ekrany z briefu: landing, logowanie, wycena, checkout, konto klienta i panel admina.

- `src/pages/` — widoki według funkcji, `App.svelte` — układ i routing.
- `src/components/` — wspólny chat, szczegóły zamówienia, statusy i elementy UI.
- `src/lib/types.ts` — dane; `api.ts` — jedyne wejście widoków do danych; `mock.ts` — lokalne symulacje konta, paneli, katalogu i zamówień. Wycena paczek, oferty przewoźnika i sesja Stripe testowa przechodzą przez adapter do lokalnego Rust API.
- `src/theme.css` — kolory, zaokrąglenia i fonty; `app.css` / `workspace.css` — wspólne style. Bez zewnętrznych fontów i silnika 3D.

Testy: `cargo test --locked --manifest-path backend/Cargo.toml` sprawdza algorytm pakowania i granice dostawców; `npm --prefix frontend run test:e2e` buduje aplikację i uruchamia Playwright na osobnym lokalnym podglądzie 18431. Wymagany Chromium: `npx playwright install chromium` w tym katalogu lub własna przeglądarka przez `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium npm --prefix frontend run test:e2e`. Screenshoty i ślady są w ignorowanym `tests/artifacts/`. Formatowanie: `npm run format:check` w tym katalogu.

Lokalne Rust API szacuje pakowanie na podstawie dostępnych wymiarów i masy modeli; analizuje pliki, a checkout nie prosi klienta o ręczne wymiary. Dostawa kurierska używa wyłącznie skonfigurowanego sandboxa Furgonetki, a brak oferty nie staje się darmową dostawą. Odbiór osobisty nie odpytuje przewoźnika; Paczkomat InPost wybieramy z rzeczywistej listy Furgonetki; punkt i paczka są walidowane. POC: jedna paczka, większe partie kurierem lub do odbioru. Stripe tworzy i odczytuje tylko sesje testowe. Nie tworzymy przesyłek, zamówień produkcyjnych ani webhooków.

Taryfa serwera to stałe 24 zł za sztukę i 15 zł przygotowania, niezależne od slicera, materiału, energii i rentowności; koszt kartonu jest doliczany raz. To kwoty testowe, nie oferta sprzedażowa. Konta, katalog, statusy i panele nadal działają na mockach. Modele są analizowane przez lokalny Rust API; PDF nadal przekazuje tylko metadane. Nie używaj kluczy live ani danych klientów. Stopień realizacji i wyniki weryfikacji: główny `PLAN.md`.

Obecnie tryb POC: build i krótki check zmienionej funkcji, bez obowiązkowych review i pełnego E2E. Narzędzia scenariuszy są ukryte; otwórz stronę z `?devtools=1`, jeśli są potrzebne. Ustawienia roboczego cennika są zapisywane lokalnie, nie zmieniają taryfy Rust API.

Checkout zachowuje kontakt w `sessionStorage`; przed przejściem do Stripe zapisuje lokalne zamówienie z ceną, przewoźnikiem i planem paczek. Powrót oraz przycisk „Sprawdź płatność” w panelu porównują status z API, ID wyceny i kwotę; aktualizują ten sam rekord klienta/admina. Gość może przypisać go przez lokalne demo rejestracji. Ponowienie prowadzi do tej samej sesji Stripe, a po anulowaniu dostępny jest link do rozpoczętej płatności. Dla takich zamówień nie ma pozornych płatności, zwrotów ani etykiet. `ParcelPlan.svelte` pokazuje listę i zawartość paczek; `PaymentReturn.svelte` wynik płatności. Lokalny restart API nadal usuwa wyceny i sesje — bez trwałego magazynu lub webhooka.

Kreator: **Druk → Wysyłka → Podsumowanie → Płatność**, z paskiem etapów. Druk: lista części i edycja jednej naraz, materiał/kolor, ilustrowane warianty warstw i budowy; własne parametry opcjonalne. Bez adresu i wymiarów pakowania. Wysyłka: kontakt, odbiór/przewoźnik/punkt i automatyczny plan paczek. Podsumowanie: części, kontakt, dostawa i pełna cena z możliwością edycji. Dopiero ostatni przycisk zatwierdza płatność Stripe. Kurier: wspólny Address Element + Payment Element; pozostałe metody: hosted Checkout. Krok druku oblicza samą taryfę bez odpytywania przewoźnika i bez zmiany zapisanej metody dostawy.

Analiza: do 32 MiB, STL w mm, STEP pojedynczej części w mm (bez złożeń), 3MF. Wymaga `bubblewrap` i `prlimit` (util-linux). Wymiary pochodzą z geometrii; masa to ostrożny szacunek pełnej bryły 2 g/cm³, nie slicer. Brak wiarygodnej masy oznacza ręczną ocenę. Analizy: 24 h / 128 modeli w RAM, restart wymaga ponownego uploadu; pliki nie są jeszcze trwale przechowywane.

Stripe Elements wymaga `STRIPE_PUBLISHABLE_KEY=pk_test_…` w `.env`; sekret pozostaje na serwerze. Wspólna grupa Address/Payment udostępnia autocomplete Stripe bez własnego klucza Google; ręczny adres działa również bez sugestii. Potwierdzenie płatności kartą testową sprawdzone; pojawienie się rzeczywistych sugestii adresów nie zostało potwierdzone w lokalnej przeglądarce.

Checkout: najpierw jeden pionowy formularz odbiorcy (email i telefon bez maski + adres z podpowiedziami Stripe), potem dostawa. Oferty pobierają się automatycznie po uzupełnieniu danych, bez osobnego przycisku wyceny. Jeden kod pocztowy zasila adres i automatyczną listę pobliskich Paczkomatów; osobne wyszukiwanie tylko na życzenie. Zmiana kodu unieważnia wybrany punkt.
