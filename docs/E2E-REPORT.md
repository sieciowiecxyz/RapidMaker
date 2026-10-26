# E2E RapidMaker — 2026-10-26

Raport historyczny dla main `0db0058`, przed integracją PR #4. Aktualne sprawdzenia: [integration-piotr.md](integration-piotr.md).

**31 scenariuszy integracyjnych/E2E + 20 kontraktów UI PASS.** Testy i naprawy wykonano na lokalnej, izolowanej instancji. Osobna blokada DPD poniżej nie jest deklarowana jako sukces.

Uruchomienie: `npm --prefix frontend run test:all`. Grupy, wymagania i pojedyncze regresje: [instrukcja](../frontend/tests/README.md). Runner tworzy osobną bazę, pliki i konta; nie zmienia normalnego localhostu. Stripe wyłącznie test, Furgonetka wyłącznie sandbox, email tylko `void@sieciowiec.xyz`.

## Co sprawdzamy

| Grupa | Rzeczywisty zakres |
| --- | --- |
| core | STL/STEP/3MF → prywatny GLB; błędne pliki/archiwa, retencja i izolacja klientów; slicer, 3 rodziny i wszystkie aktualne kolory; jakość/wytrzymałość/ilość/opcje zaawansowane; viewer, granice pakowania/API; ustawienia hasła/passkey/TOTP/recovery; dokumenty |
| sandbox | Odbiór, kurier i paczkomat; dane/faktura/walidacja; zmiana punktu i edycja; odrzucona karta i utrata odpowiedzi bez podwójnej próby; Stripe paid; panele klient/admin, pliki/chat/filtry/statusy/CSV, katalog/cennik, zwrot 5 zł + pozostałe 20 zł, blokada nadmiernego/cudzego refundu; logistyka i porzucona płatność |
| email | Hasło/passkey + rzeczywisty kod email; ręczna oferta → płatność → kopia warunków → rozmowa → odpowiedź z emaila, bez duplikatu |
| registration | Osobne bazy dla pierwszego hasła i pierwszego passkey; email, warunki, ochrona ostatniej metody i dodanie hasła |
| contracts | Kontrolowane odpowiedzi API dla błędów, niepewności, ponowień i wyłączonych funkcji; nie stanowią dowodu integracji z dostawcami |

Mapa: [kontrolki](e2e-controls.md), [konfigurator](e2e-configurator-inventory.md), [auth/checkout](e2e-auth-checkout-inventory.md), [panele](e2e-panels-inventory.md). Inwentarz obejmuje 368 deklaracji w 42 plikach Svelte, również współdzielone, dynamiczne i nieaktywne; nie oznacza 368 osobnych zaliczonych funkcji. Testujemy każdą oferowaną wartość grupy, nie iloczyn wszystkich kombinacji.

## Wyniki wykonania

| Zestaw | Wynik | Dowód prywatny |
| --- | --- | --- |
| Core: konfigurator, modele, ceny, bezpieczeństwo, dokumenty/API | 18 PASS, 159 s | core.json |
| Checkout, dostawa i panele/refundy | 7 PASS po poprawkach i powtórce panelu | sandbox-final.json: 6 PASS; panels-final.json: ostatni 1 PASS, 20,5 s |
| Logistyka DHL | 2 PASS, 62 s | logistics-dhl.json |
| Rejestracja hasłem i passkey | 2 PASS, dwie bazy/email | registration-password.json, registration-passkey.json |
| Logowanie email + ręczna oferta/płatność/email → chat | 2 PASS, 178 s | email.json |
| Kontrakty UI z kontrolowanymi odpowiedziami | 20 PASS, 4,2 s | contracts.json |

Wynik końcowy składa się z pełnych grup oraz powtórek konkretnych regresji, bez liczenia tych samych scenariuszy ponownie. Poprzednie FAIL obejmowały błędy produktu oraz autorstwa testów (selektory, oczekiwanie końca logout i poprawnego powrotu klienta na `/`); nie osłabiono kontroli kwot ani uprawnień. Podgląd zoomu zmienia rzeczywisty render i obraz. Obejrzane widoki desktop/mobile konfiguratora i paneli; wybrane podsumowania mają kontrolę overflow.

DHL: rzeczywiste nadanie/PDF/propozycje i rezerwacja podjazdu/anulowanie oraz osobne przekazanie → shipped. Dwie paczki fizyczne tworzą jedno zlecenie przewozu.

