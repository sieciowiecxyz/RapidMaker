# Plan: pełne ścieżki E2E RapidMaker

Cel: sprawdzić rzeczywistą stronę od uploadu przez konfigurację, dane/dostawę i Stripe po obsługę klienta/admina, zwroty i logistykę. Zachowujemy styl i stos; naprawiamy błędy odkryte przez testy. Wyniki i granice w krótkim `docs/E2E-REPORT.md`.

## Zakres

- Mapa wszystkich szablonowych kontrolek: `docs/e2e-controls.md` (generator `frontend/tests/inventory-controls.mjs`); doprecyzowanie aktywnych tras, opcji dynamicznych, kontrolek wyłączonych i martwych widoków w inwentarzach obszarów. Liczba kontrolek nie jest wynikiem testu.
- Konfigurator: STL/STEP/3MF, podgląd, błędne pliki, wiele części, wszystkie oferowane materiały/kolory/jakości/wytrzymałości, ilości i opcje zaawansowane; zachowanie danych przy nawigacji i odtworzeniu. Nie testujemy iloczynu wszystkich możliwych kombinacji.
- Checkout: walidacja danych/faktury, odbiór, kurier, paczkomaty i zmiana punktu, pakowanie i oferta/cena serwera, edycja/wstecz/retry, regulamin, prawdziwa płatność Stripe test oraz niepowodzenie/anulowanie.
- Tożsamość: rejestracja i hasło/passkey + email/TOTP, błędy kodów, powrót do celu, ustawienia hasła/kluczy/aplikacji, wylogowanie i ochrona tras.
- Klient/admin: listy/szukanie/filtry, szczegóły/pliki/podglądy, wiadomości, powiadomienia, statusy, ręczna oferta i zapłata, materiały/ceny, eksport, zwroty częściowe/pozostałe i limity; logistyka dostępna w rzeczywistym sandboxie.
- Ochrona: cudze zasoby, niedozwolony admin, CSRF, błędne kwoty, retry bez podwójnej operacji, retencja plików, współbieżni klienci; błędy HTTP/JS i wygląd desktop/mobile.

## Wykonanie

Playwright z rzeczywistym Rust API, PostgreSQL, parserem/slicerem; Stripe i Furgonetka wyłącznie sandbox/test, email wyłącznie void@sieciowiec.xyz. Osobna baza `rapidmaker_test_browser_*`, katalog modeli i porty 18436/18437; sprzątanie tylko własnych zasobów. Bez zmian normalnych kont, zamówień i uploadów.

Istniejące kontrakty z podstawionym API zachowujemy dla awarii/retry i trudnych stanów, ale raport wyraźnie odróżnia je od E2E prawdziwych integracji. Nie podszywamy się pod udaną odpowiedź dostawcy. Rzeczywiste błędy poprawiamy; nie osłabiamy kwot/uprawnień/2FA ani testów. Funkcja niedostępna/niezaimplementowana albo blokada dostawcy dostaje jawny status i dowód, nie PASS. Martwe lub niedziałające przyciski naprawiamy albo uczciwie wyłączamy, bez tworzenia fikcyjnej funkcji.

Testy sprawdzają efekt akcji i stan serwera, nie samo kliknięcie. Reużywamy flow/helperów; nie dublujemy istniejących scenariuszy. Formularze i błędy sprawdzamy selektorami dostępnymi dla użytkownika; WebAuthn przez wirtualny authenticator. Trace/screenshoty wyłączone tam, gdzie ujawniłyby kody/sekrety/karty. Raport nie zawiera sekretów ani danych prawdziwych klientów.

Workerzy Sol medium implementują rozłączne obszary konfiguratora i paneli. Główny agent trzyma plan, integruje helpery/runner, checkout/auth oraz wyniki; bez Astry i dodatkowych bramek review. Rozszerzony zakres testów wynika z aktualnej wyraźnej prośby użytkownika, mimo wcześniejszego trybu POC.

Kolejność i kryteria: [MILESTONES.md](MILESTONES.md). Starszy plan finalizacji zachowany w historii Git, ostatnia wersja przed tym etapem: e5a7f28.
