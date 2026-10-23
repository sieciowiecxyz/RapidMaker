# Auth — projekt

Status: propozycja do implementacji. Obecnie klient jest mockiem, operator ma osobną sesję RAM i `ADMIN_API_TOKEN`. Ten dokument nie oznacza działającego nowego logowania.

## Zachowanie

- Jeden `/logowanie`, pole email → 8-cyfrowy kod → zalogowanie. Bez hasła, linków logujących i osobnej rejestracji: pierwsze poprawne potwierdzenie tworzy konto `client`.
- Kod ważny 10 min, jednorazowy, maks. 5 prób. Powiązany z żądaniem i cookie przeglądarki, która rozpoczęła logowanie. Ponowne wysłanie po 60 s unieważnia poprzedni kod tej próby. Logowanie na innym urządzeniu wymaga własnej próby.
- Ta sama odpowiedź dla nowego/istniejącego adresu. Roli nie wybiera użytkownik; backend odczytuje ją z DB. Normalizacja emaila: trim i świadoma polityka case-insensitive; nie usuwamy kropek ani `+suffix`.
- Admin po kodzie musi użyć passkey (WebAuthn, wymagane user verification: PIN/biometria urządzenia). Do tego czasu sesja nie daje dostępu do żadnego endpointu admina. UI pokazuje następny krok; ukrycie przycisku nie jest kontrolą dostępu.
- Konto klienta nie jest wymagane do wyceny/zakupu. Email odbiorcy, dane Stripe i status płatności nie stanowią potwierdzenia własności konta.

## Sesja

Losowy sekret 32 bajty z CSPRNG; do DB trafia SHA-256, do przeglądarki wyłącznie cookie. Bez JWT, refresh tokenów i sekretów w localStorage. Każde żądanie sprawdza sesję, aktywność użytkownika i aktualną rolę.

Produkcja: `__Host-rapidmaker_session; Secure; HttpOnly; SameSite=Lax; Path=/`, bez Domain. Lax pozwala wrócić z hosted Stripe do własnego zamówienia. Jawny tryb localhost ma inną nazwę cookie, bez Secure; nie może być automatycznym fallbackiem wdrożenia.

- Klient: maks. 30 dni od logowania, wygaśnięcie po 7 dniach bezczynności. `last_seen_at` aktualizowane najwyżej raz na 5 min, nigdy nie przedłuża limitu absolutnego.
- Admin: po passkey nowy sekret sesji; maks. 8 h, 30 min bezczynności. Wrażliwe akcje (np. zmiana passkey, przyszły zwrot płatności) wymagają potwierdzenia passkey z ostatnich 5 min.
- Nowy sekret po logowaniu i podniesieniu uprawnień; poprzednia sesja unieważniana atomowo. Wylogowanie usuwa sesję w DB oraz cookie; dostępne też „wyloguj wszystkie urządzenia”.
- Zablokowanie konta, zmiana roli lub procedura odzyskania dostępu unieważniają jego sesje. Restart serwera nie wylogowuje użytkowników.

## Minimalne tabele PostgreSQL

| Tabela | Najważniejsze pola i ograniczenia |
| --- | --- |
| `users` | `id uuid`, `email text UNIQUE` (znormalizowany), `role` CHECK client/admin, `disabled_at`, `created_at`. Istnienie rekordu oznacza zweryfikowany email. |
| `auth_challenges` | `id uuid`, `email`, `purpose` (login/passkey_register/passkey_login), `user_id` nullable, `browser_hash`, `verifier`, `attempts`, `expires_at`, `consumed_at`, `created_at`. Dla WebAuthn ograniczony payload biblioteki, związany z użytkownikiem/sesją i jedną operacją. |
| `sessions` | `token_hash bytea PRIMARY KEY`, `user_id` FK, `created_at`, `last_seen_at`, `expires_at`, `admin_verified_at` nullable. Indeksy user/expiry. |
| `passkeys` | `credential_id bytea PRIMARY KEY`, `user_id` FK, zweryfikowany credential biblioteki, nazwa i czas utworzenia. Limit 5/użytkownika; klucz prywatny pozostaje na urządzeniu. |
| `auth_limits` | hash zakresu email/IP/global jako PK, początek okna, licznik. Atomowe inkrementacje w PostgreSQL; limity działają także między procesami i po restarcie. |

