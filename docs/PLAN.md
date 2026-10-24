# Checkout, adres i dostawa — backend Rust

Status: P1 (warianty pakowania i ceny dostawy) oraz C1–C6 wdrożone i sprawdzone lokalnie w sandboxach; bez Stripe live/publicznego wdrożenia. Zakres: wyłącznie backend i OpenAPI, bez pisania strony. [Milestones](MILESTONES.md).

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

Tylko Rust backend. Do 5 różnych bezpiecznych planów z obecnego katalogu kartonów; ceny Furgonetki + opakowania, walidacja od najtańszej. Zapis jednego zwycięskiego planu i usług dla tych samych paczek w istniejącym snapshocie. Do 5 wycen zbiorczych/10 walidacji/30 s; awaria nie daje oferty. Bez nowego SQL, zależności, strony i dokładnego nestingu. Kontrakt i ograniczenia: [checkout.md](checkout.md#wybór-pakowania-i-ceny-dostawy).

Sprawdzone: regresje pakowania/rankingu, rzeczywiste ceny i walidacja kilku planów w sandboxie; szczegóły w milestone P1. Rezerwy i katalog kartonów wymagają kalibracji przed użyciem produkcyjnym.
