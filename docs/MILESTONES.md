# Milestones — backend

**S0–S4:** backend wdrożony; pełny przebieg sandbox BLOCKED regulaminem DPD. Szczegóły i granice poniżej.

C1–C6: PASS lokalnie (Stripe test, Furgonetka sandbox). Tylko Rust/PostgreSQL/API; bez strony. Decyzje i kontrakt: [PLAN.md](PLAN.md). B1–B8 pozostają ukończonym lokalnym POC; dowody w [checkpoincie](milestones/backend.md).

CH0–CH4 poniżej: PASS lokalnie (API/SMTP/IMAP), bez frontendu.

## C1 — Adres i kontrakt — PASS lokalnie

- Typowane DTO szkicu, odbiorcy, opcjonalnej faktury, dostawy i błędów; jeden obiekt odbiorcy dla dalszego procesu.
- Walidacja PL, normalizacja i wymagania zależne od kuriera/punktu/odbioru. Pusty formularz może być szkicem, ale nie płatnym checkoutem.
- Ustal zgodność istniejących tras, mapowanie stanów oraz wersję Stripe API. Aktualizuj OpenAPI wraz z implementacją; nie publikuj niezaimplementowanych tras.
- Gotowe gdy: krótki test poprawnych danych i błędów pól, odbioru bez adresu oraz odrzucenia nadmiarowych/za dużych danych. Nie uznaj składni za potwierdzenie istnienia adresu.

## C2 — Trwały szkic i snapshot — PASS lokalnie

- Migracje `checkout_drafts`, `payment_attempts`, `stripe_events` i potrzebne rozszerzenie `orders`; bez drugiego modelu zamówień.
- Zapis/odczyt/zmiana własnego szkicu, kontrola wersji, limity i TTL. Claim gość → konto przenosi także szkice bez utraty powiązań. Nigdy nie uprawnia cudzy identyfikator.
- Zmiana danych unieważnia ofertę; brak ważnej własnej wyceny lub dostępnego materiału blokuje płatność. Uwzględnij dotychczasowe zamówienia i holdy w migracji.
- Gotowe gdy: dane przetrwają restart; obcy dostęp jest odrzucony; dwie równoczesne zmiany nie nadpisują się; migracja działa na pustej bazie i istniejącym schemacie.

## C3 — Dostawa z zapisanego adresu — PASS lokalnie

- Podłącz punkty i oferty Furgonetki do szkicu; adres punktu weryfikuje dostawca. Odbiór osobisty działa bez Furgonetki.
- Użyj istniejącego pakowania, danych serwera oraz kosztu całej przesyłki wielopaczkowej. Oferta zawiera metodę, zweryfikowany punkt, paczki, koszt i ważność.
- Rozdziel błędne pola, brak usługi i niedostępność dostawcy. Za duże/niewycenialne zamówienie kieruj do ręcznej wyceny bez uruchamiania płatności.
- Gotowe gdy: sandbox zwraca ofertę i punkty dla zapisanych danych; wybrany punkt odpowiada usłudze; przypadki 1/1000 sztuk nie korzystają ze stałej ceny jednej paczki; timeout nie daje pustej ani darmowej oferty. Wystarczy jeden rzeczywisty przebieg dostawcy i kilka lokalnych regresji.

## C4 — Jedna trwała próba Stripe — PASS lokalnie

- Przed siecią zapisz oczekujące zamówienie, snapshot kwoty/dostawy, próbę, idempotency key i hold plików. Stripe Hosted Checkout otrzymuje wyłącznie dane serwera.
- Bez globalnego mutexa ani transakcji DB obejmującej HTTP. Unikalność/lease w PostgreSQL; ponowienie wraca do tej samej próby. Obecne wejścia PaymentIntent/Checkout respektują tę samą blokadę.
- Powrót, anulowanie i status nie ustawiają samodzielnie `paid`. Timeout tworzenia zachowuje stan niepewny i ochronę plików.
- Gotowe gdy: sandbox tworzy sesję z dokładną kwotą/PLN; równoległe żądania, restart i utrata odpowiedzi nie tworzą drugiej płatności; zmieniony payload z tym samym kluczem daje 409. Zły właściciel, cena klienta i wygasła oferta są odrzucane.

