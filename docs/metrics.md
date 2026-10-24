# Metryki dla Grafany

Przepływ: Rust → Prometheus → Grafana. Rust wystawia OpenMetrics; Prometheus zapisuje historię, Grafana odczytuje ją ze źródła Prometheus. Ten etap nie instaluje tych serwerów.

## Dostęp

`METRICS_TOKEN` to osobny losowy sekret: 32 bajty zapisane jako 64 znaki hex, w lokalnym `.env` i zaszyfrowanym SOPS. Launcher go przekazuje; brak zmiennej wyłącza listener i zbieranie. Pusty/błędny token zatrzymuje start. Po zmianie zrestartuj API. Nie używaj kluczy Stripe, Furgonetki ani sesji użytkownika.

Adres: `http://127.0.0.1:18435/metrics`, wyłącznie loopback, nagłówek `Authorization: Bearer TOKEN`. Port 18432 i proxy Vite nie udostępniają metryk. Bez CORS; żądania z Origin, query string lub obcym Host są odrzucane. Weryfikacja hasha tokena w stałym czasie. Maksymalnie jeden eksport/s, bez kolejkowania równoległych eksportów i bez zapytań do DB. Odpowiedź `no-store`; nie logujemy tokena ani żądań eksportera. Brak dostępu: 401, zbyt częsty odczyt: 429.

Nie wystawiaj portu w publicznym reverse proxy. Zdalny Prometheus wymaga prywatnego tunelu z szyfrowaniem albo uwierzytelnionego proxy TLS; sam token po HTTP chroni tylko lokalny ruch. Dostęp do metryk i Grafany ogranicz do operatorów.

## Dane

| Metryka | Znaczenie |
|---|---|
| `rapidmaker_http_requests_total` | ukończone odpowiedzi według szablonu trasy, metody i klasy statusu |
| `rapidmaker_http_duration_seconds` | histogram czasu przygotowania odpowiedzi według trasy/metody |
| `rapidmaker_http_in_flight` | liczba żądań przygotowujących odpowiedź; anulowanie zwalnia licznik |
| `rapidmaker_uptime_seconds` | czas od inicjalizacji zbierania w procesie |

Trasy to stałe szablony Axum, np. `/api/models/{id}`; nieznane to `unmatched`. Metody są z zamkniętej listy lub `OTHER`, statusy to klasy 1xx–5xx/other. Brak ID klientów/zamówień, surowych URL, query, emaili, IP, treści, nagłówków i błędów dostawców. Liczba serii zależy od zarejestrowanych tras, nie liczby klientów. Eksport nie wlicza sam siebie. Czas nie obejmuje pobrania body ani osobnej pracy slicera. Liczniki resetują się po restarcie; używaj `rate`/`increase`.

To metryki HTTP jednego procesu; nie obejmują jeszcze głębokości kolejek, czasu slicera, zasobów hosta ani PostgreSQL. Dla osobnego `--slicer-worker` listener nie jest uruchamiany.

## Prometheus i Grafana

Przykład dla Prometheusa działającego na tym samym hoście. Zapisz **sam token**, bez `METRICS_TOKEN=`, do pliku poza repo o prawach 0600, czytelnego dla procesu Prometheusa. Nie wklejaj sekretu do YAML, dashboardu ani adresu URL. W kontenerze `127.0.0.1` oznacza jego własną przestrzeń sieciową — potrzebny jest jawnie skonfigurowany dostęp do loopback hosta.

```yaml
scrape_configs:
  - job_name: rapidmaker
    scrape_interval: 15s
    scrape_timeout: 5s
    sample_limit: 25000
    body_size_limit: 4MB
    static_configs:
      - targets: ['127.0.0.1:18435']
    authorization:
      type: Bearer
      credentials_file: /run/secrets/rapidmaker_metrics_token
```

W Grafanie dodaj źródło Prometheus. Przydatne zapytania:

```promql
# żądania/s
sum(rate(rapidmaker_http_requests_total[5m]))
# błędy serwera/s
sum(rate(rapidmaker_http_requests_total{status_class="5xx"}[5m]))
# p95 czasu przygotowania odpowiedzi
histogram_quantile(0.95, sum by (le) (rate(rapidmaker_http_duration_seconds_bucket[5m])))
# aktywne żądania
rapidmaker_http_in_flight
# dostępność eksportera (dodaje Prometheus)
up{job="rapidmaker"}
```

Prometheus/Grafana wymagają własnej konfiguracji prywatnego dostępu i retencji. Nie udostępniaj ich anonimowo w Internecie. Zalecenia: [model bezpieczeństwa Prometheusa](https://prometheus.io/docs/operating/security/).
