# Logowanie i zabezpieczenia konta

Rejestracja przekazuje `legal_version`; backend sprawdza aktualną wersję, zapisuje akceptację po potwierdzeniu emaila i kolejkuje kopię warunków w tej samej transakcji. Brak/stara wersja: 409. Istniejące logowanie nie jest ponowną rejestracją. [Dokumenty i ograniczenia](legal.md).

Klient i admin: **hasło albo passkey → obowiązkowo drugi składnik**. Domyślnie kod email; po włączeniu aplikacji uwierzytelniającej TOTP zastępuje email. Role i uprawnienia nadaje wyłącznie backend.

## Przepływ

1. `/logowanie`: email i hasło albo natywny passkey (PIN/biometria).
2. Kod email lub TOTP. Przy TOTP można użyć jednego z ośmiu jednorazowych kodów odzyskiwania; nie ma obejścia przez email.
3. Dopiero pełne logowanie daje dostęp do konta, zamówień i operacji admina. Sesja oczekująca wygasa po 10 minutach.

Nowe konto i stare konto bez hasła/passkey: potwierdzenie email otwiera ograniczony etap konfiguracji. Ustawienie pierwszego hasła lub passkey kończy rejestrację z potwierdzonym emailem. Później sam email nie loguje ani nie resetuje zabezpieczeń. Migracja `0018` wylogowuje dotychczasowe sesje.

`/konto/ustawienia`: ustawienie/zmiana hasła, dodawanie i usuwanie passkey, TOTP, wylogowanie pozostałych urządzeń. Zmiana zabezpieczeń wymaga pełnego logowania z ostatnich 5 minut, rotuje cookie i unieważnia pozostałe sesje. Ostatniego passkey nie usuniesz bez ustawionego hasła. Maksymalnie pięć kluczy. TOTP dodaje się przez lokalny klucz/URI do aplikacji; kody odzyskiwania są wyświetlane tylko po aktywacji i nie trafiają do storage przeglądarki. Wyłączenie TOTP przywraca email jako drugi składnik.

## API

| Metoda i ścieżka | Działanie |
| --- | --- |
| POST `/api/auth/password/login` `{email,password}` | Hasło → sesja oczekująca na drugi składnik. |
| POST `/api/auth/passkey/login/start` `{email}` | Opcje WebAuthn i challenge_id. |
| POST `/api/auth/passkey/login/finish` `{challenge_id,credential}` | Passkey → sesja oczekująca na drugi składnik. |
| POST `/api/auth/factor/verify` `{code}` | Email/TOTP/kod odzyskiwania → pełna sesja. |
| POST `/api/auth/factor/resend` `{}` | Ponowienie kodu email; niedostępne dla TOTP. |
| POST `/api/auth/start` `{email}`, `/verify` `{challenge_id,code}` | Wyłącznie rejestracja/migracja kont bez hasła i passkey. |
| GET `/api/auth/me` | user, stage, second_factor, csrf_token, expires_at, admin_verified. |
| PUT `/api/account/password` `{password}` | Ustawienie lub zmiana hasła. |
| GET `/api/account/security` | has_password, totp_enabled, passkeys, recovery_codes_remaining. |
| POST `/api/account/totp/start` `{}` | Tymczasowy secret i otpauth_uri, ważne 5 minut. |
| POST `/api/account/totp/confirm` `{code}` | Aktywacja, nowa sesja i osiem recovery_codes. |
| DELETE `/api/account/totp` | Wyłączenie TOTP i unieważnienie kodów odzyskiwania. |
| POST `/api/auth/passkey/register/start` `{bootstrap?}` i `/finish` `{challenge_id,credential,name}` | Rejestracja własnego klucza. |
| GET `/api/auth/passkeys`, DELETE `/api/auth/passkeys/{id}` | Lista/usuwanie własnych kluczy. |
| POST `/api/auth/logout`, `/logout-all` `{}` | Wylogowanie urządzenia/wszystkich sesji. |
| POST `/api/auth/claim-guest` `{}` | Przypisanie własnych zasobów gościa po pełnym logowaniu. |

