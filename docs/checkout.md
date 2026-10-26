# Checkout — kontrakt backendu

Rozpoczęcie płatności wymaga aktualnego `legal_version` również przez endpointy zgodności. Zapis wersji/czasu i terminu produkcji jest atomowy z zamówieniem; po potwierdzonej zapłacie outbox wysyła specyfikację i pełne dokumenty. Retry nie dubluje kopii. [Warunki i ograniczenia](legal.md).

## Zwroty płatności

- `POST /api/admin/orders/{id}/refunds`: `{"request_id":"UUID","amount_grosze":500}`. Kwota brutto w groszach, np. 500 = 5 zł; pełny zwrot to pozostała opłacona kwota. Wymaga sesji admina po passkey oraz CSRF/Origin. `202` potwierdza zapis zlecenia, nie zwrot pieniędzy.
- `GET /api/admin/orders/{id}/refunds`: lista i aktualne statusy, bez kluczy wewnętrznych. Powtarzaj ten sam `request_id` i kwotę przy utracie odpowiedzi. Nowy UUID oznacza kolejny zwrot; inna kwota dla użytego UUID daje `409`.
- PostgreSQL blokuje zamówienie i sumuje zwroty wykonane oraz zarezerwowane. Nie można zwrócić więcej niż zapłacono. Limit: 100 zleceń/zamówienie i 30 nowych/admina/dobę. `paid_at` i powiązanie płatności są sprawdzane lokalnie, a worker przed mutacją ponownie weryfikuje opłaconą sesję/PaymentIntent w Stripe: ID, metadata, PLN, kwotę i tryb testowy.
- Rejestr `payment_refunds` zachowuje autora, kwotę, powiązanie płatności i stały klucz Stripe. Lease 120 s; wspólny worker checkoutu odpytuje oczekujące zwroty co najmniej po 60 s. Nowe zlecenia podejmuje w kolejnych cyklach workera, bez HTTP do Stripe pod blokadą SQL.
- `queued` — zapisany; `unknown` — niepewny wynik, kwota nadal zarezerwowana; `pending`/`requires_action` — status Stripe; `succeeded` — zwrot potwierdzony; `failed`/`canceled` — środki dostępne do ponownego zlecenia. `manual_review` wymaga sprawdzenia w Stripe i nadal rezerwuje kwotę. Ponowienia nie generują nowego klucza. Bez znanego ID zwrotu najwyżej 10 prób i tylko przed upływem 23 h; potem bez kolejnego POST. Znany zwrot ma do 1000 odczytów.
- Zwrot jest niezależny od statusu produkcji: nie anuluje druku ani wysyłki, nie usuwa plików i nie zmienia opłaconej kwoty historycznej. Na razie bez formularza, emaila o zwrocie i automatycznego importu zwrotów zlecanych ręcznie w Dashboard Stripe. Stripe dodatkowo egzekwuje faktycznie dostępną kwotę. Obsługiwane wyłącznie klucze testowe; `requires_action` wymaga interwencji w Stripe.

## Checkout i dostawa

