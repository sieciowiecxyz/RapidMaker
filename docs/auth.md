# Auth — Rust API

Stan 2026-10-23: backend klienta i admina działa. Frontend logowania/paneli wymaga podłączenia. Stary Bearer `ADMIN_API_TOKEN` i sesja operatora nie dają już dostępu.

## Logowanie i uprawnienia

- Email → 8-cyfrowy kod, 10 min, 5 prób, związany z przeglądarką. Pierwsze potwierdzenie tworzy klienta. Ta sama odpowiedź dla istniejącego/nowego adresu; SMTP failure unieważnia próbę. Adresy ASCII: trim + lowercase, bez usuwania kropek/`+suffix`.
- Klient dostaje sesję. Admin po kodzie dostaje tylko sesję oczekującą, maks. 10 min; dostęp administracyjny wymaga passkey z PIN/biometrią (WebAuthn UV Required).
- Role i aktywność pochodzą z PostgreSQL, nigdy z formularza/localStorage. Każdy endpoint admina sprawdza aktualną rolę i MFA; zapis dodatkowo Origin i CSRF.
- Modele, wyceny i zamówienia sprawdzają własność. `claim-guest` wymaga jednocześnie sesji konta i oryginalnego cookie gościa, przenosi tylko własne aktywne/nieprzypisane zasoby w jednej transakcji. Bez przejmowania zakupów po samym emailu.
- Po claim istniejące oferty checkoutu w RAM mogą wymagać ponownego przeliczenia. Robocze modele nadal wygasają po 24 h; nowe zamówienia mają osobną ochronę plików przed płatnością, przez realizację i 90 dni po zakończeniu. Dostęp przez endpointy plików zamówienia wymaga jego autoryzacji.

## Sesje i kryptografia

Losowe tokeny 32 B z CSPRNG, w DB SHA-256, w przeglądarce wyłącznie HttpOnly cookie. Kod: HMAC-SHA-256 z `AUTH_CODE_KEY`, związany z wyzwaniem, adresem i przeglądarką. CSRF ma osobną domenę HMAC. Bez JWT i własnej implementacji WebAuthn.

- Klient: 30 dni absolutnie, 7 dni bezczynności. Admin po MFA: nowy token, 8 h absolutnie, 30 min bezczynności. Aktualizacja last_seen najwyżej raz/5 min.
- Zmiana roli/blokada unieważnia sesje w DB; logout-all usuwa wszystkie własne sesje. Restart zachowuje logowanie.
- Dodawanie/usuwanie passkey wymaga MFA z ostatnich 5 min albo lokalnego bootstrapu. Maks. 5 kluczy; ostatniego nie można usunąć. Ponowne potwierdzenie passkey odnawia świeżość i rotuje sesję.
- Wyzwania WebAuthn: PostgreSQL, związane z sesją/użytkownikiem/celem, 5 min, jednorazowe, limity startów. Stan biblioteki nigdy nie trafia do klienta. Sprawdzane podpis, RP ID, dokładny Origin, UV oraz licznik; aktualizacje credential są trwałe.
- Kody: 3 wysyłki/15 min/email, 20/IP, 100 globalnie; 20 błędnych weryfikacji/15 min/email i 5/kod. Stan auth ograniczony/sprzątany partiami. Brak sekretów, kodów i pełnych adresów w logach.

## Konfiguracja

| Klucz | Znaczenie |
| --- | --- |
| `AUTH_MODE` | `disabled` domyślnie, `localhost` albo `production`. Brak automatycznego fallbacku do HTTP. |
| `AUTH_ORIGIN` | Local: domyślnie `http://localhost:18430`. Production: wymagany dokładny origin HTTPS z nazwą domenową, bez ścieżki/loginu/query. |
| `AUTH_CODE_KEY` | 32 losowe bajty hex, wyłącznie `.env`/SOPS. |
| `AUTH_TRUSTED_PROXY` | Opcjonalnie jeden zaufany adres IP proxy. Tylko od niego przyjmowany pojedynczy IP w X-Forwarded-For; łańcuch jest odrzucany. Bez konfiguracji używamy adresu połączenia. |

RP ID to hostname Origin. WebAuthn lokalnie wymaga **localhost**, nie `127.0.0.1`. Klucze localhost i domeny produkcyjnej są oddzielne. Production wymaga TLS na reverse proxy i zachowania publicznego Host; backend nadal słucha na loopback. Host i Origin są kontrolowane, bez CORS wildcard.

