# Email i powiadomienia — plan implementacji

Status 2026-10-23: E1–E4 PASS jako lokalny backend. Wdrożone: logowanie klienta, trwałe zamówienia/rozmowy, outbox i preferencje. Frontend nadal wymaga podłączenia; admin używa obecnej sesji operatora. Pełna migracja auth/passkey i webhooki produkcyjne pozostają osobnym zakresem.

Dowód E2–E4: rzeczywisty kod z iCloud odczytany przez IMAP został zużyty przez HTTP raz; inna przeglądarka i ponowienie odmówione. Sesja i zamówienie przetrwały restart. CSRF, cudze zamówienia/rozmowy i unieważnienie sesji sprawdzone. Wyłączenie preferencji zablokowało opcjonalne zdarzenia; chat klienta zapisał się i worker wysłał alert operatorowi. Odmowa SMTP unieważniła próbę. 20 zwykłych testów + 4 testy PostgreSQL, fmt/Clippy/build PASS. Rust +2233 linii łącznie z testami inline (tokei, cały backend/crates, bez pustych linii i komentarzy); crypto używa hmac/sha2/subtle/getrandom, bez nowych crate’ów aplikacji.

Dowód E1: lokalna komenda Rust wysłała jedną wiadomość do `void@sieciowiec.xyz`; SMTP zaakceptowało ją, IMAP potwierdził odbiorcę/nadawcę, temat i tekst wiadomości. Build, fmt, Clippy oraz 4 testy notifications i 11 API PASS. Rust produkcyjny +249 LoC, testy +101 (tokei, nowe pliki oraz zmienione lib.rs API/main.rs app, bez pustych linii i komentarzy). Jedna nowa bezpośrednia zależność `lettre`; aktualizacja istniejących rustls-webpki/webpki-roots usuwa 2 zgłoszenia TLS. Audyt całego lockfile nadal zgłasza 9 wcześniejszych podatności w lz4_flex, quick-xml, rsa i zip — nie jest czysty.

## Cel i stan

`notifications` wysyła przez iCloud SMTP. `auth` dostarcza crypto, `storage` zapisuje sesje, zamówienia, wiadomości i outbox; `api` orkiestruje HTTP i jeden worker. `app` zatrzymuje HTTP/workera po SIGINT/SIGTERM. Bez brokera i dodatkowej usługi. Lokalna komenda `--mail-test` nadal działa bez DB/HTTP.

Konfiguracja klienta: `AUTH_MODE=localhost`, `AUTH_CODE_KEY` = 32 losowe bajty zapisane hex w `.env`/SOPS; `AUTH_ORIGIN` domyślnie `http://127.0.0.1:18430`. Brak trybu oznacza disabled. Publiczny tryb auth celowo nie jest jeszcze obsługiwany; ciasteczka tej implementacji są przeznaczone dla localhost.

API: `/api/auth/{start,verify,me,logout,logout-all}`, `GET/PUT /api/account/notifications`, `/api/account/orders/{id}` i `/messages`, `POST /payment-status` z `{}`. Mutacje zalogowanego klienta wymagają Origin, JSON i `X-CSRF-Token` z `/me`. `POST /api/auth/claim-guest {}` przypisuje zamówienia tylko z jednoczesną sesją klienta i cookie gościa; nie wyszukuje zakupów po emailu. Gość ma dostęp cookie przez 24 h, zamówienie przypisane do konta wymaga tego konta.

Operator: `/api/admin/orders`, `POST /{id}/status`, `GET/POST /{id}/messages`, `GET /api/admin/notifications` (liczby zaległych/błędnych zadań), pod istniejącą ochroną panelu. Wiadomość: `{body}`, status: `{status}`; statusów płatności nie nadaje operator.

Zamówienie powstaje z serwerowej wyceny po utworzeniu Stripe intent/session. Potwierdzenie płatności zapisuje się z mailem atomowo dopiero po weryfikacji odpowiedzi Stripe (ID, metadata, kwota, waluta i test mode). Obecne endpointy statusu oraz własny `/payment-status` wykonują tę weryfikację; drugi działa po restarcie (limit raz/15 s na zamówienie). Nie jest to webhook ani automatyczne monitorowanie płatności bez odpytywania. Zmiany realizacji i chat mają własne trwałe zdarzenia i limity.

Na start wysyłka tekstowa: kod logowania, potwierdzenie zamówienia/płatności, zmiana statusu i informacja o nowej wiadomości. Bez marketingu, załączników i silnika szablonów. Odpowiadanie na chat przez email to późniejszy, osobny zakres.

## Granice kodu

```text
backend/crates/
  notifications/src/
    lib.rs          # Mailer, konfiguracja i wysyłka SMTP
    messages.rs     # konkretne treści wiadomości, bez SQL/HTTP
  storage/src/      # auth.rs, email.rs, orders.rs — SQL i transakcje
  api/              # odbiorca, uprawnienia, zdarzenie i orkiestracja
  app/              # start i sygnał zakończenia; worker w api/email.rs
```

