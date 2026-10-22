# Rapidmaker — przebudowa od zera

**Teraz: POC — więcej implementacji, mniej testów.** Najpierw działające funkcje i wygląd; rozbudowa backendu tylko wtedy, gdy jest potrzebna do bieżącej funkcji.

- Przy zmianach kodu: build i jeden krótki check zmienionej funkcji. Przy UI obejrzyj zmieniony widok; przy samych docs sprawdź diff.
- Bez obowiązkowych nowych testów, pełnych zestawów E2E, audytów, review Astrą ani subagentów do testów. Nie powtarzaj zielonych kontroli bez nowego powodu.
- Dodatkowy test tylko dla konkretnego błędu lub ryzyka zmiany. Zachowaj kontrolę kwot, uprawnień i sekretów.
- Pełna walidacja produkcyjna i pentesty są na później. Kryteria docelowe poniżej nie blokują iteracji POC; po krótkim sprawdzeniu przechodź do następnej funkcji.

Mode: goal
Active: M0

## Wizja produktu
**Rapidmaker — rapidmaker.pl:** polska usługa FDM, **wgraj model, zobacz cenę, zamów wydruk**. To skrót wizji scalonej z dawnych briefów, w tym `GOALS.md` zachowanego w historii Git. Upload na pierwszym ekranie, wycena bez konta; klient nie musi znać slicerów ani drukarek. Strona powstaje od zera, prosta i ciemna, z zieloną paletą repo [sieciowiecxyz/RapidMaker](https://github.com/sieciowiecxyz/RapidMaker); źródła i kolory w `ARCHITECTURE.md`. Jasna hierarchia, dobre ustawienia domyślne, krótki polski język i natychmiastowa informacja po akcji. Każdy ekran ma oczywisty następny krok; opcje zaawansowane są schowane. Przy zmianach UI rób screenshoty mobile/desktop, analizuj wygląd i obsługę, poprawiaj problemy i sprawdzaj ponownie.

Pierwsze wydanie:
- Prywatne **STL, STEP/STP i 3MF** → backend → siatka produkcyjna oraz osobny GLB → Three.js. Wszystkie trzy formaty przed startem; oryginał zachowany.
- Pełny katalog wspieranych filamentów **Bambu Lab i ich kolorów**. Oddziel katalog producenta od stanów magazynowych i zgodności z drukarką/dyszą; materiały podporowe nie są osobną ofertą wydruku. Prosty wybór materiału, koloru, jakości i ilości. Wytrzymałość nie jest obietnicą wynikającą z samego infill.
- Prawdziwy slicer i prosty, jawny algorytm wyceny: opłata startowa za faktyczne przygotowanie/partię, zużyty filament i odpady, energia, czas maszyny, praca operatora, opakowanie, opłaty i podatki oraz rozsądna dodatnia marża. Ilość uwzględnia wspólną płytę i kolejne partie, bez mnożenia całego startu przez liczbę sztuk. Rabaty i minimum nie mogą powodować strat. M3 porównuje koszt/cenę/zysk dla 1, 2, 10 i 100 sztuk, małych/dużych i wielomateriałowych modeli; wykrywa zbyt niskie ceny i nieuzasadnione narzuty. Cena sztuki i całości jest czytelna, dostawa osobno. Parametry i sensowność cen wymagają pomiarów oraz kalibracji; dane testowe nie potwierdzają rentowności sprzedaży.
- Koszyk, dostawa i checkout gościa, Stripe, potwierdzenia oraz status. **M3A po M3:** plan pakowania wskazuje liczbę paczek i zawartość każdej na podstawie wymiarów części, masy i zabezpieczeń. Backend dobiera wykonalne warianty do sposobu dostawy wybranego przez kupującego i sprawdza dostępność oraz cenę w Furgonetce. Duże/niepewne zamówienia trafiają do ręcznej wyceny przed płatnością; dostępny jest odbiór osobisty. Koszt opakowań trafia raz do końcowej ceny koszyka, przewóz osobno.
- **Konto i panel klienta:** proste logowanie emailem, własne zamówienia i statusy, chat z operatorem oraz ustawienia email. Powiadomienia o zmianie statusu i odpowiedzi operatora: natychmiast lub wyłączone, z jawnymi ustawieniami domyślnymi. Maile logowania/bezpieczeństwa i potwierdzenia zamówienia/płatności są odrębne.
- **Panel admina:** wspólna z klientem rozmowa przy zamówieniu, aktualizacja dozwolonych statusów realizacji, pliki, pakowanie, wyjątki i historia. Dostęp tylko uprawnionych stron, MFA admina i trwały audyt; status płatności wynika ze Stripe. E2E z dwoma klientami, adminem i gościem sprawdzają oba panele, chat, statusy i respektowanie preferencji także przy retry/restartach.
- **Logi i gotowość do Grafany:** JSON z korelacją żądań/zadań, bez sekretów i treści klienta; prywatne metryki Prometheus, liveness/readiness oraz sprawdzony lokalnie tor do Loki/Grafany. Monitoring nie jest wymagany do działania aplikacji. M1A ustanawia podstawę po M1; instrumentacja kolejnych funkcji powstaje wraz z nimi.
- Prywatność, szukanie luk, testy bezpieczeństwa i wizualne UI/UX przy każdej odpowiedniej zmianie; osobny pentest przed startem.

Poza MVP: marketplace, inne technologie druku, rozbudowane SEO/ERP, własny serwer pocztowy i serwer czatu. Konto klienta jest dostępne; upload, wycena i checkout gościa nadal działają bez zakładania konta. Przypisanie zamówienia gościa do konta wymaga potwierdzenia własności.

## Sposób przebudowy
**Pierwszy krok M0:** przegląd aktualnego stanu → `git commit` → `git push` → potwierdzenie SHA na remote → nowy branch `codex/rapidmaker-rebuild` i izolowany worktree. Snapshot zachowuje kod i docs, z wyłączeniem sekretów, cache i danych klientów. Lokalne i zdalne `main` mają rozbieżną historię; wypchnij kopię na osobną nową gałąź bez merge/force-push. To zapis stanu, nie potwierdzenie działania aplikacji.

Nowy, mały rdzeń Rust oraz struktura katalogów powstają od zera. W M0 ustal małe docelowe drzewo i mapę stare → nowe; wdrażaj je stopniowo w M1–M6. Stary kod służy do rozpoznania wymaganych zachowań i błędów do uniknięcia. Przenoś tylko potrzebne, zweryfikowane reguły i fixture’y; zastąpiony kod i katalogi usuwaj po krótkim sprawdzeniu działania. Nie przenoś starych warstw i błędów. Docelowo jeden modułowy pakiet aplikacji i ten sam program w trybie workera; mapa faktycznej struktury pozostaje w `ARCHITECTURE.md`.

**Nazwy:** marka Rapidmaker, domena `rapidmaker.pl`, identyfikatory Rust/crate/bin `rapidmaker`. Migrację utrzymywanych nazw, konfiguracji i aktywnych odwołań wykonaj spójnie w nowym branchu; historyczne adresy repo i zewnętrzne identyfikatory pozostają poprawnymi odniesieniami. Obecne nazwy `rapidfab*` nie zostały jeszcze zmigrowane.

**M0–M6, w tym M1A po M1 i M3A po M3, budujemy i testujemy lokalnie.** Publiczny deployment jest poza obecnym planem. Używamy prawdziwego PostgreSQL, slicera, sandboxów Stripe/Furgonetki i SMTP; koszty, profile, kartony i dostawa mogą mieć jawne dane testowe. Ich kalibracja i zatwierdzenie są warunkiem sprzedaży. Brak dostępu do API nie blokuje niezależnych lokalnych prac, ale mock nie zalicza kryterium integracji.

## Jakość i przejście dalej
**KISS: maksymalnie mało kodu przy zachowaniu czytelności, poprawności i bezpieczeństwa.** Minimalne zależności, warstwy, endpointy i tabele; każda nowa rzecz ma konkretne zastosowanie. API ma jawne wejścia, odpowiedzi i błędy, a schemat SQL proste relacje, ograniczenia i migracje. Oceniaj przyrost i usuwaj zbędny kod; nie skracaj kosztem walidacji ani potrzebnych testów.

Sprawdzaj tylko zmieniony fragment: build i krótki test logiki lub przebiegu w UI. Nie powtarzaj zielonych kontroli bez nowej zmiany lub błędu. Bez obowiązkowych review Astrą i testujących subagentów; workerzy mają przyspieszać implementację. Pełny E2E, audyt i pentest są osobnymi zadaniami według potrzeby. Przy płatnościach zachowaj kontrolę kwoty i ponowień, przy danych klientów — uprawnienia. Wynik i ograniczenia zapisuj krótko; nie deklaruj sprawdzenia, którego nie wykonano.

Stack i granice: [ARCHITECTURE.md](ARCHITECTURE.md). Kolejność i kryteria: [MILESTONES.md](MILESTONES.md). Testy: [docs/testing.md](testing.md).

Nowy frontend wszystkich ekranów działa w `frontend/`: większość widoków używa lokalnych mocków, a checkout ma osobne Rust demo API dla szacowania paczek, sandboxa Furgonetki i Stripe test. Zakres oraz komendy: [README](../frontend/README.md); wymagania: [brief](rapidmaker-frontend-brief.md). Wycena w API używa stałej taryfy demonstracyjnej; geometria jest wpisywana ręcznie i nie wynika z analizy pliku. Wykończenie, pasowanie, inserty, ekspres i inne niepotwierdzone wymagania kierują w UI do ręcznej wyceny. To nie zalicza M0–M6 ani nie potwierdza cen sprzedażowych.

**Redesign UI — 2026-10-18:** na prośbę użytkownika usunięto szkło, gradienty tła i nadmiar kart. Prawie czarne tło, proste sekcje/listy, spokojniejsze pola i zielone akcje. Nowy landing, otwarta wycena i checkout, spłaszczone panele oraz zwijane parametry modeli. Wspólny CSS uproszczony zamiast kolejnej warstwy nadpisań. Build: 0 błędów/ostrzeżeń; krótki przebieg landing → wycena → checkout, widoki mobile/desktop i panel admina obejrzane, bez poziomego overflow i błędów JS. Bez ponawiania testów płatności.

**Checkpoint POC — 2026-10-18:** wszystkie 12 widoków frontendu, lista zamówień i zwarte ustawienia. Rust demo obsługuje pakowanie wielu części, oferty kurierskie Furgonetki i wybór rzeczywistego Paczkomatu InPost. UI pokazuje zawartość każdej paczki, zwija wybór punktu i zachowuje kontakt po opuszczeniu checkoutu. Stripe ma osobny ekran wyniku oraz powrót do tej samej nieopłaconej sesji. Przed płatnością powstaje lokalne zamówienie z niezmienną kwotą, przewoźnikiem i planem paczek; klient i admin widzą ten sam rekord. Powrót/sprawdzenie aktualizuje status po odczycie API i porównaniu sesji, wyceny oraz kwoty. Dla tych rekordów symulacje płatności/zwrotów/etykiet są wyłączone.

Sprawdzenie bieżącej zmiany: build UI bez błędów/ostrzeżeń; jeden przepływ InPost → Stripe → powrót/anulowanie PASS (11 s); desktop/mobile obejrzane. Następnie prawdziwy Checkout sandbox opłacony syntetyczną kartą: API potwierdziło `paid`, `complete`, `livemode=false`, 5844 grosze; ekran wyniku obejrzany. Bez prawdziwej wpłaty i nadania, bez pełnego zestawu testów.

Sprawdzenie powiązania zamówienia: build bez błędów/ostrzeżeń; jeden przebieg InPost → sesja Stripe → anulowanie → rejestracja gościa → panel klienta/admina PASS (17 s). Oferta i nieopłacona sesja pochodzą z rzeczywistych sandboxów; lokalną aktualizację do `paid` sprawdzono kontrolowaną odpowiedzią w teście, bez kolejnej wpłaty. Widoki mobile/desktop obejrzane; skrócono placeholder podglądu i ukryto nieaktywny formularz zmiany ceny dla Stripe. Nie uruchamiano pełnego E2E ani review subagentami.

Granice: wymiary/masa wpisywane ręcznie, taryfa druku demonstracyjna 24 zł/szt. + 15 zł przygotowania; opakowania raz, przewóz według API. Konta/chat/statusy/prośby o wycenę nadal lokalne; brak slicera, analizy geometrii, webhooka i serwerowego magazynu zamówień. Zamówienia POC zapisują się tylko w przeglądarce. Paczkomat InPost: rzeczywista lista punktów, ponowna walidacja i wycena jednej paczki; większa partia kurierem lub do odbioru. Nie oznacza gotowości sprzedaży ani zaliczenia M3A/M4. Serwery w tle: frontend 18430, Rust API 18432. Sekrety poza Gitem, 0600. Furgonetka: aplikacja „Rapidmaker localhost test”, autoryzowana przez właściciela, token około 30 dni; syntetyczny adres nadawcy do testów.


Rozpoczęto M0: zabezpieczenie aktualnego kodu, push kopii i izolowany branch/worktree, następnie bezpieczny baseline. Implementacja nowego rdzenia jeszcze nie ruszyła; **0/9 etapów zaliczonych**, procent nieoszacowany przed baseline. Stripe sandbox i transport Apple SMTP/IMAP były sprawdzone; Sandbox Furgonetki jest sprawdzony w osobnym checkout demo, produkcyjny rdzeń pozostaje do zbudowania. PLAN jest jedynym zapisem postępu; dowody i następny krok: [aktywny checkpoint M0](milestones/M0.md).

## Milestones
- [ ] M0 — Snapshot, nazwy i lokalne środowisko
- [ ] M1 — Fundament Rust
- [ ] M1A — Logi i gotowość do Grafany
- [ ] M2 — Trzy formaty i GLB
- [ ] M3 — Wycena
- [ ] M3A — Pakowanie, dostawa i odbiór
- [ ] M4 — Zamówienie, Stripe, iCloud
- [ ] M5 — Konta, panele i powiadomienia
- [ ] M6 — UI i bezpieczeństwo

## Dostęp i blokery — 2026-10-18
| Etap | Potrzebne / udział użytkownika |
| --- | --- |
| M0 środowisko — preflight | Read-only 2026-10-18: Rust/Cargo, Task, GitHub i rootless Podman działają; daemon Docker jest niedostępny. Pełny przegląd snapshotu i bezpieczny lokalny start według `docs/testing.md` są realizowane w aktywnym M0. |
| M0 / M4 Stripe test | Zalogowano w Brave i sprawdzono istniejący prywatny sandbox; nazwę zmieniono na **Rapidmaker — test**. Banner potwierdza tryb sandbox, dostępne są klucze testowe. Bloker logowania usunięty. Klucze są w lokalnym `.env.stripe-test` (0600, wykluczony przez `.git/info/exclude`); wcześniejszy odczyt API zakończył się HTTP 200 i `livemode=false`. Lokalny checkout demo tworzy i sprawdza sesje Stripe test; wpłata sandbox została potwierdzona. Brak webhooków i serwerowego magazynu zamówień. Sandbox nie wymaga aktywacji płatności live; ponowne logowanie może wymagać MFA. |
| M0 / M4 iCloud | `void@sieciowiec.xyz` jako From, Reply-To i adres odbioru testów. Rzeczywisty test `rf-mail-b390b4c9ae`: SMTP STARTTLS i logowanie (235), wysyłka oraz odbiór identycznego Message-ID przez IMAP potwierdzone. Mail trafił do folderu `void@sieciowiec.xyz`, nie INBOX. Istniejące dane i ustawienia testowe są w lokalnym `.env` (0600, poza Gitem). Integracja nowej aplikacji Rust, outbox i retry pozostają w M4; po unieważnieniu hasła potrzebny udział właściciela. |
| M3 katalog Bambu — TODO | Ustalić import, kompletność i aktualizację: lokalny `/opt/bambustudio-bin/resources/profiles/BBL/filament/filaments_color_codes.json`, profile slicera oraz [oficjalne źródło Bambu Studio](https://github.com/bambulab/BambuStudio/blob/master/resources/profiles/BBL/filament/filaments_color_codes.json). Zachować ID materiału/koloru, nazwę, HEX, zgodność i wersję źródła. Katalog kolorów nie zawiera cen; domyślny `filament_cost` z profilu nie jest potwierdzonym kosztem zakupu w PLN/kg. Aktualne ceny i sposób ich aktualizacji do ustalenia. |
| M3 wycena | Testuj jawne koszty PLN/kg i PLN/kWh, masę z odpadami/purge, energię rozgrzewania/druku i liczbę partii. Przed sprzedażą operator zatwierdza drukarkę, dyszę, materiały, profile, koszty i pomiary kalibracyjne. |
| M3A pakowanie / Furgonetka — POC działa | Rust pakuje wiele części i pobiera zweryfikowane oferty kurierskie oraz punkty/oferty InPost w sandboxie. Gabaryty pochodzą z analizy modelu, masa jest ostrożnym szacunkiem pełnej bryły; katalog kartonów jest testowy. Bez nadań. Fizyczna kalibracja i produkcyjna geometria pozostają przed sprzedażą; nie zalicza całego M3A. |
| M4 Stripe / checkout — POC działa | Sesja Stripe test ma serwerową kwotę i weryfikowany powrót; rzeczywista wpłata sandbox potwierdzona. Powiązanie z zamówieniem i oba panele są lokalne. Brak webhooków, serwerowego magazynu zamówień i produkcyjnej wyceny; nie zalicza M4. |
| M6 bezpieczeństwo | Własny lokalny staging, syntetyczne dane, jawny zakres skanów, admin MFA oraz sprawdzona kopia i odtworzenie DB/plików. |

Bloker zewnętrzny zatrzymuje zależne kryterium, nie niezależne prace. Mock nie zastępuje spełnienia kryterium dostawcy. Żadnych prawdziwych obciążeń, refundów ani publicznego deployu w ramach przygotowania planu.

### Aktualizacja POC — kreator wyceny
- Druk → wysyłka → podsumowanie → płatność; adres dopiero w wysyłce. Jedna edytowana część, wizualne warianty jakości/budowy i ukryte opcje eksperckie.
- Weryfikacja: build, wizualny desktop/mobile i przejście do podsumowania odbioru osobistego. Bez pełnej regresji E2E; taryfa nadal testowa.

### POC — logistyka i adres
- Checkout nie pyta o wymiary części ani liczbę paczek. Plan liczy Rust z dostępnych danych; brak wiarygodnej analizy modelu wymaga ręcznej wyceny lub odbioru.
- Błędy pól adresu osobno od braku oferty i niedostępności Furgonetki. Najtańsza dostępna oferta dotyczy bieżącego planu paczek.
- Kurier: wspólny formularz Stripe Address Element + Payment Element, potwierdzenie po podsumowaniu. Własny klucz Google nie jest potrzebny; działanie sugestii potwierdził użytkownik (2026-10-18). Pozostałe metody zachowują hosted Checkout.

### POC — geometria i płatność osadzona
- Upload analizuje STL/STEP/3MF (32 MiB); geometria serwerowa zasila pakowanie. Proces izolowany, ograniczony czas/pamięć; szczegółowe granice w ARCHITECTURE.md.
- Wymiary są odczytane, masa nadal szacowana: pełna bryła 2 g/cm³. Slicer, trwałe pliki, pełne STEP i kalibracja ceny pozostają do wykonania.
- Płatność kartą testową przez Payment Element potwierdzona w UI; backend sprawdza kwotę i stan Stripe. Konta/zamówienia nadal lokalne, bez webhooków; nie zamyka M3/M3A/M4.
- Krótka weryfikacja: build frontendu 0 błędów/ostrzeżeń, build Rust; HTTP analizuje STL powyżej 64 KiB, quote ignoruje podmienione wymiary; wznowienie zwraca tę samą płatność. Widok desktop/mobile sprawdzony.

### Korekta checkoutu POC
- Najpierw jeden pionowy formularz „Dane odbiorcy”: email i telefon bez maski, następnie imię i adres z podpowiedziami Stripe. Potem dostawa. Bez podziału kontakt/adres na osobne kolumny i bez dekoracyjnych separatorów. Brak automatycznego wstawiania profilu demo. Szkic kontaktu osobno dla użytkownika/gościa.
- Adres w kroku wysyłki, karta wyłącznie w płatności po podsumowaniu; można wrócić bez utraty danych. Brak alarmu o dostawie przed uzupełnieniem pól; metoda dostawy należy do wersji wyceny.

- Paczkomaty pobieramy automatycznie z kodu odbiorcy: do 12 punktów w promieniu 10 km od lokalizacji kodu, w kolejności dostawcy. Inna lokalizacja jest opcjonalna; zmiana kodu usuwa poprzedni wybór punktu. Zweryfikowano rzeczywiste punkty sandboxa dla Krakowa.

- Oferty odświeżają się automatycznie po uzupełnieniu/zmianie danych (600 ms przerwy, bez pętli ponowień); stare odpowiedzi są pomijane. Szczegóły pakowania są zwinięte. Brak osobnego przycisku sprawdzania dostawy.
- Weryfikacja tej korekty: build 0 błędów/ostrzeżeń. Nowy układ i automatyczne pobieranie ofert wymagają jeszcze krótkiego sprawdzenia w UI: narzędzie utraciło dostęp do przeglądarek.

### Korekta dostawy POC
- Faktura na końcu danych odbiorcy, przed dostawą. Szczegóły pakowania wyłącznie w podsumowaniu.
- Wybór metody nie czeka na API przewoźnika; poprzednie zapytanie jest anulowane. Błędy walidacji Furgonetki trafiają do pól, osobno od błędów konfiguracji i braku usługi. Wybieramy trzy najtańsze poprawne oferty, sprawdzając kolejne po odrzuceniu tańszej.
- Weryfikacja: build Rust/frontendu, 9 testów modułu providers; HTTP nowego serwera: syntetyczny adres zwraca DPD/DHL/FedEx, brak numeru zwraca błąd pola street. Brak dostępu narzędzia do UI nadal uniemożliwia potwierdzenie wizualne i klikanie metod.

- Korekta czytelności: telefon bez automatycznych odstępów; podsumowanie i szczegóły paczek bez nakładających się separatorów. Build oraz krótki przegląd podsumowania na desktopie sprawdzone.

### UI/UX POC — przegląd 2026-10-18
- Zakończono przegląd wizualny głównej, wyceny, dostawy/podsumowania/karty, logowania, obu paneli zamówień i ich szczegółów, chatu, ustawień klienta oraz produkcji, materiałów i ustawień admina. Sprawdzano komputer i telefon; poprawiono wykryte problemy. To przegląd interfejsu POC, nie zamknięcie backendowych kryteriów M6.
- Zachowany ciemny, zielony styl. Mniej powtarzających się separatorów i pustej przestrzeni. Na telefonie cena + następny krok są stale dostępne, pola mają 16 px; nagłówek działa także przy 320 px. Pełne rozbicie kosztów pozostaje w treści. Logowanie jest wycentrowane, karty przeglądarki mają nazwy podstron.
- Stripe zachowuje podpowiedzi adresu. Wyłączono nakładkę deweloperską i dodatkową rejestrację Link przy karcie. Kontakt jest odtwarzany przed montowaniem pól Stripe; reload zachowuje imię i adres. Etapy pozwalają wrócić do podsumowania; zmiana parametrów URL odświeża właściwy widok.
- Materiały: zwarta lista i pojedynczy rozwijany edytor, zapis po wszystkich polach, komunikat przy edytowanej pozycji, wyczyszczenie wyszukiwania. Zamówienia admina są listą na telefonie i tabelą na komputerze. Ustawienia są wyrównane, a status konfiguracji integracji odróżnia się od błędu odczytu.
- Opłacone/anulowane/zwrócone zamówienie nie pokazuje nieaktywnej zmiany oferty; aktywna ręczna wycena zachowuje formularz. Kwoty z walutą nie łamią się na telefonie. Produkcja ma krótsze odstępy i widoczny link do szczegółów.
- Błąd odczytu nie udaje pustego koszyka. Nieudane sprawdzenie płatności pokazuje nieznany wynik i dalsze kroki, zamiast formularza dostawy. Debug opisuje rzeczywisty zakres mocków i testowych API.
- Dowody: build frontendu 0 błędów/ostrzeżeń; obejrzane zmienione ekrany na komputerze i telefonie. Sprawdzono zapis materiału bez zmiany wartości, brak wyników i wyczyszczenie filtra, pusty katalog, błędy katalogu/checkoutu i przywrócenie normalnego stanu. Poprzednia iteracja: syntetyczny adres → DPD/DHL/FEDEX → wybór DHL → podsumowanie → karta; reload danych i powrót przez etapy. Bez nowych wpłat, nadań i pełnego E2E.

### Porządki struktury — 2026-10-20

Działające projekty przeniesiono do `frontend/` i `backend/`; parser zachowano w `backend/geometry/`, modele w `backend/tests/fixtures/`. Dokumenty planowania są w `docs/`. Usunięto legacy, Docker, Taskfile, narzędzie tokenów, `.codex`, LICENSE i artefakty. Jeden `.env` ma kopię SOPS; token odświeżania Furgonetki zachowano w nim jako `FURGONETKA_OAUTH_JSON` (launcher go nie przekazuje). Build frontendu i backendu PASS. Strona, API config i proxy Vite zwracają HTTP 200; upload cube_20mm.stl przez nowe API zwrócił analizę HTTP 200. Pełnego E2E ani odszyfrowania SOPS nie uruchamiano. Kryteria przyszłych etapów pozostają planem.


### Integracja frontendu Piotra — 2026-10-22

Bazą jest `codex/clean-start`; importujemy wyłącznie frontend z `codex/rapidmaker-frontend` (`ef8ceb9`), bez starej historii, Pythona, Dockera i `.idea`. Integrację przygotowano na `codex/frontend-integration`; wspólny wynik trafia do `main`. Rust API, geometria, SOPS, hooki i dokumentacja pozostają bazą projektu.

Landing i konfigurator Piotra używają WebGL do STL; upload STL/STEP/3MF i ceny przechodzą przez Rust API. Dotychczasowy checkout Stripe/Furgonetki i panele zachowują logikę w nowym motywie. Opcje techniczne są dostępne pod `/wycena/zaawansowana`, a powrót odtwarza koszyk. Pełny viewer GLB, trwałe zamówienia, slicer, konta i email pozostają dalszym planem.

Checkpoint: build frontendu PASS (0 błędów/ostrzeżeń), build backendu PASS. Upload prawdziwego `cube_20mm.stl` → podgląd WebGL → konfiguracja → analiza/wycena Rust → oferty DPD/FEDEX/UPS z Furgonetki sandbox → Stripe karta testowa → opłacone zamówienie 44,24 zł w panelu admina PASS. Niezgodność metod płatności naprawiono przez zgodne `allowed_payment_method_types`; status ponownie odczytano przyciskiem „Sprawdź płatność”. Obejrzano landing/checkout na komputerze i landing/panel na telefonie; poprawiono nakładającą się nawigację mobilną. Przykłady bez analizy kierowane są do ręcznej wyceny. Istniejące scenariusze technicznego konfiguratora kierują do nowej ścieżki zaawansowanej; pełny zestaw E2E NOT RUN. Nie zamawiano przesyłek ani nie wykonywano płatności live. Dalsze milestones pozostają otwarte.

### Workspace micro-crates — 2026-10-23

Backend przeniesiono do `backend/crates/`. Aktywne pakiety: `app` (API i orkiestracja), `geometry`, `packing` (bez Axum), `pricing` (zachowana taryfa POC). Domyślna binarka: `rapidmaker-api`; launcher i dokumentacja mają nowe ścieżki. Na życzenie użytkownika dodano puste, kompilowalne szkielety: `slicer`, `storage`, `orders`, `auth`, `payments`, `shipping`, `notifications`, `observability`. Nie są podłączone do runtime i nie oznaczają gotowych funkcji; obecne Stripe/Furgonetka nadal w app.

PASS: build API, Clippy wszystkich pakietów i targetów z `-D warnings`, formatowanie oraz 31 istniejących testów workspace (9 API, 12 geometrii, 10 pakowania); brakującą zależność testową `serde_json` pakowania uzupełniono. Po restarcie nowej binarki: HTTP config 200, prawdziwy upload STL przez Vite → izolowany worker → wycena 3900 gr dla jednej sztuki z przygotowaniem. Granice bezpieczeństwa i kontrakty HTTP zachowane. Pełny frontend E2E i ponowna płatność sandbox NOT RUN — zmiana dotyczy struktury Rust. Nie znaleziono `CLAUDE.md` w obecnym repo ani przeszukanej historii Git.

Pomiar tokei: kod Rust 3831 → 3830 LoC (−1), łącznie kod produkcyjny i testy inline, bez komentarzy/pustych linii. Wersje zewnętrznych zależności bez zmian.
