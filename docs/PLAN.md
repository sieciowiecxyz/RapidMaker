# Backend Rust — checkout, dostawa i rozmowy

**S0–S4 — nadawanie przesyłek:** backend wdrożony i sprawdzony lokalnie oraz w pełnym przepływie sandbox (nadanie, PDF, tracking, SMTP). [Dowody i granice](MILESTONES.md#s0s4--nadawanie-przesyłek).

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

Krótkie testy istotnych awarii w każdym etapie; na końcu jeden przebieg sandbox przez HTTP/CLI, bez implementacji frontendu. Stripe Hosted Checkout można wykorzystać do ręcznego zakończenia płatności testowej. W pierwotnym C1–C6 nie obejmowano nadawania, zwrotów i IMAP; późniejsze etapy poniżej dodają te funkcje. Nadal bez produkcyjnych obciążeń, nowego panelu i publicznego wdrożenia. Brak nowej integracji Grafany w tym feature.

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


## S0–S4 — Nadawanie przesyłek

Status: PASS lokalnie i w sandboxie, bez frontendu/live. DPD: zamówienie jednej przesyłki z dwoma kartonami i dwoma numerami, prywatny PDF, restart/ponowienie bez duplikatu, przekazanie, jedna wysyłka SMTP do void@sieciowiec.xyz i symulowane doręczenie wszystkich kartonów → completed. Zamówienie opłacone i sesja admina pochodziły z izolowanych fixtures; fizycznego passkey ani nowej płatności Stripe nie powtarzano.

### Flow

`opłacone → druk → ready → potwierdzenie paczek → akceptacja aktualnego kosztu → nadanie → etykiety → przekazanie → doręczenie`.

Klient płaci i wybiera usługę/punkt. Admin potwierdza rzeczywistą liczbę, zewnętrzne wymiary i masę paczek na podstawie propozycji algorytmu. Dopiero jawny POST `book` upoważnia worker do zamówienia przewozu. Wersja, UUID żądania i ID zaakceptowanej oferty chronią przed zmianą danych/podwójnym kliknięciem. Etykieta nie oznacza wysłania. Nadanie używa `request_collection=false`; podjazd jest osobną decyzją admina przez L1 poniżej.

### API i użycie

Wszystkie trasy admina wymagają sesji po passkey; zapisy także CSRF/Origin. Odczyt klienta korzysta z dotychczasowej własności konta/gościa. DTO i błędy są w Swaggerze.

| Endpoint | Działanie |
| --- | --- |
| `GET /api/admin/orders/{id}/shipment` | Oryginalny snapshot zamówienia, propozycja pakowania, stan i kod błędu nadania. |
| `PUT /api/admin/orders/{id}/shipment` | `version`, `parcels:[{outer_mm:[120,100,80],weight_g:500}]`, opcjonalnie `refund_reviewed`. Początkowo version=0, później wersja z odpowiedzi. |
| `POST …/shipment/quote` | `{version}`: walidacja/cena wyłącznie kupionej usługi; brak zakupu. |
| `POST …/shipment/book` | `{version,request_id:UUID,quote_id}`; `202` oznacza zapis zlecenia. Identyczne ponowienie zwraca istniejący batch. |
| `GET …/shipment/labels/{package_id}?format=a6` | Prywatny PDF dla ID z tej dostawy; opcjonalnie a4. |
| `POST …/shipment/handover` | `{package_ids:[…]}`: faktyczne przekazanie wskazanych przesyłek. |
| `POST …/shipment/reconcile` | `{package_id,provider_id}`: odzyskanie ID po niepewnym dodaniu; potwierdza dane i stan waiting w Furgonetce. |
| `POST …/shipment/reopen` | `{version}`: po pewnym odrzuceniu/zmianie ceny, tylko gdy komenda nie istnieje, a wszystkie znane paczki nadal czekają. Nowa wersja i ponowna quote/book, te same paczki. Nie odblokowuje niepewnego zamówienia. |
| `GET /api/account/orders/{id}/shipment` | Zapisane statusy i numery, także stan każdego kartonu w grupie; bez etykiet, kosztu zakupu i payloadów operatora. |

Przygotowanie i nadanie wymagają paid+ready. Odbiór osobisty omija tę ścieżkę. Pełny zwrot blokuje nadanie; oczekujący/niepewny zwrot wstrzymuje operację. Częściowy wymaga potwierdzenia admina; zmiana zwróconej kwoty po potwierdzeniu zatrzyma worker. Zlecenie nowego zwrotu jest blokowane podczas queued/creating/ordering, dopóki nie rozstrzygniemy nadania. Stripe i Furgonetka nie są wspólną transakcją.

### Cena i trwałość

- Checkout pozostaje niezmiennym snapshotem. Finalne paczki i koszt są osobno w `shipments`/`shipment_packages`. Zachowujemy usługę, punkt i adres klienta; v1 nie podmienia ich ani nie obsługuje palet/pobrania/osobnych partii realizacji.
- Oferta nadania ma 120 s. Osobno: `collected_shipping_grosze`, `price_grosze`, `difference_grosze`, `packaging_grosze`. Zmiana danych unieważnia akceptację. Book ponownie wycenia, a worker sprawdza cenę jeszcze przed płatną komendą. Różnica wymaga ponownej akceptacji; bez automatycznej dopłaty klienta. Dopłaty przewoźnika po pomiarze pozostają możliwe.
- Jeden batch/zamówienie, do 30 rzeczywistych kartonów. Adapter dzieli je według limitów wybranej usługi i sumuje cenę grup. Przesyłka Furgonetki może zawierać kilka kartonów; nie utożsamiamy jej ID z jednym kartonem.
- Przed HTTP zapisujemy dane, krótką unikalną referencję i UUID komendy. Lease 120 s; jedna próba workera ograniczona do 90 s, retry/backoff, do 1000 podjęć. HTTP poza transakcją. Po wyczerpaniu budżetu: `manual_review`, `retry_exhausted`.
- Dodawanie przez POST nie jest ponawiane w ciemno: po utracie odpowiedzi tylko odczyt po referencji i sprawdzenie zgodności usługi, adresów, wymiarów oraz masy. Niejasny wynik → `manual_review`. Po PUT zamawiania odczytujemy tę samą komendę UUID, bez nowego zlecenia. Reopen wymaga dowodu, że poprzednia operacja nie istnieje i nie doszło do zamówienia.
- Częściowy sukces zachowuje znane zamówione przesyłki; reszta wymaga interwencji. Nie zamawiamy ponownie całego batcha. `last_error` rozróżnia m.in. `provider_terms`, `shipping_account`, `package_unknown`, `command_unknown`, `price_or_data_changed`. Podjazdy i anulowanie rozszerza L1–L2 poniżej; rozliczenie opłaty przewozowej pozostaje osobną decyzją operatora.
- PDF max. 8 MiB, stały host API, bez redirectów, `no-store`/`nosniff`/attachment. Bez publicznych etykiet i logowania danych odbiorcy/sekretów.

### Tracking i email

Jeden worker Tokio w tym samym procesie, bez nowego crate’a/brokera. Fazy operacji: draft/queued/creating/ordering/booked/dispatched/delivered/manual_review. Osobno zapisany tracking: awaiting_handover/in_transit/delivered/exception/returned, numery i status każdego kartonu. Tracking odpytuje najwyżej 4 przesyłki równolegle, zwykle co 15 min; panel czyta DB. Niepełny lub obcy zestaw numerów nie może potwierdzić doręczenia grupy. Doręczenie/zwrot zatrzymują jej regularne odpytywanie; terminalne doręczenie nie cofa się.

Przekazanie potwierdza admin lub skan przewoźnika. Przy wielu kartonach jednego listu grupę uznajemy za przekazaną/doręczoną po potwierdzeniu wszystkich kartonów. Nie kończymy zamówienia po pierwszej paczce. Status i outbox są atomowe: jedna kopia email z przewoźnikiem, wszystkimi znanymi numerami i liczbą pozostałych grup; preferencje sprawdzane ponownie przed SMTP. Bez maila przy każdym skanie i bez zdublowanego ogólnego status_changed. Istniejąca ręczna zmiana shipped/completed nie omija dostawy; odbiór osobisty ready→completed pozostaje dozwolony. Completed ustawia retencję plików 90 dni.

### Źródło, konfiguracja i blokery

[Oficjalna specyfikacja REST](https://furgonetka.pl/js/swagger/api-rest-documentation.4cc02d4621.yaml): POST /packages dodaje, PUT/GET /order-commands/{uuid} zamawia/odczytuje wynik; GET etykiety i trackingu, PUT sandboxowej symulacji. Żaden kod nie akceptuje regulaminów ani nie ustawia zgód marketingowych.

Dotychczasowe `FURGONETKA_SANDBOX_TOKEN` i `FURGONETKA_SENDER_JSON`; bez nowych sekretów. SMTP test tylko void@sieciowiec.xyz. Live, publiczne webhooki trackingu i odnowienie tokena pozostają poza wdrożeniem. Właściciel zaakceptował krajowy regulamin DPD i zweryfikował telefon konta w panelu; wcześniejsze 409 terms_and_conditions_not_valid oraz 400 /phone zostały usunięte. Odczyt API potwierdził pustą listę wymaganych regulaminów. Marketingu ani innych przewoźników nie włączano. Nowa/zmieniona konfiguracja konta może wymagać ponownej interwencji właściciela.


## L1–L2 — Podjazdy i anulowanie

Bez frontendu. Istniejące crate’y, worker i lease nadania; jedna tabela `shipment_actions`. Admin po passkey + CSRF, aktualna wersja nadania, `request_id` UUID. Wybrane `package_ids` to lokalne ID z GET nadania; od 1 do 30 przesyłek jednego zamówienia. Jedna aktywna operacja, maksymalnie 100 operacji historii na zamówienie. Łączenie podjazdów różnych zamówień poza tym etapem.

- **L1:** POST `/api/admin/orders/{id}/shipment/pickup/proposals` z `{package_ids,ready_date,repeat:false}` pobiera terminy. POST `/pickup` z `{version,request_id,package_ids,pickup_date:{date,min_time,max_time},repeat:false}` zleca wybrany wspólny termin. Ponowienie nieudanego odbioru: `repeat:true`, tylko jeśli przewoźnik udostępnia `repickup`. Worker ponownie sprawdza dostępność i termin przed HTTP. Dopiero wynik z numerem podjazdu i zgodnymi ID oznacza sukces. Podjazd nie oznacza wysłania zamówienia.
- **L2:** POST `/api/admin/orders/{id}/shipment/cancel` z `{version,request_id,package_ids}` anuluje wybrane zamówione przesyłki. Sprawdzamy `cancel_available`. Ostateczne `canceled` wymaga stanu `cancelled` w szczegółach Furgonetki; anulowanie odroczone pozostaje `pending`. Przy anulowaniu części grup nadanie wymaga ręcznego rozstrzygnięcia. Nie tworzymy automatycznie nowej przesyłki i nie zmieniamy zamówienia ani Stripe. Anulowanie może podlegać opłatom przewoźnika.

`202` oznacza trwały zapis, nie sukces u przewoźnika. GET `/api/admin/orders/{id}/shipment/actions` zwraca historię, stan i wynik. Ponowienie identycznego request_id zwraca tę samą operację; zmiana danych pod tym ID daje konflikt. Inne równoległe operacje, etykiety i przekazanie blokujemy na czas nierozstrzygniętego zlecenia. Anulowanych paczek nie przywraca tracking.

Stan `submitting` zapisujemy przed jedynym PUT. Po utracie odpowiedzi/restartcie sprawdzamy GET tej samej komendy UUID; nie wysyłamy ponownie PUT. Nieznany lub niespójny wynik → `manual_review`; POST `/api/admin/orders/{id}/shipment/actions/{action_id}/reconcile` z `{version}` wznawia wyłącznie odczyt. Nie usuwa niepewnego zlecenia i nie upoważnia do kolejnej mutacji. Wyniki częściowe zachowują potwierdzone grupy. Anulowanie odroczone odpytujemy co 15 min; inne oczekujące operacje zwykle co minutę. Brak nowych sekretów i zależności.

Źródło: [Furgonetka — podjazdy i anulowanie](https://furgonetka.pl/api/rest/packages-instruction). Dowody i ograniczenia: MILESTONES.md, L1–L2.


## L3 — Spójne logi Rust

Wdrożony wspólny `tracing`/JSON stderr, identyfikator HTTP, kontekst workerów i bezpieczne kategorie błędów. Zastępuje ręczne logi tekstowe; nie zmienia płatności, uprawnień, retry ani protokołu stdout. Zakres i poziomy: [logging.md](logging.md). Metryki, health/readiness, Loki/Grafana i retencja kolektora pozostają osobnym etapem.