Zużycie kodu, policzenie błędnej próby, utworzenie użytkownika i sesji są transakcyjne; równoległe użycie kodu daje jeden sukces. Osobne, losowe identyfikatory nie zastępują sprawdzania właściciela.

Krótki kod ma mało entropii: zapisujemy HMAC-SHA-256 nad `challenge_id + email + browser_hash + code`, z jednoznacznym kodowaniem pól i osobnym `AUTH_CODE_KEY` w `.env`/SOPS. Porównanie stałoczasowe. Nie zapisujemy samego SHA-256 kodu; sesje mają wystarczająco losowe sekrety do zwykłego SHA-256. WebAuthn implementuje sprawdzona biblioteka, nie własna kryptografia; waliduje challenge, origin, RP ID i user verification.

## API

| Metoda i ścieżka | Efekt |
| --- | --- |
| `POST /api/auth/start {email}` | Tworzy próbę i cookie wiążące przeglądarkę; wysyła kod. `202 {challenge_id}` niezależnie od istnienia konta. Przekroczone limity → 429. |
| `POST /api/auth/verify {challenge_id,code}` | Zużywa kod, tworzy konto w razie potrzeby i nową sesję. Zwraca użytkownika oraz `admin_verified:false`; admin przechodzi do passkey. |
| `GET /api/auth/me` | Bieżący użytkownik/rola, stan potwierdzenia admina i CSRF token albo 401. Bez sekretu sesji. |
| `POST /api/auth/logout` | Unieważnia bieżącą sesję, czyści cookie. |
| `POST /api/auth/logout-all` | Unieważnia wszystkie sesje własnego konta. |
| `POST /api/auth/passkey/{register,login}/{start,finish}` | Wyzwanie i wynik WebAuthn. Zapis wymaga świeżo potwierdzonego admina albo lokalnie przyznanego, jednorazowego bootstrapu. Udane login/finish rotuje sesję i nadaje ograniczony czas dostępu admina. |
| `POST /api/auth/claim-guest` | Po zalogowaniu przypisuje tylko zasoby dostępne również przez aktualną sesję gościa. |

Mutacje są JSON POST/PUT/DELETE, nigdy GET. Same-origin frontend/API. Kontrola dokładnego Origin z konfiguracji serwera oraz `X-CSRF-Token` związany z sesją (np. HMAC z osobną domeną klucza nad hashem sesji); start/verify wymagają Origin/JSON i cookie próby. Bez CORS wildcard. Webhook Stripe ma osobną weryfikację podpisu, nie CSRF użytkownika.

## Własność i uprawnienia

- Publiczne: katalog, rozpoczęcie logowania. Gość: własne modele, wyceny i checkout przez dotychczasowe cookie gościa. Zalogowanie nie kasuje tego cookie automatycznie.
- Klient: własny profil, pliki, wyceny i zamówienia. Zapytanie SQL ogranicza wynik przez `user_id`; cudzy obiekt → 404. Nie pobieramy dowolnego obiektu, żeby filtrować go tylko w UI.
- Przypisanie gościa wymaga jednocześnie zweryfikowanej sesji konta i cookie właściciela zasobu, jest atomowe i nie nadpisuje innego właściciela. Dodaj nullable `user_id` do modeli, snapshotów i przyszłych zamówień. Rozliczone zasoby zachowują własny okres retencji, niezależnie od roboczego uploadu 24 h.
- Nie przejmujemy automatycznie wszystkich zamówień o pasującym emailu ani nie łączymy kont po nazwisku/telefonie. Dostęp do dawnego zakupu bez cookie wymaga osobnej procedury odzyskania, poza pierwszą implementacją.
- Admin: aktualna rola DB + aktywne potwierdzenie passkey. Dostęp do katalogu/cennika i realizacji zamówień; pobieranie plików tylko w kontekście obsługi konkretnego zamówienia. Żadne pole formularza nie nadaje admina.

