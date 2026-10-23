# Rapidmaker.pl — brief frontendu

Aktualizacja B1–B5: katalog, panel materiałów/cennika, upload, wycena i checkout korzystają już z Rust/PostgreSQL/slicera. Konta klientów, zamówienia i chat nadal są prototypami. Poniższy brief opisuje pierwotny zakres wizualny; bieżący kontrakt i granice mają pierwszeństwo: [architektura](ARCHITECTURE.md), [backend](../backend/README.md). Nie odtwarzaj stałej taryfy ani autoryzacji admina z mocków.


## Cel i zakres

Zaprojektuj frontend z lokalnymi mockami dla konta, paneli, katalogu i zamówień. Do wyceny dostawy i płatności użyj oddzielnego, lokalnego Rust API, które łączy się wyłącznie z sandboxami Furgonetki i Stripe. To ograniczone demo checkoutu, nie gotowy backend sklepu.

- **Konto na niby:** lokalnie symuluj utworzenie konta klienta przez email, logowanie klienta lub admina i wylogowanie. Bez prawdziwego uwierzytelniania i MFA.
- **Dane i akcje:** mockuj konto, zamówienia, statusy i chat. Upload wysyła model do lokalnego API analizy geometrii (32 MiB); podgląd pozostaje placeholderem. Checkout woła lokalny Rust API dla szacowania paczek, stałej taryfy testowej, ofert sandboxa Furgonetki i Stripe testowego (Elements dla kuriera, Checkout dla pozostałych metod). Nie twórz przesyłek ani zamówień produkcyjnych.
- **Podgląd modelu:** przygotuj pole z przykładową grafiką lub komunikatem „Miejsce na podgląd 3D”. Nie implementuj silnika 3D ani odczytu modeli.
- Opisy poniżej określają widoki i ich docelowe zachowanie. Na tym etapie pokazujemy je na przykładowych danych.

Interfejs ma być prosty (KISS), nowoczesny i z własnym zielonym motywem. Nowy kierunek na prośbę użytkownika: prawie czarne tło, czytelna biała typografia i oszczędne zielone akcenty Rapidmaker. Bez szkła, cieni i nadmiaru kart. Sekcje rozdzielaj liniami i odstępami; preferuj listy, a szczegóły techniczne zwijaj. Poniżej opisano widoki oraz funkcje, które docelowo obsłuży backend; bieżące zachowanie prototypu opisuje `frontend/README.md`.

## Lokalne dummy API — łatwe podłączenie backendu później

**Skup się na wyglądzie i obsłudze strony.** Dane kont, paneli i zamówień pozostają w kilku funkcjach mock działających w przeglądarce. Checkout ma wąski, gotowy lokalny kontrakt Rust opisany w README prototypu; nie rozbudowuj go do serwera sklepu ani produkcyjnego algorytmu wyceny.

- `src/lib/types.ts` określa typy danych i argumentów potrzebnych widokom, np. zamówienie, opcje wydruku i wynik wyceny. To wymagania frontendu, nie projekt backendu.
- `src/lib/mock.ts` zawiera przykładowe dane oraz funkcje asynchroniczne do kont, katalogu, zamówień i paneli. Stan tych elementów jest lokalny.
- `src/lib/api.ts` jest jedynym wejściem widoków do danych. Przekazuje większość wywołań do `mock.ts`, a zapytania checkoutu do lokalnego Rust API. Komponenty nie wykonują własnych zapytań `fetch`.
- Dane zamówień, klientów i cen trzymaj w mockach, nie w komponentach. Stałe teksty interfejsu, np. „Moje zamówienia”, mogą pozostać w widokach.
- Zapewnij proste scenariusze do obejrzenia UI: ładowanie, sukces, pusta lista i błąd. Symuluj tylko tyle zachowania, ile potrzeba do pokazania ekranów; bez rozbudowanej infrastruktury mockowania.
- Dalszy backend podłączaj przez `api.ts`. Zachowaj typy i sposób przekazywania błędów do widoków; jeśli kontrakt serwera się różni, dopasuj go w tym module.