Rozwijamy istniejące crate’y. Jedna nowa bezpośrednia zależność: `lettre` 0.11 do MIME, parsowania adresów i SMTP; Tokio/rustls, wyłącznie potrzebne features zgodne z obecnym backendem TLS. Bez własnego SMTP, interfejsu wielu dostawców, brokera i dodatkowego serwisu. `api` wybiera odbiorcę i rodzaj wiadomości; `notifications` składa i wysyła ją, nie ustala uprawnień ani statusu zamówienia.

## Adresy i konfiguracja

- Zachowaj istniejące `MAIL_SMTP_HOST`, `MAIL_SMTP_PORT`, `MAIL_USER`, `MAIL_PASSWORD`, `MAIL_FROM`, `MAIL_REPLY_TO`, `MAIL_TEST_TO`. Dodaj do allowlisty launchera tylko potrzebne klucze; bez dublowania sekretów. `.env` lokalnie, SOPS w Git, istniejący hook.
- iCloud: `smtp.mail.me.com:587`, wymagane STARTTLS z weryfikacją certyfikatu, hasło aplikacji. Login SMTP jest osobny od adresu nadawcy. `From` musi być adresem/aliasem dopuszczonym w koncie iCloud; na start `void@sieciowiec.xyz`. `Reply-To` również pochodzi z konfiguracji operatora.
- Dodaj jawne `MAIL_MODE=disabled|test|live`, domyślnie `disabled`. `test` pozwala wysłać wyłącznie do `MAIL_TEST_TO` (na start `void@sieciowiec.xyz`); innego odbiorcę odrzuca. Nie przekierowuj kodów logowania innych osób do skrzynki testowej. `live` wymaga świadomej konfiguracji; braki lub błędne adresy blokują start włączonego modułu.
- Odbiorca kodu pochodzi z ograniczonego żądania auth; odbiorca powiadomienia z uprawnionego konta/zamówienia w DB. Nigdy dowolne `to`, `from`, temat lub treść przesłane przez publiczny endpoint wysyłki. Zmiana emaila konta wymaga ponownej weryfikacji, zgodnie z planem auth.
- Adresy parsuje biblioteka; jeden odbiorca na wiadomość, bez CC/BCC. Odrzuć CR/LF i nadmierne długości; bez własnego regexa email. Zachowaj adres do wysyłki oddzielnie od klucza normalizacji używanego przez auth.

## E1 — Mały klient SMTP

`Mailer::send` przyjmuje poprawny adres i konkretną wiadomość, zwraca `Result`. Jeden współdzielony transport, maks. 2 wysyłki jednocześnie, limit 20 s obejmujący oczekiwanie i SMTP, treść do 32 KiB. Osobne błędy konfiguracji/adresu, przejściowego dostawcy, trwałego odrzucenia i niepewnego wyniku po timeout. Nie loguj hasła, treści, kodu ani pełnego adresu.

Lokalna komenda testowa wysyła stałą, niesekretną wiadomość tylko do `MAIL_TEST_TO`; nie dodawaj publicznego endpointu testowania. SMTP zaakceptowane oznacza przyjęcie przez dostawcę, nie potwierdzone doręczenie.

Gotowe gdy: build + krótki test błędnego adresu/trybu testowego oraz jedna jawnie zlecona wysyłka do własnej skrzynki, potwierdzona odbiorem. Samo logowanie SMTP nie zamyka etapu.

## E2 — Kod logowania

Podepnij SMTP do [auth](auth.md): najpierw zapis wyzwania, potem ograniczona wysyłka. Błąd lub niepewny wynik unieważnia próbę, API zwraca ogólny błąd. Bez automatycznego ponawiania starego kodu i bez kodów w outboxie/logach. Tryb `disabled` nie udaje udanej wysyłki. Limity prób i generowanie/weryfikacja kodu należą do auth, nie do klienta SMTP.

Gotowe gdy: kod z odebranej wiadomości działa raz, a awaria SMTP nie pozostawia używalnej próby. Zależność: implementacja wyzwań i limitów z planu auth.

## E3 — Trwałe powiadomienia

Wdrażaj wraz z trwałymi zamówieniami/zdarzeniami. Jedna tabela `email_outbox`: `id`, unikalny `event_key`, `user_id` opcjonalne dla gościa, `kind`, `recipient`, ograniczony `payload`, `status`, `attempts`, `available_at`, `locked_until`, `created_at`, `sent_at`, `last_error_code`. `payload` jest walidowanym snapshotem danych konkretnej wiadomości; bez tokenów, kodów i danych karty. Indeks kolejki po statusie/czasie; bez tabeli dla każdego rodzaju maila.