Production: `__Host-rapidmaker_session`/`__Host-rapidmaker_auth_attempt`, Secure/HttpOnly/SameSite=Lax/Path=/, bez Domain. Cookie gościa również ma __Host/Secure. Local ma osobne nazwy bez Secure. Nie traktować testów konfiguracji jako wdrożonego publicznego HTTPS.

## API

| Metoda / ścieżka | Działanie |
| --- | --- |
| POST `/api/auth/start` `{email}` | Wysyła kod, ustawia cookie próby; 202 `{challenge_id}`. |
| POST `/api/auth/verify` `{challenge_id,code}` | Zużywa kod, ustawia sesję; user, csrf_token, admin_verified. |
| GET `/api/auth/me` | Aktualne konto, CSRF, termin i stan MFA albo 401. |
| POST `/api/auth/logout`, `/logout-all` `{}` | Unieważnia sesję/sesje i czyści cookie. |
| POST `/api/auth/passkey/register/start` `{bootstrap?}` | Opcje rejestracji + challenge_id; bootstrap tylko dla lokalnego zezwolenia. |
| POST `/api/auth/passkey/register/finish` `{challenge_id,credential,name}` | Sprawdza odpowiedź WebAuthn i rotuje sesję. |
| POST `/api/auth/passkey/login/start` `{}` | Opcje potwierdzenia passkey. |
| POST `/api/auth/passkey/login/finish` `{challenge_id,credential}` | Weryfikuje podpis/UV, rotuje sesję, nadaje MFA. |
| GET `/api/auth/passkeys`, DELETE `/{id}` | Zarządzanie własnymi kluczami przy świeżym MFA. |
| POST `/api/auth/claim-guest` `{}` | Atomowe przypisanie modeli/wycen/zamówień; resources_claimed. |

Mutacje wymagają dokładnego Origin i X-CSRF-Token z `/me`; start/verify zamiast CSRF korzystają z cookie próby. Payloady są JSON. Opcje WebAuthn trzeba przekazać do navigator.credentials z konwersją base64url ↔ ArrayBuffer; credential to standardowa odpowiedź przeglądarki. Pusty opcjonalny credProps z Chromium oznacza brak deklaracji, nie rk=false; podpisane dane/UV nadal weryfikuje biblioteka.

## Pierwszy admin i odzyskanie

1. Potwierdź email w przeglądarce (klient, świeża sesja do 10 min).
2. Lokalnie: `python backend/run.py --auth-bootstrap ADRES_EMAIL`. Zwrócony sekret jest jednorazowy, ważny 10 min i związany z tą sesją; nie zapisuj go w logach/chacie.
3. W tej samej przeglądarce rozpocznij register/start z sekretem i zakończ rejestrację przez WebAuthn. Dopiero podpisana odpowiedź nadaje admina i rotuje sesję. Zarejestruj drugi klucz jako zapas.

Utrata kluczy: najpierw kod email tworzący świeżą sesję oczekującą, następnie lokalne `--auth-recover ADRES_EMAIL`. Polecenie unieważnia stare klucze i pozostałe sesje, wydaje nowy bootstrap. Sam email nigdy nie zastępuje MFA. Polecenia nie są endpointami HTTP.

## Moduły i dowody

`auth`: CSPRNG/HMAC i webauthn-rs; `storage/auth.rs`, `passkeys.rs`, `models.rs`: SQL, transakcje i własność; `api/auth.rs`, `passkeys.rs`, `admin.rs`: HTTP/CSRF/autoryzacja. Jedyna nowa biblioteka bezpieczeństwa to webauthn-rs; serializacja stanu włączona wyłącznie na potrzeby zaufanego PostgreSQL.

Sprawdzone: testy Rust, izolowany PostgreSQL oraz Chromium z wirtualnym authenticatorem: bootstrap/rejestracja/login, odmowa podpisanej odpowiedzi bez UV, replay, rotacja, brak dostępu klienta/admina przed MFA/starego Bearer, CSRF, cookie produkcyjne i Origin/Host/proxy. Fizyczny klucz i publiczny TLS nie były testowane. Źródła: [webauthn-rs](https://docs.rs/webauthn-rs/0.5.3/webauthn_rs/struct.Webauthn.html), [WebAuthn](https://w3c.github.io/webauthn/), [sesje OWASP](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html).

Walidacja: fmt, Clippy i build PASS; 18 testów Rust + 6 testów izolowanego PostgreSQL PASS. Przyrost: 9290 → 10439 (+1149) linii kodu Rust w backend/crates, razem z testami inline, bez komentarzy/pustych/generowanych plików. Audit z lokalnej bazy: 0 nowych problemów w zależnościach auth; 9 istniejących wpisów w pozostałym grafie zależności pozostaje.
