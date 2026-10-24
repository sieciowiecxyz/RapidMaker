# Rozmowa zamówienia: strona i email

Status: CH0–CH4 PASS lokalnie, 2026-10-24. SMTP/IMAP i historia są zaimplementowane. Bez frontendu, WebSocketów, załączników i nowego brokera.

## Cel i punkt wyjścia

Jedna historia przy zamówieniu w PostgreSQL. Klient i uprawniony operator odpowiadają przez API albo email; druga strona widzi ten sam tekst. Email kopiuje nową wiadomość, nie całą historię. Wyłączenie powiadomień nie usuwa rozmowy.

Działają `GET/POST /api/account/orders/{id}/messages`, odpowiedniki `/api/admin/orders/{id}/messages`, własność, sesje/CSRF, zapis wiadomości z outboxem w jednej transakcji, preferencje i SMTP iCloud. Wdrożone: odbiór IMAP, podpisane referencje, treść w kopiach email, link klient/admin do konkretnego zamówienia, paginacja i przeczytanie oraz grupowanie przez 60 s. Dawny limit 200 wiadomości został usunięty.

## Przepływ i granice

`API lub IMAP → ta sama autoryzowana operacja zapisu → order_messages + email_outbox → SMTP do drugiej strony`.

- `storage/orders.rs`: wspólne reguły uczestnictwa i transakcja. Kanał `web/email`, autor, kolejność wiadomości; email nie omija aktualnej własności, blokad konta ani limitów. Rola operatora pochodzi z bazy, nie z nagłówka poczty.
- `notifications`: treść i nagłówki SMTP oraz ograniczony parser MIME/transport IMAP. Użyj utrzymywanych bibliotek Rust; features ograniczone do potrzebnego transportu/parsing MIME. Nie pisz własnego parsera ani protokołu.
- `api`: obecne endpointy i jeden worker odbioru, start/stop razem z aplikacją. Poll co 30 s, ograniczone partie, timeout/retry; jeden odbiorca skrzynki przez lease PostgreSQL. Bez HTTP i IMAP pod blokadą/transakcją DB.
- Rozszerz obecne tabele rozmów/outboxa. Dodaj tylko trwałe powiązania odpowiedzi oraz rejestr odebranych maili z retry/deduplikacją. Metadane wyszukiwania i unikalności jako kolumny. Nie przechowuj całej skrzynki ani załączników w DB.

## Bezpieczne powiązanie odpowiedzi

Na start stałe `Reply-To: void@sieciowiec.xyz`; nie zakładaj obsługi dowolnych aliasów ani catch-all w iCloud. Każda kopia wychodząca ma osobny, trwały `Message-ID` z nieprzewidywalnym podpisanym identyfikatorem odpowiedzi, związanym z odbiorcą i zamówieniem. Ponowienie SMTP zachowuje ten identyfikator. Odbiór rozpoznaje go w `In-Reply-To`/`References`.

Zastosuj istniejące HMAC/CSPRNG i osobny sekret `MAIL_REPLY_KEY` w SOPS; bez jawnych sekretów w logach. Sprawdź podpis, zapisane powiązanie, aktualne uprawnienia i zgodność nadawcy ze zweryfikowanym adresem uczestnika. Sam `From`, temat lub numer zamówienia nie uprawniają do zapisu. Nie mapuj nieznanych maili automatycznie po adresie email klienta.

Identyfikator odpowiedzi jest uprawnieniem do dopisania tekstu do jednej rozmowy. Nie daje sesji, dostępu do plików, zmiany statusów/płatności ani uprawnień administracyjnych. Kopie klienta i operatora mają osobne powiązania; nie wysyłaj obu do jednego `To/Cc`. Odpowiedź operatora wymaga aktualnej, jawnie nadanej możliwości odpowiadania emailem; nie traktuj jej jak sesji admina po MFA. Wygaśnięcie/odwołanie powiązania i utrata dostępu blokują import.

To model zaufania do posiadania prywatnej kopii emaila: przekazanie jej dalej może ujawnić identyfikator. Nie deklaruj równoważności z logowaniem/passkey. Nieznane, niejednoznaczne lub nieautoryzowane odpowiedzi trafiają do stanu odrzucone/do weryfikacji, bez automatycznej odpowiedzi na dowolny adres.