## C5 — Webhook i rozstrzyganie awarii — PASS lokalnie

- Weryfikuj podpis na surowym body; zapisuj zdarzenie przed `2xx`. Wąski wyjątek od ochrony browserowej wyłącznie dla webhooka.
- Worker z ograniczonym retry/lease przetwarza zdarzenia i uzgadnia niepewne próby. Waliduje powiązanie, tryb, kwotę/walutę i zapłatę; deduplikacja po ID zdarzenia oraz przejściu próby.
- `paid` + email atomowo; duplikat/zmieniona kolejność nie cofają stanu i nie dodają maila. Obsłuż płatność opóźnioną, wygaśnięcie, błąd oraz webhook przychodzący przed zapisem odpowiedzi Stripe.
- Nie ponawiaj tworzenia po wygaśnięciu okna idempotencji bez rozstrzygnięcia. Hold zwalniaj tylko dla potwierdzonego zakończenia płatności; `unknown` pozostaje chronione i widoczne operatorowi przez API/CLI.
- Gotowe gdy: zły/stary podpis odrzucony; ponowienie i kolejność zdarzeń bez podwójnych skutków; awaria DB nie potwierdza przyjęcia; restart workera wznawia pracę. Test granicy dotyczy też holdów i braku fałszywego `paid`.

## C6 — Jeden pełny przebieg i zamknięcie — PASS lokalnie

- HTTP/CLI: własny model i wycena → szkic/adres → dostawa → Stripe test → webhook → trwałe `paid` i pojedynczy wpis outbox. Email testowy wyłącznie do void@sieciowiec.xyz.
- Sprawdź odczyt po restarcie i jedną próbę obcego dostępu. Swagger opisuje rzeczywiste DTO/błędy; krótka instrukcja uruchamiania i odzyskiwania niepewnej płatności.
- Gotowe gdy: zakresowe fmt/Clippy/build/testy PASS i dowód sandboxa. Brak klucza/webhooka oznacz NOT RUN, nie PASS. Istniejące podatności zależności raportuj oddzielnie; lokalny sukces nie oznacza gotowości produkcyjnej.

## Sposób pracy i blokery

Małe commity po etapie; krótki test rzeczywistego ryzyka, bez Astry i testujących subagentów. Rust LOC mierz przed/po z podaniem zakresu. Nie zmieniaj frontendu; migracja jego adapterów jest osobną pracą. Nie nadawaj przesyłek ani nie uruchamiaj Stripe live.

Sekret webhooka/Stripe CLI mogą wymagać autoryzacji właściciela. Brak danych nadawcy, aktywnych usług sandbox, cen lub materiałów blokuje odpowiedni test integracyjny, ale nie kontrakt, migracje i lokalne testy awarii. Obsługa zwrotów, etykiety i publiczne wdrożenie pozostają poza C1–C6.