Zapis zdarzenia biznesowego i outboxa w jednej transakcji PostgreSQL. `event_key` rozróżnia zdarzenie, odbiorcę i typ — powtórzony webhook nie tworzy następnego maila. Potwierdzenie płatności dopiero po zweryfikowanym, trwałym zdarzeniu Stripe; nie po powrocie przeglądarki.

Jeden worker w obecnym procesie: krótka transakcja pobiera rekordy przez `FOR UPDATE SKIP LOCKED`, ustawia lease 60 s, kończy transakcję; SMTP poza blokadą. Po wysyłce zapisuje wynik. Wygasły lease pozwala wznowić po restarcie. Aktualizacja wyniku wymaga tej samej wersji próby/lease, aby spóźniony worker nie nadpisał nowej próby.

Maks. 5 prób; opóźnienia 1/5/30/120 min z małym jitterem, tylko przy błędach przejściowych lub niepewnym wyniku. Trwałe odrzucenie kończy rekord jako `failed`; błąd konfiguracji/autoryzacji zatrzymuje wysyłkę do naprawy. Stały `Message-ID` na rekord. Awaria po przyjęciu SMTP, przed zapisem wyniku może dać duplikat — SMTP nie zapewnia exactly-once.

Worker ma ograniczoną partię i tempo (na start 10 wiadomości/min, do dostrojenia), przyrost kolejki podlega limitom zdarzeń biznesowych. Zaległości i trwałe błędy są widoczne operatorowi. Zatrzymanie aplikacji kończy bieżącą próbę lub pozostawia lease do odzyskania. Dane zakończonych rekordów usuwaj po 30 dniach małymi partiami; nie kasuj po cichu oczekujących.

Gotowe gdy: restart odzyskuje zadanie, ponowiony event nie tworzy duplikatu rekordu, przejściowy błąd jest ponawiany, trwały kończy się `failed`. Kilka testów PostgreSQL i transport lokalny; bez wielokrotnych wysyłek przez iCloud.

## E4 — Preferencje i powiadomienia klienta

Dwa pola przy koncie: `email_order_updates` i `email_chat_messages`, domyślnie włączone, edytowane wyłącznie przez właściciela. Sprawdzaj preferencję przy tworzeniu i przed wysłaniem opcjonalnego maila, aby wyłączenie objęło także kolejkę.

| Wiadomość | Odbiorca | Można wyłączyć |
| --- | --- | --- |
| Kod logowania / bezpieczeństwo konta | Weryfikowany adres / właściciel konta | Nie |
| Potwierdzenie zamówienia / płatności | Kontakt utrwalony przy zamówieniu | Nie |
| Zmiana statusu realizacji | Właściciel zamówienia | Tak |
| Nowa wiadomość w rozmowie | Drugi uprawniony uczestnik | Tak, klient |

Mail o rozmowie zawiera krótkie powiadomienie i link do panelu, bez treści prywatnego chatu. Link wymaga sesji oraz sprawdzenia własności, nie działa jako token dostępu. Admin odpowiada w panelu; `Reply-To` prowadzi na obsługiwaną skrzynkę, odpowiedź email nie trafia automatycznie do rozmowy. Dla gościa utrwal kontakt i preferencję statusów przy zamówieniu, bez tworzenia konta; dostęp do szczegółów zależy od mechanizmu własności zamówienia.

Gotowe gdy: klient może wyłączyć opcjonalne wiadomości, nadal otrzymuje kod/potwierdzenie, obcy użytkownik nie zmienia preferencji ani nie otwiera cudzej rozmowy.

## Warunki i późniejszy zakres

- E1–E4 działają w backendzie. Widoki kont/chat/preferences nadal są mockami; pełny projekt admin auth/passkey, własność plików przez konto i produkcyjne webhooki wymagają osobnego wdrożenia. API nie wysyła wiadomości z frontendowych mocków.
- Przed wysyłką do klientów sprawdzić uprawnienie `From`, odbiór na zewnętrznej skrzynce, SPF/DKIM/DMARC domeny oraz aktualne limity iCloud. Duży ruch może wymagać dostawcy transakcyjnego; nie budujemy teraz abstrakcji na zapas.
- IMAP i odpowiedzi email → chat później: korelacja wątków, uwierzytelnienie nadawcy, deduplikacja, limity MIME i ochrona przed pętlami. Nagłówek `From` sam nie wystarcza do uprawnień.
- Przy implementacji: fmt, Clippy i testy zmienionych crate’ów, bez pełnego E2E i testerów-subagentów. Zmierz Rust LOC przed/po, osobno produkcja/testy, jeśli narzędzie pozwala; nie deklaruj PASS bez wykonania.

Źródła integracji: [iCloud — konfiguracja SMTP](https://support.apple.com/en-us/102525), [lettre — transport i features](https://docs.rs/lettre/latest/lettre/). Limity workera i retencja powyżej są decyzjami projektu, do dostrojenia.