## Odbiór, limity i niezawodność

- iCloud IMAP przez TLS, `MAIL_INBOUND_MODE=disabled/test/live` (domyślnie disabled). Lokalne `.env`: test i folder `MAIL_IMAP_MAILBOX=void@sieciowiec.xyz`; bez ustawienia folderu domyślny INBOX. Odbiór read-only przez EXAMINE/BODY.PEEK; do 20 UID na partię, timeout 30 s i budżet odpowiedzi 8 MiB. Testowo wyłącznie `void@sieciowiec.xyz`.
- Odbieraj tylko odpowiedzi powiązane z naszymi wiadomościami. Pozostałych maili nie importuj, nie kasuj i nie oznaczaj jako przeczytane. Przy pierwszym starcie ustaw jawny punkt rozpoczęcia; nie importuj całej starej skrzynki.
- Deduplikacja po skrzynce, `UIDVALIDITY` i UID; dodatkowo rozpoznaj powtórną dostawę tej samej wiadomości. `Message-ID` nie jest dowodem uprawnień. Dopisanie do rozmowy, zapis odbioru i outbox w jednej transakcji. Awaria/restart nie gubi wiadomości ani nie dopisuje jej ponownie.
- Limit surowego maila 256 KiB, nowej treści 4000 bajtów zgodnie z obecnym API, timeout i ograniczona głębokość MIME. Preferuj tekst; HTML zamieniaj na bezpieczny tekst. Usuń cytaty i podpis tylko przy jednoznacznym rozpoznaniu. Przy pustej/za dużej/niejednoznacznej treści jawny stan do weryfikacji, bez cichego obcięcia.
- Ignoruj autorespondery, raporty niedostarczenia, listy i własne kopie. Nie twórz pętli email → email. Załączników nie publikuj ani nie przetwarzaj.
- Zmiana statusu czy płatności pozostaje osobnym API. Logi: ID, stan i kategoria błędu, bez tekstu, adresów, tokenów i haseł. Surowy MIME nie trafia do trwałych logów; odrzucone wiadomości zachowują tylko ograniczone metadane/przyczynę.

## Historia i powiadomienia

Stabilna kolejność `sequence` i paginacja: `GET .../messages?after=0&limit=50`, limit maks. 100. Odpowiedź zawiera `messages,next_after,read_sequence,unread`. `POST .../messages/read {"sequence":N}` działa dla klienta i admina. Limit 4000 bajtów wiadomości i 10 wiadomości/min na zamówienie pozostaje. Historia zostaje razem z zamówieniem; metadane odbioru 30 dni, referencje odpowiedzi wygasają po 30 dniach. Odczyt nie oznacza automatycznie przeczytania: osobny idempotentny zapis kursora uczestnika, aktualizowany tylko w przód.

Zgrupuj wiadomości dla jednego odbiorcy/zamówienia przez 60 s od pierwszej niepowiadomionej wiadomości, bez ciągłego przesuwania okna. Outbox zawiera stały zakres ID wiadomości i `Message-ID`; retry wysyła ten sam zakres. Własne wiadomości nie generują kopii do autora. Odczyt nie wycofuje już zakolejkowanej kopii; aktualne preferencje i dostęp są ponownie sprawdzane przed SMTP. Wyłączenie powiadomień nie blokuje zapisu odpowiedzi. SMTP może dostarczyć kopię ponownie po niepewnym wyniku; nie obiecuj exactly-once dostarczenia.

## Etapy — PASS lokalnie