**Cel:** podłączenie backendu powinno wymagać głównie zmiany `api.ts`, bez przepisywania komponentów. Nowe wymagania rzeczywistego API mogą wymagać dopasowania interfejsu; na tym etapie nie implementuj integracji na zapas.

## Kierunek wizualny i jego zmiana

Przed rozpoczęciem ekranów ustal styl z użytkownikiem. Uzgodniony kierunek jest już przyjęty, więc stosuj tokeny z `src/theme.css`; pytaj ponownie tylko, jeśli proponujesz istotną zmianę wizualną.

Przygotuj małą próbkę: przycisk, pole formularza i kartę. Pokaż też stany przycisku: zwykły, po najechaniu, fokus klawiatury i wyłączony. Dopiero po akceptacji użytkownika zastosuj ten styl na podstronach.

### Łatwa zmiana stylu

- W `src/theme.css` trzymaj wspólne ustawienia jako zmienne CSS: kolory, fonty, odstępy, zaokrąglenia, cienie, przezroczystość i rozmycie. To jedno miejsce do zmiany motywu.
- `src/app.css` importuje motyw i zawiera style bazowe. Tailwind i komponenty shadcn-svelte korzystają z tych samych zmiennych.
- Przyciski, pola i karty korzystają ze wspólnych komponentów w `src/components/ui/`. Nie kopiuj ich stylów na każdej podstronie.
- W komponentach ekranów definiuj układ. Nie wpisuj na sztywno wartości motywu, które mają być wspólne.
- Zmiana wspólnego koloru lub zaokrąglenia ma działać na całej stronie bez poprawiania każdego widoku osobno.

### Prostota i ocena wizualna

- Ogranicz ilość kodu, zachowując czytelność. Używaj wspólnych komponentów i prostych funkcji, bez zbędnych warstw i zależności.
- Każdy ekran dopasuj do telefonu i komputera. Zadbaj szczególnie o formularze, tabele i chat, czytelny kontrast, etykiety i obsługę klawiaturą.
- Obejrzyj działającą stronę i screenshoty na obu rozmiarach. Popraw zauważone problemy; samo wykonanie zrzutu lub zielony test nie oznacza, że UI jest dobre.
- Sprawdzaj zmieniony ekran i ważne przepływy. Nie uruchamiaj całej baterii testów po zmianie koloru lub zaokrąglenia.

## Lista stron do przygotowania

| Adres | Strona | Zawartość |
| --- | --- | --- |
| `/` | Strona główna | Wizytówka, rozpoczęcie wyceny i wejście na konto. |
| `/logowanie` | Logowanie i utworzenie konta | Wspólne wejście przez email; opcjonalne utworzenie konta klienta. |
| `/wycena` | Konfigurator | Dodawanie modeli, pole podglądu, opcje wydruku i wycena. |
| `/zamowienie` | Koszyk i checkout | Zakup bez konta, podsumowanie, dane klienta, dostawa i wynik symulowanej płatności. |
| `/konto/zamowienia` | Zamówienia klienta | Lista zamówień i ich statusy. |
| `/konto/zamowienia/:id` | Zamówienie klienta | Szczegóły, postęp, chat, płatność, dostawa i ponowienie zamówienia. |
| `/konto/ustawienia` | Ustawienia klienta | Dane kontaktowe, adresy, faktura, powiadomienia i usunięcie konta. |
| `/admin/zamowienia` | Zamówienia admina | Lista, wyszukiwanie, filtry i sprawy wymagające reakcji. |
| `/admin/zamowienia/:id` | Obsługa zamówienia | Modele, wycena, historia, chat, płatności i wysyłka. |
| `/admin/produkcja` | Produkcja | Kolejka, statusy, terminy i prywatne notatki. |
| `/admin/materialy` | Materiały i ceny | Filamenty, kolory, dostępność, koszty i parametry wyceny. |
| `/admin/ustawienia` | Ustawienia admina | Ustawienia panelu i stan systemu z opisanych poniżej funkcji. |