Rust/Axum + PostgreSQL; wyłącznie Stripe test i Furgonetka sandbox. Bez nowego frontendu. [Swagger](http://localhost:18430/api) pokazuje aktualne DTO i błędy.

## Przebieg

1. Upload i wycena slicera dają własne `analysis_id` oraz `quote_id`.
2. `POST /api/checkout/drafts` zapisuje części, odbiorcę i metodę dostawy; brakujące dane zwraca jako `missing_fields`.
3. `PATCH /api/checkout/drafts/{id}` z `version` zmienia wskazane pola i unieważnia ofertę. `recipient`/`delivery` zastępują cały swój obiekt; `recipient.invoice:null` usuwa dane firmy.
4. `GET /api/checkout/drafts/{id}/points` używa zapisanego kodu/miasta. Wybrany `delivery.point_id` jest ponownie sprawdzany przed wyceną. Przekaż `Point.code` bez zmian: InPost zachowuje dawny kod, inne sieci używają `orlen:CODE`, `dpd:CODE`, `dhl:CODE`. Dostępne są automaty InPost, ORLEN Paczka, DPD i DHL; punkt jest wiązany z właściwym przewoźnikiem. Lista do 12 aktywnych automatów nie gwarantuje obecności każdej sieci.
5. `POST /api/checkout/drafts/{id}/quote` z `version` zapisuje paczki, ceny i ważność; zwraca nową wersję. `manual_required` oznacza brak automatycznej oferty.
6. `POST /api/checkout/drafts/{id}/payment` z `{version,offer_id}` i `Idempotency-Key` zwraca URL Hosted Checkout (`200`) lub próbę oczekującą na rozstrzygnięcie (`202`). Ponawiaj ten sam klucz i payload. `409` oznacza konflikt, nie zgodę na tworzenie nowej należności.
7. `GET /api/checkout/drafts/{id}` zwraca zapisany stan również po restarcie. Powrót ze Stripe nie potwierdza płatności. Podpisany webhook i odczyt obiektu Stripe potwierdzają `paid`.

Minimalny szkic:

```json
{
  "parts": [{"id":"czesc-1","quantity":2,"analysis_id":"ID_MODELU","quote_id":"ID_WYCENY_SLICERA"}],
  "recipient": {"name":"Jan Kowalski","email":"void@sieciowiec.xyz","phone":"500100200","country":"PL","street":"Przykładowa 3","apartment":"","postal_code":"05-500","city":"Piaseczno"},
  "delivery": {"method":"courier"},
  "email_order_updates": true
}
```

Metody: `courier`, `locker` z ID punktu, `pickup`. Odbiór osobisty nie wymaga adresu ani przewoźnika; punkt wymaga kontaktu i ID. Opcjonalna `recipient.invoice`: `company,nip,street,postal_code,city,country`. To dane do faktury, bez automatycznego jej wystawiania. Poprawny zapis adresu nie dowodzi jego istnienia.

`422` wskazuje `field_errors`; brak pasującej usługi daje ręczną wycenę; `503` oznacza awarię/konfigurację dostawcy; `429` limit zapytań. Najtańsza zatwierdzona oferta ma `recommended:true`. `total_grosze` zawiera wydruki, opakowania i wszystkie paczki. Liczby i wymiary paczek pochodzą z serwera.

## Wybór pakowania i ceny dostawy

Backend generuje do 5 różnych planów: dotychczasowy układ mieszany oraz pełne kartony każdego rozmiaru. Każdy karton jest wypełniany przed otwarciem następnego; ostatni zawiera resztę. Pozostają tylko plany z najmniejszą znalezioną liczbą paczek. Dopiero wśród nich porównujemy koszty. Duplikaty wymiarów/mas paczek i kosztu opakowań odpadają. Zachowane limity: 1–1000 sztuk/model, do 2000 łącznie, 30 paczek, 20 kg/paczkę; punkt odbioru nadal tylko jedna paczka. Obecne kartony, odstęp 4 mm na stronę i rezerwa 33% pozostają niekalibrowanymi założeniami POC.

`shipping::quote_best` pobiera usługi i weryfikuje punkt raz, wycenia każdy plan w Furgonetce, a następnie waliduje pary plan/usługa od najniższego kosztu przewozu wszystkich paczek + opakowań. Cena obejmuje wszystkie zlecenia przewozu: automaty oraz kurier UPS/InPost po jednym kartonie, pozostali kurierzy do 15 kartonów w zleceniu, każdorazowo z walidacją dostawcy. Identyczne zlecenia są liczone raz i mnożone; brak ceny choćby jednego wyklucza całą ofertę. Maksymalnie 20 obliczeń ceny, 30 walidacji, 30 s na operację; awaria dostawcy/timeout daje 503, bez pozornej darmowej dostawy. Brak zatwierdzonej pary oznacza ręczną wycenę albo błąd konkretnego pola.

Wynik zawiera zwycięski `packing` i do 3 zatwierdzonych usług **dla tego samego układu**. `recommended` wskazuje najtańszą zatwierdzoną parę spośród sprawdzonych wariantów o najmniejszej znalezionej liczbie paczek dla wybranej metody; nie porównuje automatycznie kuriera z punktem ani paletą. Ten układ i jego koszt opakowań trafiają do istniejącego snapshotu PostgreSQL i zamówienia. Kontrakt HTTP i schemat SQL bez zmian. Bez dokładnego nestingu i gwarancji matematycznego optimum.

## Cache i identyczne żądania

- Lista usług konta Furgonetki: wspólny cache w procesie na 6 h, odświeżany dopiero przy żądaniu. Jedno równoczesne odświeżenie; błąd nie zastępuje wyniku sukcesem ani nie pozwala użyć starej listy po TTL.
- Wyceny dostawy: cache na 120 s i łączenie identycznych zapytań przez `OnceCell`, także dla starego endpointu tworzącego nowe szkice. Klucz obejmuje kolejność planów, wymiary/masy wszystkich paczek, opakowania, pełnego znormalizowanego odbiorcę i metodę/punkt. Identyfikatory modeli i pozycje części nie wpływają na taryfę. Cache przypisany do jednej konfiguracji konta/nadawcy, współdzielony przez klony adaptera.
- Limit 64 wpisów/128 KiB na klucz i 4 równoczesne obliczenia dostawcy; czekający na identyczny wynik nie zajmują dodatkowych slotów. Błędy i ręczna wycena mają tylko 2 s przerwy przed ponowieniem. Anulowanie obliczenia nie blokuje następnych żądań. Wygasłe wpisy usuwane przy dostępie; brak automatycznego odpytywania dostawcy.
- Ważna, automatyczna oferta szkicu wraca z PostgreSQL bez HTTP do Furgonetki i bez zwiększania wersji. Najpierw sprawdzamy właściciela, model, snapshot slicera, cennik i dostępność. Akceptowana aktualna wersja albo wersja dokładnie tego żądania, które zapisało ofertę; edycja nadal unieważnia snapshot. Równoczesne zapisy tego samego wyniku zwracają zwycięski snapshot.
- Trafienie w cache nie przedłuża ważności ceny. Oferta kurierska/punktowa wygasa najpóźniej z pierwotnym wynikiem dostawcy (120 s), wyceną slicera lub szkicem. Odbiór osobisty zachowuje dotychczasowe maksimum 10 min. Po wygaśnięciu nowa wycena wymaga aktualnej wersji szkicu.
- Cache usług i łączenie obliczeń działają w jednym procesie; resetują się po restarcie. Trwały snapshot oferty zachowuje ważność w PostgreSQL. Bez Redis, brokera, zmian frontendu i nowych tabel. Cache nie jest gwarancją niezmienności taryfy produkcyjnej przewoźnika.

## Trwałość i bezpieczeństwo

- Własność konto/gość; sesja konta wymaga CSRF przy zapisie. Szkic ma 24 h, oferta najwyżej 10 min i nie dłużej niż snapshot slicera. Rozpoczęta płatność zamraża dane. Claim gościa przenosi też szkice i próby.
- PostgreSQL: `checkout_drafts`, `payment_attempts`, `stripe_events`; istniejące `orders` i outbox. Jedna aktywna próba na szkic, jedno zamówienie na próbę; powtórzenia rozpoznaje zapisany klucz/fingerprint. Wybrana usługa, punkt i paczki pozostają w `orders.fulfillment`. Historyczny brak informacji pozostaje NULL.
- Przed Stripe atomowo zapisujemy kwotę, dane, próbę, zamówienie i ochronę plików. Aktualny cennik i dostępność sprawdzane także w transakcji. HTTP dostawcy jest poza nią. Nie ma globalnego mutexa checkoutu ani kolejki w RAM.
- Worker Tokio uzgadnia próby i zdarzenia przez PostgreSQL lease 120 s; ograniczone retry. Tworzenie z tymi samymi parametrami może być ponawiane najwyżej przez 23 h. Po tym bez ID dostawcy → `manual_review`, bez kolejnej płatności i bez zwalniania plików. Znane próby mają limit 1000 odczytów; zdarzenia 20 prób, później jawny stan do interwencji.
- Podpis webhooka: surowe body do 1 MiB, HMAC SHA-256, tolerancja 300 s. Wyjątek od cookie/Origin/CSRF dotyczy wyłącznie `/api/webhooks/stripe`. `2xx` po trwałym przyjęciu; worker nie polega na kolejności zdarzeń. Kontroluje ID, metadata, kwotę, PLN i `livemode:false` po odczycie ze Stripe.
- `paid` i email zapisują się atomowo i jednokrotnie. Niepewna płatność chroni pliki. Potwierdzone wygaśnięcie/anulowanie pozwala je zwolnić; opłacone korzystają z obecnej retencji zamówień. Stan płatności (`payment_state`) jest osobny od produkcji (`status`). Potwierdzony błąd opóźnionej płatności Hosted Checkout trafia do `manual_review` z zachowaniem plików; wymaga uzgodnienia przed ich zwolnieniem.
- Limity: 30 szkiców i prób/właściciela/dobę, 10 tys. oczekujących globalnie; dostawa 30 zapytań/min/właściciela, 300/min globalnie, 4 równoległe obliczenia dostawy oraz dotychczasowy limit 4 pozostałych operacji dostawców w procesie. JSON i odpowiedzi mają limity rozmiaru/czasu. Sekretów, adresów i danych kart nie logujemy.
- Zakończone zdarzenia Stripe: deduplikacja 30 dni; szkice bez prób sprzątane po TTL przy kolejnym utworzeniu. Zamówienia, próby i nierozstrzygnięte przypadki zachowujemy do audytu/rozstrzygnięcia; brak automatycznego usuwania ich po 24 h.

Stare `/api/checkout/quote`, `/session`, `/intent`, statusy i resume pozostają adapterami tego samego trwałego silnika. Utworzenie płatności/status zwracają autoryzowany `order_id` trwałego zamówienia; UI klienta używa go do linku szczegółów, nigdy lokalnego UUID szkicu. Wygasłe dawne oferty RAM trzeba przeliczyć; istniejące zamówienia ze Stripe zachowują odczyt statusu. Sekret PaymentIntent zwraca tylko autoryzowany endpoint resume; nigdy szkic ani log. Frontend kont/admina nadal wymaga migracji auth/CSRF.

## Lokalny webhook i odzyskiwanie

`.env`: istniejące klucze testowe Stripe/Furgonetki i nowy `STRIPE_WEBHOOK_SECRET`. Launcher go odczytuje; hook przed commitem zapisuje SOPS.

```sh
stripe listen --events checkout.session.completed,checkout.session.expired,checkout.session.async_payment_succeeded,checkout.session.async_payment_failed,payment_intent.succeeded,payment_intent.processing,payment_intent.payment_failed,payment_intent.canceled --forward-to http://127.0.0.1:18432/api/webhooks/stripe
```

CLI musi korzystać z tego samego konta testowego co backend. Sekret listenera zapisz lokalnie do `.env`, potem uruchom backend ponownie; nie wklejaj sekretu do czatu/Git. Lokalnie nie trzeba publicznego endpointu. Token Furgonetki może wymagać odnowienia.

```sh
python backend/run.py --checkout-status UUID_PROBY
python backend/run.py --checkout-status UUID_PROBY ORYGINALNE_ID_STRIPE
python backend/run.py --checkout-close UUID_PROBY
```

Pierwsze polecenie odczytuje i uzgadnia znaną płatność. Drugie odzyskuje referencję po utracie odpowiedzi: Stripe musi potwierdzić metadata, kwotę i walutę. Trzecie zamyka otwarty Hosted Checkout i zapisuje potwierdzone wygaśnięcie; nie tworzy nowej płatności. Dawny PaymentIntent anuluj w Stripe i uzgodnij pierwszym poleceniem. Nie oznaczaj `unknown` jako nieopłaconego ręcznie w SQL.

Testy lokalne nie obejmują Stripe live, publicznego TLS/proxy, refundacji ani nadawania paczek. Pakowanie pozostaje ostrożnym szacunkiem wymagającym kalibracji; samo pole volume nie dowodzi fizycznego dopasowania. Pełny przebieg sandbox sprawdzamy przez API/Stripe CLI, bez pisania strony.
