# Milestones — stabilny checkout backend

C1–C6: TODO. Tylko Rust/PostgreSQL/API; bez strony. Decyzje i kontrakt: [PLAN.md](PLAN.md). B1–B8 pozostają ukończonym lokalnym POC; dowody w [checkpoincie](milestones/backend.md).

## C1 — Adres i kontrakt

- Typowane DTO szkicu, odbiorcy, opcjonalnej faktury, dostawy i błędów; jeden obiekt odbiorcy dla dalszego procesu.
- Walidacja PL, normalizacja i wymagania zależne od kuriera/punktu/odbioru. Pusty formularz może być szkicem, ale nie płatnym checkoutem.
- Ustal zgodność istniejących tras, mapowanie stanów oraz wersję Stripe API. Aktualizuj OpenAPI wraz z implementacją; nie publikuj niezaimplementowanych tras.
- Gotowe gdy: krótki test poprawnych danych i błędów pól, odbioru bez adresu oraz odrzucenia nadmiarowych/za dużych danych. Nie uznaj składni za potwierdzenie istnienia adresu.

## C2 — Trwały szkic i snapshot

- Migracje `checkout_drafts`, `payment_attempts`, `stripe_events` i potrzebne rozszerzenie `orders`; bez drugiego modelu zamówień.
- Zapis/odczyt/zmiana własnego szkicu, kontrola wersji, limity i TTL. Claim gość → konto przenosi także szkice bez utraty powiązań. Nigdy nie uprawnia cudzy identyfikator.
- Zmiana danych unieważnia ofertę; brak ważnej własnej wyceny lub dostępnego materiału blokuje płatność. Uwzględnij dotychczasowe zamówienia i holdy w migracji.
- Gotowe gdy: dane przetrwają restart; obcy dostęp jest odrzucony; dwie równoczesne zmiany nie nadpisują się; migracja działa na pustej bazie i istniejącym schemacie.

## C3 — Dostawa z zapisanego adresu

- Podłącz punkty i oferty Furgonetki do szkicu; adres punktu weryfikuje dostawca. Odbiór osobisty działa bez Furgonetki.
- Użyj istniejącego pakowania, danych serwera oraz kosztu całej przesyłki wielopaczkowej. Oferta zawiera metodę, zweryfikowany punkt, paczki, koszt i ważność.
- Rozdziel błędne pola, brak usługi i niedostępność dostawcy. Za duże/niewycenialne zamówienie kieruj do ręcznej wyceny bez uruchamiania płatności.
- Gotowe gdy: sandbox zwraca ofertę i punkty dla zapisanych danych; wybrany punkt odpowiada usłudze; przypadki 1/1000 sztuk nie korzystają ze stałej ceny jednej paczki; timeout nie daje pustej ani darmowej oferty. Wystarczy jeden rzeczywisty przebieg dostawcy i kilka lokalnych regresji.

## C4 — Jedna trwała próba Stripe

- Przed siecią zapisz oczekujące zamówienie, snapshot kwoty/dostawy, próbę, idempotency key i hold plików. Stripe Hosted Checkout otrzymuje wyłącznie dane serwera.
- Bez globalnego mutexa ani transakcji DB obejmującej HTTP. Unikalność/lease w PostgreSQL; ponowienie wraca do tej samej próby. Obecne wejścia PaymentIntent/Checkout respektują tę samą blokadę.
- Powrót, anulowanie i status nie ustawiają samodzielnie `paid`. Timeout tworzenia zachowuje stan niepewny i ochronę plików.
- Gotowe gdy: sandbox tworzy sesję z dokładną kwotą/PLN; równoległe żądania, restart i utrata odpowiedzi nie tworzą drugiej płatności; zmieniony payload z tym samym kluczem daje 409. Zły właściciel, cena klienta i wygasła oferta są odrzucane.

## C5 — Webhook i rozstrzyganie awarii

- Weryfikuj podpis na surowym body; zapisuj zdarzenie przed `2xx`. Wąski wyjątek od ochrony browserowej wyłącznie dla webhooka.
- Worker z ograniczonym retry/lease przetwarza zdarzenia i uzgadnia niepewne próby. Waliduje powiązanie, tryb, kwotę/walutę i zapłatę; deduplikacja po ID zdarzenia oraz przejściu próby.
- `paid` + email atomowo; duplikat/zmieniona kolejność nie cofają stanu i nie dodają maila. Obsłuż płatność opóźnioną, wygaśnięcie, błąd oraz webhook przychodzący przed zapisem odpowiedzi Stripe.
- Nie ponawiaj tworzenia po wygaśnięciu okna idempotencji bez rozstrzygnięcia. Hold zwalniaj tylko dla potwierdzonego zakończenia płatności; `unknown` pozostaje chronione i widoczne operatorowi przez API/CLI.
- Gotowe gdy: zły/stary podpis odrzucony; ponowienie i kolejność zdarzeń bez podwójnych skutków; awaria DB nie potwierdza przyjęcia; restart workera wznawia pracę. Test granicy dotyczy też holdów i braku fałszywego `paid`.

## C6 — Jeden pełny przebieg i zamknięcie

- HTTP/CLI: własny model i wycena → szkic/adres → dostawa → Stripe test → webhook → trwałe `paid` i pojedynczy wpis outbox. Email testowy wyłącznie do void@sieciowiec.xyz.
- Sprawdź odczyt po restarcie i jedną próbę obcego dostępu. Swagger opisuje rzeczywiste DTO/błędy; krótka instrukcja uruchamiania i odzyskiwania niepewnej płatności.
- Gotowe gdy: zakresowe fmt/Clippy/build/testy PASS i dowód sandboxa. Brak klucza/webhooka oznacz NOT RUN, nie PASS. Istniejące podatności zależności raportuj oddzielnie; lokalny sukces nie oznacza gotowości produkcyjnej.

## Sposób pracy i blokery

Małe commity po etapie; krótki test rzeczywistego ryzyka, bez Astry i testujących subagentów. Rust LOC mierz przed/po z podaniem zakresu. Nie zmieniaj frontendu; migracja jego adapterów jest osobną pracą. Nie nadawaj przesyłek ani nie uruchamiaj Stripe live.

Sekret webhooka/Stripe CLI mogą wymagać autoryzacji właściciela. Brak danych nadawcy, aktywnych usług sandbox, cen lub materiałów blokuje odpowiedni test integracyjny, ale nie kontrakt, migracje i lokalne testy awarii. Obsługa zwrotów, etykiety i publiczne wdrożenie pozostają poza C1–C6.