`:id` oznacza identyfikator zamówienia. Chat, płatności i wysyłka są częściami szczegółów zamówienia, bez osobnych podstron.

### Wspólne logowanie i opcjonalne konto przez email

- Jeden adres `/logowanie` i wspólne wejście przez email. Użytkownik nie wybiera roli; nie ma osobnego formularza logowania admina.
- Klient może wybrać „Utwórz konto”, podać email i potwierdzić go linkiem. Na makiecie pokaż komunikat o wysłaniu linku oraz przycisk „Symuluj potwierdzenie emaila”; niczego faktycznie nie wysyłaj.
- Po potwierdzeniu mock tworzy konto z rolą `client`. Dla istniejącego konta symuluj logowanie linkiem z emaila, bez hasła. Ponowne wysłanie linku służy też odzyskaniu dostępu. Rejestracja nigdy nie nadaje roli admina.
- Na potrzeby makiety przygotuj dwa fikcyjne konta: klienta i admina. Pokaż ich testowe adresy email przy formularzu, aby łatwo sprawdzić oba panele.
- Formularz wywołuje `api.ts`; po symulowanym potwierdzeniu mock zwraca użytkownika z rolą `client` lub `admin`. Nie rozpoznawaj roli w komponencie po adresie email.
- Po zalogowaniu klient trafia na `/konto/zamowienia`, a admin na `/admin/zamowienia`. Przycisk „Moje konto” prowadzi do odpowiedniego panelu.
- Zasymuluj błędny email, wygasły link, ładowanie i wylogowanie. Przy wejściu na adres panelu bez logowania pokaż formularz logowania; klient nie powinien widzieć panelu admina.
- To wyłącznie symulacja interfejsu. Docelowo backend uwierzytelni użytkownika, zwróci rolę i sprawdzi uprawnienia do każdej chronionej operacji. Konta testowe nie mogą służyć do prawdziwego logowania.

## 1. Strona główna

- Landing page — wizytówka wzorowana na obecnej rapidmaker.pl.
- Przycisk „Zaloguj się”, a po zalogowaniu „Moje konto”.
- Widoczne miejsce rozpoczęcia wyceny: przycisk prowadzący do konfiguratora, a następnie zakupu.

## 2. Wycena i opcje wydruku

Wycena i zakup są dostępne bez logowania. Konto jest opcjonalne.

- **Pliki:** dodawanie kilku modeli STL, STEP lub 3MF przez przeciągnięcie pliku; postęp przetwarzania i czytelne błędy.
- **Pole podglądu 3D:** miejsce na model oraz przyciski obrotu, zoomu i resetu — na razie bez działania silnika 3D. Wymiary i liczba części są przykładowe. Dla STL pokaż wybór jednostek.
- **Materiał i kolor:** docelowo cały wspierany katalog Bambu, z wyszukiwaniem, krótkimi opisami i próbkami kolorów. Do makiety użyj kilku realistycznych przykładów, bez pobierania katalogu. Rozróżnij zgodność z drukarką i dostępność magazynową; do zamówienia pozwalaj wybrać tylko kombinacje możliwe do realizacji.
- **Jakość:** proste, opisane warianty, np. „Standard” i „Dokładny”.
- **Wytrzymałość:** proste warianty domyślne, które pokazują przypisane ustawienia ścian i wypełnienia. Nie obiecuj wytrzymałości na podstawie samego procentu wypełnienia.
- **Liczba sztuk:** osobno dla każdego modelu; przeliczenie ceny całej partii.
- **Uwagi:** np. ważne wymiary lub powierzchnie. Specjalne wymagania mogą skierować zamówienie do ręcznej wyceny.
- **Dostawa lub odbiór osobisty:** szacowana liczba paczek i koszt po podaniu potrzebnych danych.
- **Podsumowanie:** cena za sztukę dla każdego modelu, cena partii, dostawa, łączna kwota i przewidywany termin. Po zmianie opcji pokaż przeliczanie i oznacz poprzednią cenę jako nieaktualną. Opóźniony wynik nie może nadpisać nowszego; zatwierdzenie zamówienia wymaga aktualnej wyceny.
- **Ręczna wycena:** dla dużego lub nietypowego zamówienia pokaż „Wymagana ręczna wycena” i dalszy krok zamiast płatności. Dotyczy to także niepewnego kosztu wysyłki. Wyniki i kwoty pochodzą z mocków.