Dowody i granice: [checkpoint](milestones/backend.md#checkout-c1c6--2026-10-24). Live, publiczny TLS/proxy i frontend nie były wdrażane.

## P1 — Warianty pakowania i cena całej dostawy — PASS lokalnie, 2026-10-24

- Do 5 planów bez duplikatów; liczba sztuk, brak kolizji, wymiary, ochrona, masa i limity zachowane. Dotychczasowy `estimate` pozostaje dostępny.
- Rzeczywiste wyceny Furgonetki porównane z kosztami opakowań; walidacja od najtańszej pary plan/usługa. Jeden zwycięski plan w ofercie, PostgreSQL i zamówieniu; dotychczasowy HTTP/SQL bez zmian.
- PASS: 25 testów jednostkowych API/pakowania/dostawy; osobno test sandbox porównujący 3 plany (1/12/2 paczki) z wycenami każdego z osobna. Test rankingu chroni wybór kosztu całkowitego i tożsamości planu, także przepełnienie kwoty. Bez nadawania przesyłek.
- PASS: zakresowe fmt, Clippy i build. Dokładny nesting, palety, automatyczne porównanie różnych metod i kalibracja kartonów poza zakresem.
- Pomiar tokei, 3 zmienione pliki Rust: produkcja 1583 → 1703 (+120), testy 252 → 424 (+172); bez pustych linii i komentarzy. Bez nowych plików, crate’ów, zależności i migracji.

### Korekta P1 — najpierw pojemność i liczba paczek

- Kartony każdego rozmiaru wypełniane przed otwarciem kolejnego; porównanie taryf tylko dla planów z najmniejszą znalezioną liczbą paczek. Bez twierdzenia o globalnym optimum.
- PASS: 27 testów API/pakowania/dostawy, w tym 12 sztuk zawsze w jednej paczce i 999 sztuk jako pełne kartony + reszta. Dawny test wymagający wariantu wielopaczkowego zmieniony zgodnie z nowym priorytetem.
- PASS: rzeczywisty sandbox dla 12 sztuk: L=[12] i XL=[12], bez wariantu 12 osobnych paczek. Log pokazuje rozmiary kartonów i sztuki w każdej paczce. Fmt/Clippy/build zakresowe; bez nowych zależności i migracji.
- Korekta LOC (tokei, 2 zmienione pliki Rust): produkcja 891 → 886 (-5), testy 396 → 427 (+31); bez pustych linii i komentarzy.

## P2 — Cache i łączenie zapytań dostawy — PASS lokalnie

- Usługi 6 h, wyceny 120 s, współdzielenie równoczesnych wyników i krótka przerwa po błędzie. Ograniczone zasoby; anulowanie nie blokuje cache. Oryginalny termin ważności zachowany.
- PASS: 18 testów API/dostawy; jeden oddzielny test porównania taryf sandbox pominięty w tym zestawie. Regresje obejmują TTL, jeden loader, błędy, anulowanie, przepełnienie cache, klucz danych i wersję szkicu.
- PASS: osobna baza PostgreSQL i rzeczywisty sandbox: upload/GLB/slicer → 8 równoczesnych wycen → ten sam zapis/wersja/termin; stare i aktualne wersje retry; starszy endpoint; zmiana magazynu odrzucona. Po restarcie bez Furgonetki ważna oferta działa, obcy właściciel nie ma dostępu. Wygasła oferta i zmieniony adres nie korzystają ze starej ceny.
- PASS: fmt, zakresowe Clippy/build. Bez migracji, nowych plików i zależności. Cache procesu nie łączy zapytań między oddzielnymi instancjami backendu; snapshoty PostgreSQL pozostają trwałe.
- LOC (tokei, 3 zmienione pliki Rust): produkcja 1749 → 1926 (+177), testy 204 → 427 (+223), bez pustych linii i komentarzy.

## CH0–CH4 — Rozmowy przez email — PASS lokalnie

Szczegóły i kryteria: [plan rozmów](chat-email.md).

- CH0 PASS: sprawdź IMAP iCloud oraz prawdziwe „Odpowiedz” z zachowaniem referencji. Brak konfiguracji oznacza NOT RUN.
- CH1 PASS: wspólne uprawnienia/zapis, paginacja i kursor przeczytania; bez limitu 200 wpisów na całą rozmowę.
- CH2 PASS: kopia nowej treści, link właściwego panelu, referencja odbiorcy, grupowanie i trwały outbox.
- CH3 PASS: worker IMAP, bezpieczne przypisanie autora/rozmowy, MIME, limity, deduplikacja i restart.
- CH4 PASS: dwukierunkowy test API → email → odpowiedź → historia, izolacja uczestników i preferencje. Bez pisania strony ani wysyłania do produkcyjnych klientów.

Dowody: 19 testów jednostkowych PASS; osobny test PostgreSQL (205 wpisów, własność, grupowanie, deduplikacja i rollback) oraz migracja starej historii PASS. Rzeczywisty SMTP/IMAP round-trip w obie strony, restart/replay i cofnięcie uprawnienia PASS, wyłącznie void@sieciowiec.xyz. Dwie role w jednej skrzynce; różne konta sprawdzone w DB. Fmt/Clippy/build PASS. Cargo audit: 9 wcześniejszych zgłoszeń, żadne nie dotyczy nowych pakietów IMAP/MIME. Szczegóły i granice: chat-email.md.
- Pomiar tokei (zmienione i nowe źródła Rust): produkcja 1692 → 3034 (+1342), testy 510 → 931 (+421), bez pustych linii/komentarzy. 4 nowe moduły Rust i migracja 0014; nowe biblioteki transportu IMAP/MIME oraz istniejące zależności TLS/HMAC. Bez nowego crate’a aplikacji.


## R1 — Zwroty Stripe — PASS lokalnie, 2026-10-24

- Pełne/częściowe kwoty przez POST/GET `/api/admin/orders/{id}/refunds`; admin/passkey/CSRF, typy w OpenAPI. PostgreSQL: rezerwacja kwoty, UUID ponowienia, autor i trwałe lease/retry; wspólny worker checkoutu, bez nowej kolejki/crate’a.
- PASS: 13 testów jednostkowych API/payments; osobny test PostgreSQL obejmuje współbieżność, idempotencję, sumę, restart/lease, niepewny wynik, błędne mapowanie i limit 23 h. Fmt, zakresowe Clippy `-D warnings`, locked build PASS.
- PASS: świeża rzeczywista płatność Stripe test 1000 gr, zwrot 400 gr i pozostałych 600 gr; Stripe potwierdził pełne 1000 gr. Ponowienie i restart bez duplikatu, nadmiarowa kwota odrzucona, brak dostępu klienta/bez sesji/CSRF, prywatne pola poza JSON/OpenAPI. Sesja admina z fixture; fizycznego passkey nie testowano ponownie. Realny zwrot sprawdzono dla PaymentIntent; gałąź Hosted Checkout nie miała osobnego pełnego przebiegu.
- Granice: tylko Stripe test, bez frontendu/emaili, automatycznego anulowania realizacji oraz importu ręcznych zwrotów Dashboard. Niepewne/wyczerpane operacje trafiają do `manual_review` z rezerwacją kwoty; wymagają uzgodnienia w Stripe. Bez nowych zależności.
- LOC tokei, 8 zmienionych/nowych źródeł Rust: produkcja 2072 → 2652 (+580), testy 326 → 651 (+325); bez pustych linii i komentarzy. Trzy nowe moduły `refunds.rs` i migracja 0015. Kontrakt: [checkout](checkout.md#zwroty-płatności).


## S0–S4 — Nadawanie przesyłek

[Kontrakt i uruchomienie](PLAN.md#s0s4--nadawanie-przesyłek). Bez frontendu/live. Status nie oznacza potwierdzonego rzeczywistego nadania.

| Etap | Stan i dowód |
| --- | --- |
| S0 | Częściowy PASS: konto/usługi, walidacja204, rzeczywista cena i POST dodania200. Zamówienie409 `terms_and_conditions_not_valid`; nie zaakceptowano zgód automatycznie. Komenda nie powstała. |
| S1 | PASS lokalnie: snapshot, rzeczywiste paczki, cena kupionej usługi, admin/passkey-session/CSRF, wersje; ponowienie i refund gating w PG. |
| S2 | Kod + lokalne PG PASS: trwały booking, lease/retry, odzyskiwanie, reopen bez nowych paczek, częściowy sukces. Realne zamówienie/PDF BLOCKED regulaminem; nie raportujemy PASS. |
| S3 | Lokalna logika/parser/PG PASS: wiele numerów, częściowy sukces, dedup email, preferencje i completed/retencja, bez cofania doręczenia. Rzeczywisty tracking i wysyłka nowego szablonu po nadaniu NOT RUN. |
| S4 | BLOCKED: API sandbox sprawdzone do zlecenia nadania (dwa kartony testowe, fixture opłaconego ready i sesji admina); dalsza etykieta → tracking → email czeka na zapis DPD. Fizycznego passkey ani nowej płatności Stripe nie powtarzano. |

Weryfikacja: 33 testy jednostkowe API/shipping/notifications; osobny izolowany PG dostawy oraz regresja zwrotów. Jedna starsza integracja cen sandbox pominięta w jednostkowym zestawie, bez zmiany oczekiwań. Fmt, zakresowe Clippy `-D warnings` i locked build PASS. Pomiar tokei, 15 zmienionych/nowych źródeł Rust: produkcja 3615 → 5911 (+2296), testy 1161 → 1556 (+395), bez pustych linii/komentarzy; osobny tests.rs i moduły inline policzone jako testy. Cztery nowe moduły Rust i migracja 0016; bez nowych crate’ów. Bez nowych zależności. Testowe szkice z sandboxa nie oznaczają kupionych etykiet; żadnych live/nadań z podjazdem.