**DPD BLOCKED:** dwie próby nadania odrzucone `carrierValidationError`, szczegóły: timeout odpowiedzi API DPD. Zamówienia pozostały opłacone, nadanie w manual_review; brak potwierdzonego nadania/etykiety/podjazdu. Przełączenie scenariusza logistyki na DHL nie usuwa tej blokady z raportu.

## Wykryte i poprawione błędy

- Utrata jednostek STL i jakości roboczej przy przejściu między konfiguratorami; błędna wartość pola po odrzuceniu zmiany. Zachowanie poprawnych wartości i otwartego edytora przy ponowieniu.
- Odtworzenie animacji pozostawiało podgląd w pauzie; kontrola zoomu podczas przewijania/zmiany rozmiaru nie powinna zależeć od opóźnionego IntersectionObserver.
- Nakładka linku wiersza blokowała przycisk rozliczenia. Zwykły klik działa bez `force`.
- Gościnny błąd 401 zapętlał remontowanie checkoutu. Fałszywy powrót płatności pokazuje błąd zamiast nieskończonego ładowania.
- Wynik płatności wskazywał lokalny identyfikator zamiast trwałego zamówienia. API zwraca autoryzowany `order_id`, zalogowany klient otwiera właściwe szczegóły; gość zachowuje dobrowolne założenie konta.
- Po pierwszym dodaniu hasła UI nadal blokował usunięcie jedynego passkey. Stan kluczy odświeża się po zapisie hasła; backend nadal chroni ostatnią metodę.
- Furgonetka normalizuje telefon krajowy do +48. Porównanie przesyłki uznaje równoważny polski numer, nadal odrzuca obcy prefiks/odbiorcę i zmienioną geometrię. Masa jest jawnie zaokrąglana w górę do 100 g przed wyceną i nadaniem, zamiast zaskakującej normalizacji dostawcy po zapisie.

## Granice

Nie jest to audyt penetracyjny, test obciążeniowy ani certyfikat bezpieczeństwa/produkcji. Cennik fixture sprawdza obliczenia i rozliczenie groszy; nie potwierdza rentowności Twoich rzeczywistych kosztów. Nie sprawdzamy fizycznego authenticatora, rzeczywistego przewozu ani całej macierzy przeglądarek/urządzeń. Dostawcy mogą ograniczyć sandbox; podjazd wymaga potwierdzenia, samo HTTP 202 nie wystarcza. Wyłączone funkcje, m.in. wystawianie faktur, rysunek PDF i niewdrożone pola produkcji, pozostają jawnie niedostępne. Linki tel/mailto sprawdzamy bez telefonowania/wysyłania wiadomości poza autoryzowanymi testami.

Trace oraz screenshoty formularzy z kodami/kluczami/kartą są wyłączone; prywatne zrzuty wybranych podsumowań i paneli zawierają tylko dane fixture. Prywatnych raportów JSON/logów/DOM/screenów nie commitujemy. Usunięto dwa stare testy demonstracyjnego logowania; rzeczywiste odpowiedniki są w nowych scenariuszach, bez przywracania fikcyjnych sukcesów.

## Kod i sprawdzenia

Bez nowych zależności. Nowe pliki to testy/helpery/runner/inwentaryzacja i dokumentacja, bez nowej warstwy produktu. Rust `backend/crates`: 28140 → 28171, **+31 linii kodu** (tokei, bez komentarzy/pustych/generowanych; łącznie z testami inline i przykładem fixture, bez niezależnego podziału production/test). Sprawdzone: Rust build; fmt --all; Clippy api/shipping all-targets z -D warnings; api 22 PASS / 1 ignored, shipping 20 PASS / 1 ignored, pricing 5 PASS, packing 15 PASS. Ignorowane testy wymagają osobnych fixture; sandbox i katalog są sprawdzane przez opisane integracje, ale nie deklarujemy uruchomienia ignorowanych testów. Frontend: svelte-check 0 błędów/ostrzeżeń + Vite build, Python py_compile. Pozostaje wcześniejsze ostrzeżenie C++ w generowanym mostku OCCT; Clippy bez błędów. Prettier dla zmienionych sformatowanych plików frontendu/testów, Python py_compile i diff --check PASS. Nie uruchamiano CI ani produkcji. Cudze zmiany AGENTS.md i result.json pozostawiono poza commitem.