| Etap | Implementacja | Krótkie potwierdzenie |
|---|---|---|
| CH0 | Test dostępu IMAP i round-trip nagłówków na iCloud; wybór parsera/klienta Rust, weryfikacja modelu odpowiedzi | Wiadomość i zwykłe „Odpowiedz” zachowują referencję; brak dostępu = NOT RUN, bez importu starej poczty |
| CH1 | Wspólna operacja zapisu, pochodzenie wiadomości, paginacja i kursory przeczytania | API/odbiór używają tych samych uprawnień; wiadomość 201+ dostępna; obcy autor odrzucony |
| CH2 | Kopia treści, właściwe linki klient/admin, trwałe referencje i grupowanie przez istniejący outbox | Kilka wiadomości = jedna kopia zakresu; poprawny odbiorca; retry zachowuje ID; brak kopii do autora |
| CH3 | Worker IMAP, deduplikacja, MIME, bezpieczne powiązanie i restart | Odpowiedź → dokładnie jeden wpis rozmowy; powtórny odbiór/restart bez duplikatów; zły token/nadawca, autoresponder i załącznik nie dopisują tekstu |
| CH4 | Jeden pełny przebieg backendowy i aktualne OpenAPI/docs | API klienta → email operatora → odpowiedź → historia klienta; również odwrotnie. Preferencje, uprawnienia i retry sprawdzone |

Zakres testów proporcjonalny: regresje powiązania/uprawnień i MIME, jeden test transakcji PostgreSQL i rzeczywisty round-trip przez testową skrzynkę. Bez Astry, testujących subagentów i frontendu. Nie wysyłaj do klientów produkcyjnych. Test końcowy może wymagać dwóch rozróżnialnych tożsamości pocztowych pod kontrolą właściciela; wspólne `void` nie dowodzi izolacji klient/admin.

Warunki odbioru: dostęp IMAP/app-password, zachowanie nagłówków przez wybranych klientów pocztowych, odrębne tożsamości do testu dwóch stron. Catch-all ani nowy dostawca nie są wymaganiem podstawowego wariantu. Bez obsługiwanej referencji wiadomość zostaje do ręcznej weryfikacji; bezpieczny fallback to odpowiedź w panelu.

Konfiguracja IMAP iCloud: [Apple](https://support.apple.com/en-in/102525). Opis wspiera możliwość transportu; dostęp konkretnego konta i round-trip wymagają własnego testu CH0.

## Konfiguracja i diagnostyka

- `MAIL_REPLY_KEY`: osobny losowy sekret min. 32 bajty, `.env`/SOPS; zmiana klucza unieważnia stare referencje.
- IMAP korzysta z `MAIL_USER`/`MAIL_PASSWORD`; opcjonalnie oddzielne `MAIL_IMAP_USER`/`MAIL_IMAP_PASSWORD`. Serwer iCloud `imap.mail.me.com:993`, TLS; `MAIL_IMAP_MAILBOX` wskazuje folder docelowy.
- `python backend/run.py --mail-receive`: jedna partia tą samą ścieżką co worker; wypisuje liczniki imported/duplicate/rejected/review/ignored. Zajęty lease może dać zera. Pierwsze uruchomienie ustala punkt rozpoczęcia, bez importu starej poczty.
- `python backend/run.py --mail-operator EMAIL on` lub `off`: lokalne uprawnienie odpowiadania emailem dla istniejącego admina. `off` odwołuje dotychczasowe referencje operatora. Samo otrzymanie alertu nie nadaje tego uprawnienia.
- Formularze logowania/frontendu nadal wymagają podłączenia. API admina zachowuje sesję/passkey/CSRF; kanał email daje tylko wąskie uprawnienie dopisania wiadomości.

Dowód CH4: osobna baza; wiadomości przez API gościa/admina oraz SMTP → rzeczywisty folder iCloud → Rust IMAP → wspólna historia w obu kierunkach. Grupowanie, treść/link, restart/replay UID, paginacja/read/CSRF i odwołanie referencji PASS. Dwie role używały jednego fizycznego `void`; izolację różnych kont, rollback, 205 wpisów i preferencje sprawdzono osobno w PostgreSQL. Test używał sesji admina z fixture, bez ponownego testu fizycznego passkey. W izolowanej bazie przyspieszono `available_at` outboxa oraz cofnięto kursor UID/zwolniono lease, aby wymusić ponowny odbiór tego samego maila. Reguły produktu pozostały bez zmian. Testy nie obejmują dostarczalności produkcyjnej ani wszystkich klientów pocztowych.
