# Logi backendu

`app` inicjalizuje raz `tracing` przez istniejący crate `observability`. Jeden JSON na linię, na **stderr**; stdout CLI i workera geometrii zachowuje swój protokół. Biblioteki emitują zdarzenia, nie instalują własnego loggera.

## Użycie

Domyślnie `LOG_LEVEL=info`. Obsługiwane: `off`, `error`, `warn`, `info`, `debug`, `trace`. Można ustawić zmienną przed uruchomieniem albo w lokalnym `.env`; launcher ją odczytuje. Błędna wartość zatrzymuje start, bez wypisywania jej treści.

```sh
LOG_LEVEL=debug python backend/run.py 2> /tmp/rapidmaker.jsonl
tail -f /tmp/rapidmaker.jsonl | jq .
```

Nie uruchamiaj drugiej instancji na zajętym porcie. Backend nie utrzymuje własnego pliku ani rotacji; przekierowanie lub kolektor zarządza zapisem i retencją.

## Zawartość

- `timestamp`, `level`, `target`, `fields.event`; zależnie od zdarzenia: `reason`, `stage`, `status`, `elapsed_ms` i bezpieczne liczniki.
- HTTP: własny UUID w nagłówku odpowiedzi `x-request-id`; w kontekście `span` — `request_id`, metoda i szablon trasy, np. `/api/admin/orders/{id}/shipment`. Nieznane trasy: `unmatched`; inne niż standardowe metody: `OTHER`. Pomijamy identyfikator dostarczony przez klienta.
- Log HTTP powstaje po przygotowaniu odpowiedzi; czas nie obejmuje pełnego pobierania jej body. Udane GET/HEAD/OPTIONS są na `debug`, mutacje na `info`, 4xx na `warn`, 5xx na `error`.
- Workery mają kontekst zadania/próby/zamówienia/operacji. Zdarzenia zapisania zlecenia łączą HTTP z późniejszą pracą po ID. Kontekst asynchroniczny używa `Instrument`, bez trzymania guardów spanów przez `await`.
- Baza/migracje, auth/passkey, upload, wyceny, płatności/zwroty, nadania/podjazdy/anulowanie, SMTP/IMAP i chat mają zdarzenia na swoich granicach. Częste odpytywanie i obliczenia są na `debug`; bez logu każdej figury, sztuki lub pustej kolejki.

Nie logujemy nagłówków, URL-i z parametrami, body, danych adresowych, emaili, wiadomości, kodów, sesji, kluczy, hashy właściciela, nazw modeli, ścieżek klientów ani surowych błędów/odpowiedzi dostawców. Kategorie błędów są jawne i ograniczone. Hook paniki zapisuje lokalizację **w kodzie źródłowym**, bez jej payloadu. Logi zależności są wyłączone nawet przy `trace`, aby ich SQL lub diagnostyka protokołów nie omijały tej zasady.

JSON można zbierać w Loki i oglądać w Grafanie; kolektor, dashboardy, metryki i nowe endpointy health/readiness nie są częścią tej zmiany.
