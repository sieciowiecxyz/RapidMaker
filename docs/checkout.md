# Checkout — kontrakt backendu

Rust/Axum + PostgreSQL; wyłącznie Stripe test i Furgonetka sandbox. Bez nowego frontendu. [Swagger](http://localhost:18430/api) pokazuje aktualne DTO i błędy.

## Przebieg

1. Upload i wycena slicera dają własne `analysis_id` oraz `quote_id`.
2. `POST /api/checkout/drafts` zapisuje części, odbiorcę i metodę dostawy; brakujące dane zwraca jako `missing_fields`.
3. `PATCH /api/checkout/drafts/{id}` z `version` zmienia wskazane pola i unieważnia ofertę. `recipient`/`delivery` zastępują cały swój obiekt; `recipient.invoice:null` usuwa dane firmy.
4. `GET /api/checkout/drafts/{id}/points` używa zapisanego kodu/miasta. Wybrany `delivery.point_id` jest ponownie sprawdzany przed wyceną.
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

## Trwałość i bezpieczeństwo

- Własność konto/gość; sesja konta wymaga CSRF przy zapisie. Szkic ma 24 h, oferta najwyżej 10 min i nie dłużej niż snapshot slicera. Rozpoczęta płatność zamraża dane. Claim gościa przenosi też szkice i próby.
- PostgreSQL: `checkout_drafts`, `payment_attempts`, `stripe_events`; istniejące `orders` i outbox. Jedna aktywna próba na szkic, jedno zamówienie na próbę; powtórzenia rozpoznaje zapisany klucz/fingerprint. Wybrana usługa, punkt i paczki pozostają w `orders.fulfillment`. Historyczny brak informacji pozostaje NULL.
- Przed Stripe atomowo zapisujemy kwotę, dane, próbę, zamówienie i ochronę plików. Aktualny cennik i dostępność sprawdzane także w transakcji. HTTP dostawcy jest poza nią. Nie ma globalnego mutexa checkoutu ani kolejki w RAM.
- Worker Tokio uzgadnia próby i zdarzenia przez PostgreSQL lease 120 s; ograniczone retry. Tworzenie z tymi samymi parametrami może być ponawiane najwyżej przez 23 h. Po tym bez ID dostawcy → `manual_review`, bez kolejnej płatności i bez zwalniania plików. Znane próby mają limit 1000 odczytów; zdarzenia 20 prób, później jawny stan do interwencji.
- Podpis webhooka: surowe body do 1 MiB, HMAC SHA-256, tolerancja 300 s. Wyjątek od cookie/Origin/CSRF dotyczy wyłącznie `/api/webhooks/stripe`. `2xx` po trwałym przyjęciu; worker nie polega na kolejności zdarzeń. Kontroluje ID, metadata, kwotę, PLN i `livemode:false` po odczycie ze Stripe.
- `paid` i email zapisują się atomowo i jednokrotnie. Niepewna płatność chroni pliki. Potwierdzone wygaśnięcie/anulowanie pozwala je zwolnić; opłacone korzystają z obecnej retencji zamówień. Stan płatności (`payment_state`) jest osobny od produkcji (`status`). Potwierdzony błąd opóźnionej płatności Hosted Checkout trafia do `manual_review` z zachowaniem plików; wymaga uzgodnienia przed ich zwolnieniem.
- Limity: 30 szkiców i prób/właściciela/dobę, 10 tys. oczekujących globalnie; dostawa 30 zapytań/min/właściciela, 300/min globalnie, 4 równoległe operacje dostawców w procesie. JSON i odpowiedzi mają limity rozmiaru/czasu. Sekretów, adresów i danych kart nie logujemy.
- Zakończone zdarzenia Stripe: deduplikacja 30 dni; szkice bez prób sprzątane po TTL przy kolejnym utworzeniu. Zamówienia, próby i nierozstrzygnięte przypadki zachowujemy do audytu/rozstrzygnięcia; brak automatycznego usuwania ich po 24 h.

Stare `/api/checkout/quote`, `/session`, `/intent`, statusy i resume pozostają adapterami tego samego trwałego silnika. Wygasłe dawne oferty RAM trzeba przeliczyć; istniejące zamówienia ze Stripe zachowują odczyt statusu. Sekret PaymentIntent zwraca tylko autoryzowany endpoint resume; nigdy szkic ani log. Frontend kont/admina nadal wymaga migracji auth/CSRF.

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