### Opcje dokładniejszej wyceny FDM

Użyj wzorców konfiguracji znanych z [Xometry](https://xometry.eu/en/fdm-3d-printing/) i [JLC3DP](https://jlc3dp.com/help/article/how-to-place-a-3d-printing-order) jako inspiracji do jasnego formularza. To wymagania Rapidmaker, nie obietnica, że każda opcja tych firm jest dostępna lub ma być skopiowana.

- Zostaw proste ustawienia na wierzchu: materiał, kolor, jakość / wysokość warstwy, liczba sztuk. Pozostałe ustawienia umieść w zwijanej sekcji „Opcje zaawansowane”.
- Pokaż w niej: liczbę ścian, procent i wzór wypełnienia, orientację modelu, podpory, wykończenie powierzchni, pasowanie, liczbę i rozmiar insertów, termin standardowy lub ekspresowy oraz opcjonalny rysunek techniczny i uwagi.
- Każda opcja ma krótki opis prostym językiem. Pokaż, które ustawienia zmieniają cenę lub termin, i odśwież podsumowanie po zmianie. Jeśli wybór wymaga oceny operatora, oznacz cenę jako nieważną do czasu ręcznej oferty.
- Zależności i niedostępne warianty pokaż przy polu. Nie zakładaj, że podpory rozpuszczalne, wykończenie, ścisłe pasowanie, inserty, ekspres ani dowolna orientacja są wykonalne dla każdego materiału i modelu.
- Nietypowe wymagania, tolerancje, widoczne powierzchnie, wkładki, dopasowanie do innych części, duży nakład lub ręcznie dobrana orientacja prowadzą do przeglądu przez operatora, dopóki możliwości i cena nie zostaną potwierdzone.
- Wszystkie wyceny prototypu są przykładowe. Interfejs nie może sugerować rentowności, gwarantowanych tolerancji ani potwierdzonej wykonalności.

## 3. Koszyk i płatność

Checkout używa lokalnego Rust demo API i sandboxów dostawcy/płatności. Zalogowany użytkownik nie jest wymagany; dane wprowadzane są tylko na potrzeby testowej oferty.

- **Bez obowiązkowego konta:** zakup jako gość. Po zamówieniu można wybrać „Utwórz konto” i przejść przez opisaną symulację potwierdzenia emaila.
- **Kilka paczek:** Rust szacuje rozmieszczenie z wpisanych przez klienta wymiarów i masy. Pokaż każdą paczkę, koszt kartonów raz oraz jedną łączną dostawę. Brak wymiarów, niepewny plan lub brak oferty kieruje do ręcznej oceny.
- **Wynik płatności:** Stripe Checkout testowy tworzy sesję i jej stan jest ponownie odczytywany z API. Wyraźnie pokaż anulowanie, oczekiwanie i potwierdzenie testowe; nie twórz produkcyjnego zamówienia.
- **Potwierdzenie:** informacja o stanie sesji Stripe i dalszym kroku. To stany testowego checkoutu, bez udawania realizacji zamówienia.

Zakres lokalnego demo:

- **Stripe Checkout:** wyłącznie testowa sesja płatności; bez webhooków, trwałego zamówienia i kluczy live.
- **Furgonetka API:** testowe pobranie usług, wycena i walidacja ofert kurierskich. Nie zamawiamy ani nie drukujemy etykiet. Paczkomaty czekają na walidację konkretnego punktu; odbiór osobisty nie odpytuje przewoźnika.
- **Pakowanie:** szacowanie z wymiarów i masy wpisanych przez użytkownika, nie z geometrii pliku. To plan testowy; fizyczne kartony, masa i zabezpieczenia wymagają kalibracji.

Cena pochodzi ze slicera i konfiguracji operatora: koszty części, jedno przygotowanie, straty, marża i VAT. Opakowania i dostawa oddzielnie; brak konfiguracji nie może oznaczać wymyślonej ceny ani darmowej dostawy.

Symulacje pozwalają przygotować frontend. Integracje uznajemy za gotowe dopiero po testach z dostawcami.

## 4. Panel klienta

- **Moje zamówienia:** lista z numerem, datą, kwotą i statusem.
- **Szczegóły zamówienia:** podgląd modeli, materiał, kolor, liczba sztuk, cena i przewidywany termin.
- **Postęp:** oś „płatność → przygotowanie → druk → wysyłka lub odbiór”.
- **Chat przy zamówieniu:** rozmowa z obsługą i oznaczenie nowych wiadomości.
- **Płatność i dostawa:** opłacenie zamówienia, dokument sprzedaży, tracking lub informacje o odbiorze osobistym.
- **Ponowienie zamówienia:** te same modele i ustawienia, z ponownym sprawdzeniem dostępności i aktualnej ceny.
- **Ustawienia konta:** dane kontaktowe, adresy dostawy, dane firmy do faktury VAT (w tym NIP) oraz usunięcie konta z informacją o zachowywanych danych.
- **Powiadomienia email:** oddzielne ustawienia dla nowej wiadomości, zmiany statusu, wysyłki i gotowości do odbioru. Na makiecie zapisuj je lokalnie, bez wysyłania emaili.

## 5. Panel admina

Cztery zakładki: **Zamówienia · Produkcja · Materiały i ceny · Ustawienia**. Chat, płatności i wysyłka znajdują się wewnątrz zamówienia.

- **Zamówienia:** lista, wyszukiwanie, filtry statusów i oznaczenie spraw wymagających reakcji.
- **Szczegóły zamówienia:** modele i podgląd 3D, materiał, kolor, liczba sztuk, cena, dane klienta i historia zmian.
- **Produkcja:** kolejka wydruków, zmiana statusu, termin realizacji i prywatne notatki.
- **Chat:** rozmowa z klientem przy zamówieniu, nieprzeczytane wiadomości i powiadomienia email.
- **Wyceny:** akceptacja nietypowych zleceń i ręczna oferta z zapisanym powodem zmiany ceny.
- **Płatności:** status Stripe i zwroty. Opłacenie zamówienia potwierdza Stripe.
- **Wysyłka i odbiór:** paczki, etykiety Furgonetki, tracking i odbiór osobisty.
- **Materiały i ceny:** dostępne filamenty, kolory, koszty i parametry wyceny.
- **Stan systemu:** nieudane konwersje, wyceny i wiadomości email z możliwością bezpiecznego ponowienia.
- **Do ustalenia:** anulowanie zamówienia lub wysłanie zaktualizowanej oferty; później backend wyśle odpowiedni email.

Docelowo: logowanie admina z MFA (dodatkowym potwierdzeniem logowania), kontrola uprawnień w backendzie Rust i historia istotnych operacji.

## Technologie

### Frontend — prototyp

- **Svelte 5 + TypeScript:** interfejs.
- **Tailwind CSS + małe wspólne komponenty UI:** wygląd i własny zielony motyw. Dodawaj komponent z zewnętrznego zestawu tylko wtedy, gdy rzeczywiście upraszcza używany ekran.
- **Vite:** budowanie i lokalny podgląd strony w przeglądarce.
- **Pole podglądu:** zwykły komponent zastępczy, bez Three.js i ładowania GLB. Silnik podglądu zostanie podłączony później.

Frontend Svelte jest w `frontend/`; jego obecny stan i ograniczenia opisuje `frontend/README.md`. Starszy frontend usunięto; pozostaje w historii Git.

### Backend i usługi — kontekst do późniejszego podłączenia

- **Rust + Axum:** API.
- **PostgreSQL:** baza danych.
- **Izolowany worker:** przetwarzanie STL, STEP i 3MF na siatkę produkcyjną oraz GLB do przyszłego podglądu w Three.js.
- **Stripe Checkout:** płatności.
- **Furgonetka API:** wysyłka; dodatkowo odbiór osobisty.
- **iCloud:** email, testowo `void@sieciowiec.xyz`.

## Przykładowa struktura projektu

```text
frontend/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── public/
│   └── favicon.svg
├── src/
│   ├── main.ts                 # uruchomienie aplikacji
│   ├── App.svelte              # układ strony i routing
│   ├── theme.css               # wspólne ustawienia wyglądu
│   ├── app.css                 # import motywu i style bazowe
│   │
│   ├── lib/
│   │   ├── api.ts              # dostęp do danych; dziś mocki, później API
│   │   ├── mock.ts             # przykładowe dane i lokalne symulacje
│   │   ├── types.ts            # wspólne typy danych i argumentów
│   │   └── format.ts           # PLN, daty, jednostki
│   │
│   ├── components/
│   │   ├── ui/                # tylko używane elementy shadcn-svelte
│   │   ├── Header.svelte
│   │   ├── ModelViewer.svelte  # pole podglądu, bez silnika 3D
│   │   ├── OrderStatus.svelte
│   │   └── OrderChat.svelte    # wspólny dla klienta i admina
│   │
│   └── pages/
│       ├── Home.svelte
│       ├── Login.svelte         # wejście przez email i utworzenie konta
│       ├── configure/
│       │   ├── Configure.svelte
│       │   ├── ModelUpload.svelte
│       │   ├── PrintOptions.svelte
│       │   └── QuoteSummary.svelte
│       ├── checkout/
│       │   └── Checkout.svelte
│       ├── account/
│       │   ├── Orders.svelte
│       │   ├── Order.svelte
│       │   └── Settings.svelte
│       └── admin/
│           ├── Orders.svelte
│           ├── Order.svelte
│           ├── Production.svelte
│           ├── Materials.svelte
│           └── Settings.svelte
│
└── tests/
    └── e2e/
        ├── upload.spec.ts
        ├── checkout.spec.ts
        └── orders.spec.ts
```

## Bieżący POC

Priorytetem jest implementacja i wygląd; build i krótki check zamiast rozbudowanych testów/review. Ustawienia jako zwarte wiersze: etykieta po lewej, wartość po prawej. Bez dużych plakietek „Tryb lokalnego demo” i technicznych paneli na głównych ekranach; narzędzia developerskie tylko przez `?devtools=1`. Przy Stripe pozostaje krótkie oznaczenie płatności testowej.

POC dostawy: zwarta lista paczek z wymiarami, masą i rozwijaną zawartością; po wyborze Paczkomatu zwiń wyniki do jednego wiersza. Po powrocie ze Stripe pokaż status odczytany z API i kwotę; przy braku wpłaty umożliwiaj powrót do tej samej sesji. Kontakt zachowuj w sesji przeglądarki.

Kreator: **Druk → Wysyłka → Podsumowanie → Płatność**, z paskiem etapów. Druk: lista części i edycja jednej naraz, materiał/kolor, ilustrowane warianty warstw i budowy; własne parametry opcjonalne. Bez adresu i wymiarów pakowania. Wysyłka: kontakt, odbiór/przewoźnik/punkt i automatyczny plan paczek. Podsumowanie: części, kontakt, dostawa i pełna cena z możliwością edycji. Dopiero ostatni przycisk zatwierdza płatność Stripe. Dla kuriera adres Stripe jest w wysyłce, karta dopiero w kroku płatności po podsumowaniu; oba korzystają z jednej grupy Elements. Krok druku oblicza samą taryfę bez odpytywania przewoźnika i bez zmiany zapisanej metody dostawy.

Hierarchia tła: ciemne marginesy, jedna lekko jaśniejsza powierzchnia robocza konfiguratora/checkoutu (`work-surface`), zielone aktywne wybory. Kolor powierzchni jest w `src/theme.css`; nie dodawaj osobnego dużego panelu do każdej sekcji.
