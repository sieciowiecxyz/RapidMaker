# Backend Rust — checkout, dostawa i rozmowy

Aktywny następny zakres: **S0–S4 — nadawanie przesyłek**, wyłącznie plan, jeszcze bez implementacji. Szczegóły poniżej; [kryteria etapów](MILESTONES.md#s0s4--nadawanie-przesyłek--plan).

R1 — zwroty płatności: wdrożone i sprawdzone lokalnie w Stripe test (400 + 600 = 1000 gr, ponowienie i restart bez duplikatu). Pełny lub częściowy zwrot przez API admina, bez frontendu. Rejestr PostgreSQL rezerwuje kwotę przed Stripe; wspólny worker płatności ponawia ten sam klucz i uzgadnia status. Wymagane: admin/passkey/CSRF, potwierdzona płatność, brak nadmiarowego lub podwójnego zwrotu. Zwrot nie zmienia statusu produkcji. Kontrakt i granice: [checkout](checkout.md#zwroty-płatności).

Status: P1 (warianty pakowania i ceny dostawy) oraz C1–C6 wdrożone i sprawdzone lokalnie w sandboxach; bez Stripe live/publicznego wdrożenia. Zakres: wyłącznie backend i OpenAPI, bez pisania strony. [Milestones](MILESTONES.md).

Rozmowy przez stronę i email: [CH0–CH4 PASS lokalnie](chat-email.md). Poniższy checkout oraz P1/P2 są wdrożone.

## Cel i punkt wyjścia

Jedno API prowadzi od własnych wycen slicera i danych odbiorcy przez ofertę dostawy do potwierdzonego zamówienia. Restart, timeout lub ponowione żądanie nie mogą zgubić płatności ani utworzyć drugiej należności.

Punkt wyjścia przed C1–C6: auth klient/gość/admin, katalog i slicer, pakowanie, wyceny Furgonetki sandbox, Stripe test, zamówienia i outbox PostgreSQL, ochrona plików oraz Swagger. Oferty checkoutu i część stanu płatności były w RAM; brakowało webhooków i pełnej walidacji adresu. B1–B8 i wcześniejsze dowody zostają w [checkpoincie](milestones/backend.md), [architekturze](ARCHITECTURE.md) i historii Git.

## Minimalna konstrukcja

- Jeden proces Axum/Tokio i PostgreSQL/SQLx. Bez Redis, RabbitMQ, mikroserwisów ani nowego frameworka zadań.
- `api`: HTTP, uprawnienia i koordynacja. `storage`: migracje, jawne SQL i transakcje. `pricing`/`packing` nadal liczą cenę i paczki.
- Istniejące `payments` i `shipping` zawierają konkretne adaptery Stripe/Furgonetki wyjęte z checkout.rs. Bez dodatkowych crate’ów, generycznych repozytoriów i warstwy usług dla każdej funkcji.
- Typowane request/response i błędy w OpenAPI; kwoty całkowite w groszach, waluta PLN. Dozwolone pola i rozmiary jawne; nie przenoś dowolnego JSON klienta do dostawców.
- Docelowo jeden nowy flow: Stripe Hosted Checkout. Istniejące PaymentIntent/Checkout zachowują zgodność przez cienkie adaptery do wspólnej kontroli płatności; nie rozwijamy osobnego silnika dla każdego. Usunięcie starych tras dopiero po migracji ich klientów.

## Dane odbiorcy i dostawa

Jeden zapisany obiekt: imię i nazwisko, email, telefon, kraj PL, ulica z numerem, opcjonalny lokal, kod pocztowy i miasto. Opcjonalnie osobne dane firmy do faktury; samo zapisanie NIP nie wystawia faktury. Początkowo tylko dostawy krajowe, kurier/punkt odbioru/odbiór osobisty.

Normalizuj odstępy, kod `NN-NNN` i telefon do jednoznacznego formatu. Waliduj długości, znaki sterujące i wymagane pola zależnie od metody. Odbiór osobisty nie wymaga adresu; punkt odbioru wymaga kontaktu i zweryfikowanego ID punktu, a kod miejscowości służy do wyszukania. Adres punktu pobiera backend. Brak Google/geokodera i książki adresowej na tym etapie; poprawny format nie dowodzi, że adres fizycznie istnieje.

Wyszukiwanie punktów i wycena używają danych zapisanego szkicu. Ten sam adres trafia do Furgonetki i snapshotu zamówienia; Stripe nie zbiera ponownie ani nie zmienia adresu dostawy. Zmiana części, ilości, adresu lub punktu unieważnia dotychczasową ofertę. Po rozpoczęciu płatności szkic jest zamrożony.

Pakowanie korzysta z istniejącego algorytmu oraz serwerowych wymiarów/masy; klient nie podaje liczby paczek ani ich parametrów. Oferty mają pełny koszt wszystkich paczek i opakowań, bez podwójnego naliczenia. Najtańsza poprawna oferta może być oznaczona jako rekomendowana. Przekroczenie limitów lub brak danych → ręczna wycena; bez fikcyjnej ceny palety. Limity przewoźnika potwierdza adapter Furgonetki.

Błędy: `422` + `field_errors` dla konkretnych pól; poprawne dane bez dostępnej usługi → `manual_required` z krótką przyczyną; timeout/awaria dostawcy → `503` i możliwość ponowienia. Nie zamieniaj awarii na „błędny adres”, brak ofert ani darmową dostawę.

## Proponowany kontrakt HTTP

| Operacja | Zachowanie |
| --- | --- |
| `POST /api/checkout/drafts` | Tworzy własny szkic z ID snapshotów części, odbiorcą, opcjonalnymi danymi faktury i metodą dostawy. Zwraca ID, wersję i braki wymagające uzupełnienia. |
| `GET /api/checkout/drafts/{id}` | Odczyt własnego szkicu, ofert i stanu płatności po restarcie. |
| `PATCH /api/checkout/drafts/{id}` | Aktualizuje dane z oczekiwaną wersją; konflikt → 409. Po rozpoczęciu płatności edycja zablokowana. |
| `GET /api/checkout/drafts/{id}/points` | Ograniczona lista punktów dla zapisanego kodu/miejscowości; nie obiecujemy sortowania po odległości bez danych geograficznych. |
| `POST /api/checkout/drafts/{id}/quote` | Ponownie sprawdza snapshoty/stan materiałów; zapisuje paczki, oferty, kwoty i termin ważności. |
| `POST /api/checkout/drafts/{id}/payment` | Wymaga wersji, ID oferty i `Idempotency-Key`; zwraca URL Stripe albo stan oczekiwania na rozstrzygnięcie. |
| `POST /api/webhooks/stripe` | Odbiera podpisane zdarzenia Stripe bez sesji użytkownika. |

Kontrakt wdrożony; rzeczywiste DTO i mapowanie starych tras są w OpenAPI oraz [checkout.md](checkout.md). Nie dodawaj osobnego CRUD adresów. Szkic nie jest jeszcze zamówieniem. Link powrotu ze Stripe pokazuje status; nie potwierdza zapłaty.

## PostgreSQL i płatność

Minimalnie trzy nowe tabele: `checkout_drafts` (właściciel, wersja, dane/snapshot oferty i ważność), `payment_attempts` (powiązania, kwota, stan, stały klucz Stripe, ID sesji, retry/lease) i `stripe_events` (unikalne ID zdarzenia, potrzebne dane, stan przetworzenia/retry). Rozszerz istniejące `orders` o powiązanie próby i stany przed uzyskaniem ID Stripe; nie twórz drugiej tabeli zamówień. Pola używane do blokad, wyszukiwania i ograniczeń mają własne kolumny; ograniczone JSONB tylko dla snapshotów. Unikalność w bazie: jedna aktywna próba na szkic i jedno zamówienie na próbę. Rozdziel stan płatności od statusu produkcji.

Przed siecią, w krótkiej transakcji: sprawdź właściciela/wersję/ofertę, zamroź kwotę i dane, utwórz oczekujące zamówienie, próbę oraz ochronę plików. Następnie wywołaj Stripe poza transakcją/blokadą, zawsze z zapisanym kluczem i identycznymi parametrami. Zapisz wynik. Równoległe żądania i retry muszą wskazać tę samą próbę. Ten sam klucz klienta z inną treścią → 409. Zakres klucza obejmuje właściciela i operację.

Timeout oznacza `unknown`, nie odmowę zapłaty. Nie twórz wtedy nowej próby. Krótki worker Tokio z retry/lease PostgreSQL uzgadnia nierozstrzygnięte próby i zdarzenia. Po przekroczeniu okna idempotencji Stripe nie powtarzaj w ciemno tworzenia sesji: odczyt znanego obiektu, korelacja webhooka lub ręczne rozstrzygnięcie. Zmiana sposobu dostawy wymaga najpierw potwierdzonego zamknięcia poprzedniej płatności.

Webhook: limit body, oryginalne bajty, podpis Stripe i tolerancja czasu przez sprawdzoną bibliotekę; sekret w SOPS. Tylko ta trasa jest wyłączona z cookie/CSRF/Origin, pozostałe zachowują ochronę. `2xx` dopiero po trwałym przyjęciu; duplikat jest bezpieczny, błąd DB daje retry. Worker sprawdza tryb test/live, powiązanie obiektu i zamówienia, kwotę, walutę oraz faktyczny status płatności. Nie polega na kolejności zdarzeń ani samym `checkout.session.completed`. Obsługuje także płatności opóźnione, wygaśnięcie i niepowodzenie. `paid` i outbox emaila zapisuje raz, w jednej transakcji; starsze zdarzenie nie cofa zapłaty.

Nie zwalniaj plików tylko dlatego, że minął TTL oferty. Próba niepewna lub płatność w przetwarzaniu nadal chronią pliki. Zwolnienie po potwierdzonym zamknięciu możliwości zapłaty i sprawdzeniu innych zamówień; opłacone zachowują obecną retencję. Szkice bez prób wygasają po 24 h; ważność oferty nie przekracza ważności snapshotów slicera. Rekordy deduplikacji i zdarzenia mają osobną, jawną retencję, dłuższą od okna powtórzeń dostawcy.

## Granice i weryfikacja

Używaj obecnych kont/gości, kontroli własności, CSRF/Origin i limitów. Dodaj limity liczby szkiców, prób, punktów i wywołań dostawców; ograniczony czas/rozmiar odpowiedzi HTTP. URL powrotu z konfiguracji serwera. Loguj ID i rezultat, bez adresów, tokenów, podpisów ani sekretów Stripe. Nie przechowuj danych kart.

Krótkie testy istotnych awarii w każdym etapie; na końcu jeden przebieg sandbox przez HTTP/CLI, bez implementacji frontendu. Stripe Hosted Checkout można wykorzystać do ręcznego zakończenia płatności testowej. Bez produkcyjnych obciążeń, nadawania paczek, refundacji, IMAP, panelu admina i wdrożenia publicznego. Brak nowej integracji Grafany w tym feature.

Potrzebne: lokalny PostgreSQL, istniejące testowe Stripe/Furgonetka, poprawne dane nadawcy, jawne ceny i aktywny materiał. Nowy sekret webhooka oraz Stripe CLI do przekazywania zdarzeń na localhost; brak dostępu może wymagać logowania właściciela. Wpisy `.env` synchronizuje SOPS przed commitem. Produkcyjne HTTPS, konto Stripe/live, zasady retencji i kalibracja pakowania wymagają osobnego etapu.

Źródła kontraktu: [webhooki Stripe](https://docs.stripe.com/webhooks), [idempotencja](https://docs.stripe.com/api/idempotent_requests), [potwierdzanie Checkout](https://docs.stripe.com/payments/checkout/fulfill-orders). Szczegóły zdarzeń i okna retry potwierdź przy implementacji dla przypiętej wersji API.

Dowody, aktualne ograniczenia i pomiar LOC: [checkpoint C1–C6](milestones/backend.md#checkout-c1c6--2026-10-24).

## P1 — Warianty pakowania i najtańsza dostawa — wdrożone

Tylko Rust backend. Do 5 różnych bezpiecznych planów z obecnego katalogu kartonów; najpierw najmniejsza liczba paczek w znalezionych planach, potem ceny Furgonetki + opakowania i walidacja od najtańszej. Zapis jednego zwycięskiego planu i usług dla tych samych paczek w istniejącym snapshocie. Do 5 wycen zbiorczych/10 walidacji/30 s; awaria nie daje oferty. Bez nowego SQL, zależności, strony i dokładnego nestingu. Kontrakt i ograniczenia: [checkout.md](checkout.md#wybór-pakowania-i-ceny-dostawy).

Sprawdzone: regresje pakowania/rankingu, rzeczywiste ceny i walidacja kilku planów w sandboxie; szczegóły w milestone P1. Rezerwy i katalog kartonów wymagają kalibracji przed użyciem produkcyjnym.

## P2 — Cache i łączenie zapytań dostawy — wdrożone

Usługi Furgonetki w cache procesu 6 h; identyczne wyceny 120 s i jeden aktywny lider na klucz. Ważna oferta szkicu używana z PostgreSQL po lokalnej walidacji, bez nowego HTTP i zwiększenia wersji. Ważność nie odnawia się przy odczycie; zmiana danych unieważnia ofertę. Limit 64 wpisów/4 liderów, błędy 2 s; bez schedulera, nowych zależności, SQL i frontendu. Kontrakt: [checkout.md](checkout.md#cache-i-identyczne-żądania).

## CH0–CH4 — Jedna rozmowa przez API i email — wdrożone lokalnie

Rozwijamy istniejące rozmowy PostgreSQL i outbox SMTP: treść w kopii email, bezpieczne referencje odpowiedzi, jeden worker IMAP i wspólny zapis wiadomości. Najpierw test prawdziwego iCloud i nagłówków, potem historia/uprawnienia, wysyłka, odbiór i jeden przebieg końcowy. Osobny kursor przeczytania, paginacja i grupowanie alertów. Bez frontendu, załączników, brokera i nowego serwisu.

Projekt, granice zaufania i blokery: [chat-email.md](chat-email.md). SMTP/IMAP i wspólna historia sprawdzone rzeczywistymi mailami testowymi, bez frontendu.


## S0–S4 — Nadawanie przesyłek — plan

### Flow i panel admina

`opłacone → druk → gotowe → potwierdzenie paczek i kosztu → zamówienie przesyłek → etykiety → przekazanie przewoźnikowi → doręczenie`.

Płatność klienta zapisuje wybór dostawy, adres/punkt i szacowane pakowanie. Nie zamawia kuriera. Przy `ready` admin widzi ten snapshot, potwierdza rzeczywiste wymiary zewnętrzne i masę paczek, sprawdza aktualną cenę, następnie klika „Zamów przesyłki”. To jedyna akcja upoważniająca backend do płatnego zamówienia. Dalej: „Pobierz etykiety”, „Przekazano przewoźnikowi”, lista numerów/statusów i czytelny błąd do obsługi. Najpierw API/Swagger; frontend panelu jest osobnym zadaniem. Kupujący ma tylko odczyt własnej dostawy; nie zamawia kuriera ani nie wybiera opcji na koszt sklepu.

### Dane i koszt

- Wykorzystaj `orders.fulfillment`, obecne pakowanie, adresy, adapter `shipping`, auth/passkey/CSRF oraz outbox. Algorytm podpowiada paczki; przed nadaniem operator potwierdza faktyczne parametry. Nie zmieniaj opłaconego snapshotu checkoutu: finalne pakowanie i aktualny koszt zapisz osobno.
- Zachowaj metodę, usługę i punkt kupującego. Zmiana przewoźnika/punktu/adresu wymaga uzgodnienia z klientem i osobnej rewizji. Odbiór osobisty omija Furgonetkę. Duże zamówienie bez dostępnej usługi trafia do wyceny ręcznej; v1 bez palet i pobrania.
- Pokazuj osobno pobraną od klienta cenę dostawy, aktualny koszt przewoźnika oraz różnicę; koszt opakowań osobno. Admin akceptuje aktualną ofertę dla konkretnej wersji danych. Zmiana paczek/danych unieważnia akceptację; przed zamówieniem ponowna walidacja i wycena, różnica wymaga ponownego zatwierdzenia. Bez automatycznej dopłaty klienta lub ukrytej podmiany usługi. Dopłaty przewoźnika po pomiarze są możliwe; lokalna akceptacja nie gwarantuje ceny ostatecznej faktury.
- Nadanie tylko przy potwierdzonej płatności i `ready`. Pełny zwrot blokuje nowe nadanie. Oczekujący/niepewny zwrot wstrzymuje automatyczne przetwarzanie; po częściowym zwrocie admin potwierdza, co faktycznie wysyła. Zlecenie zwrotu po rozpoczęciu nadania wymaga interwencji, nie automatycznego usuwania przesyłki. Lokalne decyzje serializuj na zamówieniu; operacje Stripe i Furgonetki nie są jedną transakcją.

### Minimalny backend

Bez nowych crate’ów, serwisu i brokera. `shipping` — transport Furgonetki; `storage` — transakcje i lease; `api/shipments.rs` — uprawnienia/HTTP; mały worker w tym samym procesie — zlecenia i tracking; `notifications` — treść emaila. Nie przenoś całej logiki do routera.

Dwie tabele: `shipments` (wersja danych, snapshot, zaakceptowany koszt, UUID żądania i komendy, autor, stan, lease/retry) i `shipment_packages` (powiązania paczek z ID Furgonetki, numery, statusy). Jeden aktywny batch nadania na zamówienie; v1 wysyła wszystkie jego części. Batch może mieć wiele paczek, także częściowy sukces dostawcy. ID przesyłki Furgonetki nie musi oznaczać jednego kartonu: zachowaj grupowanie oraz limity przewoźnika z walidacji i wyceny. Unikalne ID dostawcy, ograniczone payloady i liczba paczek; bez ogólnej tabeli dowolnych komend.

Proponowane API (jeszcze nie istnieje):

| Endpoint | Działanie |
| --- | --- |
| `GET /api/admin/orders/{id}/shipment` | Snapshot, koszt, statusy, błędy i dozwolone akcje. |
| `PUT /api/admin/orders/{id}/shipment` | Potwierdzenie paczek, kontrola wersji; bez zakupu. |
| `POST /api/admin/orders/{id}/shipment/quote` | Aktualna walidacja/cena dla tej wersji. |
| `POST /api/admin/orders/{id}/shipment/book` | UUID ponowienia + zaakceptowana oferta; `202` oznacza zapis zlecenia. |
| `GET /api/admin/orders/{id}/shipment/labels/{package_id}` | Prywatny PDF A6, opcjonalnie A4, wyłącznie zapisane ID tej przesyłki. |
| `POST /api/admin/orders/{id}/shipment/handover` | Potwierdzenie faktycznego przekazania konkretnych paczek. |
| `GET /api/account/orders/{id}/shipment` | Własne numery/statusy z DB, bez etykiet, kosztów zakupu i sekretów. |

### Wywołania Furgonetki i awarie

Zweryfikowana [oficjalna specyfikacja REST](https://furgonetka.pl/js/swagger/api-rest-documentation.4cc02d4621.yaml), 2026-10-24: `POST /packages` dodaje dane; `PUT /order-commands/{uuid}` zleca zamówienie, a `GET` tej komendy podaje wynik. Etykieta: `GET /packages/{package_id}/label`; tracking: `GET /packages/{package_id}/tracking`. Sandbox oferuje `PUT /packages/tracking/simulate`. Dokładne kontrakty powtórzeń i identyfikacji grup trzeba sprawdzić w S0, bez zakładania idempotencji niedokumentowanego POST.

- Przed HTTP zapisz snapshot, lokalny UUID, referencję i UUID komendy w PostgreSQL; blokada/lease uniemożliwia dwa równoległe nadania. Powtórzenie identycznego żądania oddaje istniejący batch; inny payload dla tego UUID — konflikt. Żadnego HTTP pod blokadą SQL.
- `POST /packages` nie ma tu potwierdzonego klucza idempotencji. Po timeout szukaj po zapisanej krótkiej unikalnej referencji, potwierdź zgodność danych; brak jednoznacznego wyniku → `manual_review`, bez nowego POST. Referencja nie jest gwarancją deduplikacji dostawcy. Po utracie odpowiedzi zamawiania odczytaj istniejącą komendę UUID; nie generuj kolejnej. Ponowienia PUT tylko po potwierdzeniu kontraktu w sandboxie.
- Zapisuj sukces/błąd każdej przesyłki. Częściowy sukces nie uprawnia do zamówienia całego batcha ponownie. Stan niepewny zachowuje blokadę; admin widzi referencje i może uzgodnić wynik z Furgonetką przez chronioną akcję z dowodem dostawcy, bez ręcznej edycji SQL. Anulowanie już zamówionej przesyłki i rozliczenie jej kosztu poza v1; nie traktuj skasowania szkicu/komendy jako potwierdzonego anulowania kuriera.
- Etykiety pobieraj przez stały host API, bez dowolnych URL/redirectów; limit czasu/rozmiaru, `application/pdf`, `no-store`, attachment. Nie ujawniaj etykiet klientowi ani w logach. Błędy pól pokaż przy konkretnych danych; konto, saldo, token i regulamin to błędy operatora, nie błędnego adresu klienta.

### Tracking, statusy i email

- Rozdziel stan operacji (`draft/queued/booking/unknown/booked/failed/manual_review`) od transportu (`awaiting_handover/in_transit/delivered/exception/returned`). Etykieta ≠ wysłanie. Istniejąca ręczna zmiana `ready → shipped/completed` musi respektować nowy flow; odbiór osobisty pozostaje oddzielną ścieżką.
- Najpierw polling w workerze: szybkie, ograniczone odczyty komendy z backoff; tracking aktywnych przesyłek np. co 15 min, z jitterem i retry/backoff. Odczyt panelu korzysta z DB, nie wywołuje dostawcy. Po doręczeniu/zwrocie zatrzymaj regularne odpytywanie; awaria API nie oznacza anulowania. Publiczne webhooki dopiero później, po sprawdzeniu uwierzytelniania dostawcy.
- Potwierdzenie admina lub wiarygodny skan przewoźnika oznacza przekazanie paczki; późne/zdublowane zdarzenia nie cofają terminalnego stanu. `completed` po doręczeniu wszystkich paczek; wyjątki/zwrot do nadawcy pokazuj adminowi. Przekazanie części paczek nie oznacza wysłania wszystkich.
- Zmiana statusu i email przez istniejący outbox w jednej transakcji. Po faktycznym przekazaniu: przewoźnik, numery/linki i informacja, jeśli pozostałe paczki czekają. Jeden logiczny email na nowo przekazaną grupę, deduplikacja przy restartach i preferencje klienta; bez podwójnego ogólnego „status changed”. Nie wysyłaj powiadomień przy każdym skanie. Wyłącz mail z dokumentami Furgonetki do nadawcy (`skip_email_send`); osobne SMS/maile przewoźnika sprawdź w S0, bez obietnicy ich wyłączenia.

### Blokery i zakres

S0: sprawdzić sandbox, bieżący token/odnowienie, uprawnienia API, kompletność nadawcy, regulaminy, dostępność usługi, środki testowe i zachowanie wielopaczkowe. W produkcji dodatkowo saldo/rozliczenie, poprawne dane firmy i jawne włączenie nadawania live; obecny adapter jest sandbox-only. Brak salda/uprawnień/regulaminu → czytelny blokujący błąd, bez automatycznego akceptowania warunków. Kod nie nadawał jeszcze paczek i nie był tu testowany integracyjnie; ten etap to tylko plan. Testy wdrożenia: krótki sandbox, w tym dwie paczki, częściowy błąd, utrata odpowiedzi/restart i brak duplikatu/emaila; bez Astry i pełnych audytów.