## Ochrona i email

Początkowe limity do dostrojenia: 3 wysyłki/15 min na email, 20/15 min na IP, globalnie 100/15 min; 5 prób/kod i 20 błędnych prób/15 min na email. Brak trwałego blokowania konta przez obcą osobę. Retencja wygasłych wyzwań/limitów 24 h, maks. 10 tys. rekordów prób; admission przed generowaniem i wysyłką. Indeksy, cleanup partiami, ograniczona równoległość SMTP.

Adres źródłowy tylko z połączenia lub jawnie zaufanego reverse proxy; nie ufamy dowolnemu `X-Forwarded-For`. W obecnym proxy Vite wszystkie połączenia mogą wyglądać jak localhost — to tryb developerski, nie gotowa konfiguracja limitów produkcji.

Kod wysyła `notifications` przez iCloud SMTP z hasłem aplikacji (na start `void@sieciowiec.xyz`). IMAP nie jest potrzebny do logowania. Pierwsza wersja wysyła z timeoutem, po zapisie wyzwania; błąd dostawcy unieważnia próbę i zwraca ogólny błąd bez ujawniania istnienia konta. Bez pozornego sukcesu i bez kodów w logach. Trwały outbox/retry można dodać wraz z powiadomieniami zamówień.

Nie logujemy kodów, cookie, tokenów ani pełnych nagłówków. Zdarzenia: start/weryfikacja/odmowa/wylogowanie, identyfikator żądania, użytkownika i powód techniczny bez sekretów. Zmiana emaila i usuwanie konta nie wchodzą do pierwszej wersji; wymagają osobnego potwierdzenia i zasad retencji.

## Crate’y i wdrożenie

- `auth`: generowanie/sprawdzanie wyzwań, reguły sesji i uprawnień; sprawdzone biblioteki CSPRNG/HMAC/WebAuthn. Bez HTTP i SQL.
- `storage`: jawny SQL, migracje, transakcje, limity i cleanup.
- `api/auth.rs`: endpointy, cookie/CSRF, ekstraktor bieżącego użytkownika; osobna kontrola admina. `api` orkiestruje `auth`, `storage`, `notifications`.
- `notifications`: minimalny klient SMTP; konfiguracja wyłącznie backendowa.
- Frontend: jeden adapter auth zamiast klienta mock + operatora; `/me` jest źródłem roli. Widok przechowuje tylko stan formularza.

Kolejność: (1) sesje/kody/SMTP i klient, (2) własność gość/konto, (3) passkey i migracja admina. Przed publicznym panelem wymagane całe trzy kroki. Pierwszy admin jest nadawany lokalnym poleceniem operatora dla zweryfikowanego konta: jednorazowe zezwolenie na rejestrację passkey, 10 min, związane z kontem i sesją; rola/dostęp aktywowane dopiero po zapisaniu klucza. Odzyskanie po utracie wszystkich passkey: lokalna procedura operatora z unieważnieniem sesji, bez emailowego obejścia MFA. Warto zarejestrować drugi passkey jako zapas.

Stare `ADMIN_API_TOKEN` i cookie operatora usuwamy po uruchomieniu nowej kontroli wszystkich tras admina; do tego są wyłącznie lokalnym mechanizmem przejściowym. Publiczny start nie może zostawić bocznej ścieżki Bearer omijającej passkey.

Gotowe gdy: kod błędny/wygasły/powtórzony i dwa równoległe użycia są obsłużone; restart zachowuje sesję; logout/blokada ją unieważniają; cudzy model/zamówienie jest niedostępne; klient ani admin bez passkey nie zapisze katalogu; CSRF i próby ponad limit są odrzucone. Jeden krótki przebieg przeglądarki klient/admin. Nie wymaga pełnego pentestu przy każdej zmianie.

Źródła: [OWASP — sesje](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html), [OWASP — MFA](https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html). Czasy i limity powyżej są decyzją projektu, nie wartościami narzuconymi przez OWASP.