`stage`: setup, primary, second_factor, authenticated. `second_factor`: email albo totp. Business API przyjmuje wyłącznie authenticated. Mutacje sprawdzają Origin i CSRF; publiczne starty logowania mają własne limity i powiązanie cookie. Schematy endpointów są generowane w `/api/`.

## Bezpieczeństwo

- Hasło: 12–128 znaków, maks. 512 bajtów UTF-8, bez normalizacji. Argon2id: 19 MiB, dwie iteracje, równoległość 1, losowa sól. Kosztowna praca poza pętlą async, ograniczona semaforem i limitami DB.
- TOTP: RFC 6238, 6 cyfr, 30 s, tolerancja jednego kroku. Zużyte kroki nie działają ponownie. Sekret szyfrowany XChaCha20-Poly1305, klucz wyprowadzany z AUTH_CODE_KEY, związany z ID konta. Kody odzyskiwania przechowywane jako hashe i zużywane atomowo.
- Próby drugiego składnika: 5 na sesję oraz 20 na konto/15 minut, dodatkowe limity IP/globalne. Nowa przeglądarka lub adres IP nie resetują limitu konta. Niepotwierdzone logowania nie wypierają działających sesji.
- WebAuthn: webauthn-rs, dokładny Origin/RP ID, wymagane UV, jednorazowe wyzwania PostgreSQL związane z kontem i sesją. Brak własnej kryptografii podpisów.
- Cookie HttpOnly/SameSite=Lax; w produkcji Secure i prefix __Host. DB trzyma hashe tokenów, bez JWT. Klient: 30 dni/7 dni bezczynności; admin: 8 h/30 min. Rola/blokada/logout unieważniają dostęp w DB.
- Własność modeli, zamówień, rozmów i plików sprawdzana na serwerze. Claim gościa wymaga obu sesji; email zamówienia nie nadaje własności.

## Konfiguracja i odzyskanie

AUTH_MODE=localhost albo production (domyślnie disabled). Lokalnie AUTH_ORIGIN=http://localhost:18430 — passkey wymaga localhost, nie 127.0.0.1. Production wymaga dokładnego HTTPS Origin i publicznego TLS. AUTH_TRUSTED_PROXY opcjonalnie wskazuje jeden zaufany adres proxy.

AUTH_CODE_KEY: 32 losowe bajty hex w `.env`/SOPS. **Zachowaj kopię klucza: jego wymiana uniemożliwia odczyt istniejących sekretów TOTP.** Nie ma automatycznego przejścia na email po błędzie odszyfrowania. Hasła, kody, sekrety i pełne adresy nie trafiają do logów.

Pierwszy admin: zakończ pełne logowanie klienta, następnie lokalnie `python backend/run.py --auth-bootstrap EMAIL`. W tej samej świeżej sesji otwórz `/konto/ustawienia?bootstrap=1` i zarejestruj passkey z jednorazowym zezwoleniem. Samo CLI ani formularz nie nadają roli bez poprawnej ceremonii.

`--auth-recover EMAIL` służy tylko wymianie kluczy admina, który już zalogował się hasłem + email (bez TOTP). Nie resetuje hasła/TOTP. Utrata telefonu: podstawowa metoda + kod odzyskiwania. Utrata hasła: alternatywny passkey. Utrata wszystkich metod wymaga osobnej procedury właściciela; email nie omija MFA.

## Moduły i sprawdzenie

`auth/credentials.rs`: Argon2, TOTP i szyfrowanie. `storage/credentials.rs`: transakcje, limity, etapy, rotacja i replay. `api/account_security.rs`: endpointy; istniejące auth/passkeys zachowują granice HTTP. Frontend: Login, AccountSecurity, Passkeys i wspólny adapter sesji/CSRF.

Testy obejmują izolowany PostgreSQL, cookie/Origin, autoryzację routera oraz Chromium z wirtualnym WebAuthn i kontrolowanym API: hasło + email, passkey + TOTP, błędny kod, ustawienia i brak sekretów w browser storage. Nie zastępują fizycznego klucza, publicznego TLS ani testu doręczenia emaila. Aktualny wynik: docs/testing.md.
