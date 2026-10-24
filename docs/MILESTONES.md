# Finalizacja — kolejność i kryteria

Active: F1 (równolegle rozpoznanie F3/F4). Cel i kontrakty: [PLAN.md](PLAN.md). Dowody zapisuj tutaj krótko: PASS/FAIL/NOT RUN, komenda/raport i commit. Starsze etapy są w historii Git, a nie dowodem obecnej integracji.

## F0 — Stan wyjściowy i uruchomienie

- [x] Kod/style sprawdzone; frontend/API HTTP 200, lokalna konfiguracja DB/Stripe/Furgonetka/auth obecna (sam odczyt konfiguracji, nie dowód sandboxów). Infrastruktura testów korzysta z osobnej DB/storage; przygotować świeżą instancję do F6.
- [x] Wyjściowy Rust LOC: 26219, łącznie z testami inline, bez komentarzy/pustych/generowanych. P1P ogranicza automatyczny druk niezależnie od progu 500 mm; TPUT/profile wymagają rzeczywistego dopuszczenia.
- Przejście: frontend/API dostępne; znane fakty i blokery zapisane, prace podzielone na rozłączne pliki.

## F1 — Podglądy i prosty wybór druku

- [ ] STL/STEP/3MF: rzeczywisty podgląd przed wyborem koloru, domyślnie biały; kolor aktualizuje wyłącznie wygląd. Podglądy także w częściach zamówienia klienta/admina, z autoryzowanym dostępem.
- [ ] PLA/ABS/TPU jako przyciski podobne do jakości/wytrzymałości; bez support i powielonych odmian. Kolory wybieralne niezależnie od magazynu, także przy wycenie/koszyku; brak profilu/ceny prowadzi uczciwie do ręcznej obsługi.
- [ ] Ponowny upload, usunięcie części, błąd i wiele plików bez pomylenia modelu/podglądu.
- Przejście: różne prawdziwe fixture trzech formatów przechodzą od uploadu do podglądu; zrzuty desktop/mobile, build i regresje zmiany PASS.

## F2 — Poprawna automatyczna wycena

- [ ] Slicer: 1/wiele/1000 sztuk, kilka części i płyt, jakość/wytrzymałość/kolor, brak starej ceny po zmianie parametrów.
- [ ] Regresje groszy, minimum/VAT, jednego przygotowania, energii/filamentu i całej partii; brak ujemnych/NaN/przepełnień i zmyślonych stawek.
- [ ] >500 mm na osi → ręczna wycena; mniejszy model niepasujący do faktycznej P1P również ręcznie. Granica, rotacja i błędny model sprawdzone.
- [ ] Dostawa wielu sztuk: realna masa/wymiary serwera, porównanie kilku pakowań i cen sandbox, cache/ważność oferty, brak stałej ceny dla 1000 sztuk.
- Przejście: zapisane wejścia i obliczenia kontrolne zgadzają się z API/slicerem; zmiana konfiguracji nie płaci poprzedniej kwoty.

## F3 — Ręczna oferta → email → zapłata

- [ ] Trwała prośba klienta i własne pliki; admin „Oczekuje na wycenę”, formularz ceny/zakresu/dostawy/terminu, walidacja PLN/groszy.
- [ ] Publikacja oferty + outbox atomowo; klient dostaje link do chronionego zamówienia i może zapłacić na naszej stronie.
- [ ] Wersja i termin 7 dni, retry/restart/podwójne kliknięcie, brak edycji przy rozpoczętej/niepewnej/opłaconej próbie; bez obchodzenia własności i kwoty.
- Przejście: Playwright klient → admin → wiadomość → klient → Stripe test → paid; jeden zapis zapłaty/emaila. Nie tylko mock HTTP.

## F4 — Retencja, uploady i bezpieczeństwo

- [ ] Porzucony upload/szkic 24 h; wygasła nieopłacona próba po uzgodnieniu Stripe zwalnia hold i usuwa wszystkie pliki po terminie.
- [ ] Ręczne zgłoszenie 14 dni/oferta 7 dni; opłacone i niepewne płatności nie tracą plików; wspólny model chroni każde aktywne zamówienie. Snapshoty pozostają.
- [ ] Negatywne uploady: rozmiar/archiwum/path/symlink/zewnętrzne zasoby, uszkodzony plik, timeout i quota; dostęp do cudzych GLB/oryginałów/zamówień/rozmów/admina, CSRF i replay.
- Przejście: testy izolowanej DB/storage z kontrolowanym czasem dowodzą usuwania i ochrony; brak sekretów/PII w raportach. Bez atakowania dostawców.

## F5 — Spójność całej strony

- [ ] Przejść landing, konfigurator, checkout, logowanie/rejestrację/2FA, konto, zamówienia/podglądy/chat/ustawienia i admina zgodnie ze stylem Piotra.
- [ ] Ujednolicić tokeny/odstępy/przyciski/pola, usunąć zbędne linie i teksty demo, poprawić focus/błędy/loading/mobile bez pobocznego redesignu.
- [ ] Linki, powrót do checkoutu po loginie, puste listy, nieaktywna oferta i retry nie gubią danych ani nie sugerują fikcyjnego sukcesu.
- Przejście: obejrzane screenshoty desktop/mobile wszystkich kluczowych widoków; brak poziomego przepełnienia i niespójnych kontrolek.

## F6 — Automatyczny przebieg całości i wielu klientów

- [ ] Jeden udokumentowany sposób uruchomienia Playwright + prawdziwe lokalne API/osobna DB/storage i przygotowanie użytkowników/testowych stawek, bez sekretów w repo.
- [ ] Uploady STL/STEP/3MF → podglądy → slicer → koszyk → dostawa → Stripe test → konto/admin/status/chat/email; osobno ręczna wycena i zapłata, anulowanie/odrzucenie i wygasanie.
- [ ] Hasło i passkey z drugim składnikiem, TOTP bez fallback email; dwa konta nie widzą cudzych danych, błędne/ponowione płatności nie zmieniają kwot.
- [ ] SMTP/odbiór testowy void: powiadomienie o ofercie/statusie, chat → email → odpowiedź w tej samej rozmowie; kontrolowane limity liczby wiadomości.
- [ ] Kilku klientów i uploadów jednocześnie: HTTP pozostaje responsywne, limitowane workery, brak pomieszanych wyników, lease/retry po restarcie, czytelne 202/429.
- Przejście: realne przebiegi PASS, powtarzalny raport bez sekretów i jasne oddzielenie testów mock od sandbox; każda usterka naprawiona albo jawny zewnętrzny bloker. Goal complete dopiero po spełnieniu wymagań, nie po samym napisaniu testów.

## Checkpoint

2026-10-25: plan utworzony na prośbę użytkownika; wykonanie rozpoczyna F0. HEAD przed finalizacją: b42f380. Lokalne zmiany użytkownika: AGENTS.md i result.json — zachować poza commitami celu. Obecny frontend/API są połączone; część wcześniejszych E2E używa mocków. Nie jest to jeszcze dowód przejścia F1–F6. Styl: storefront/theme.css → theme.css, zgodnie z frontend/README.md. P1P i limity obecnych profili są mniejsze niż próg ręcznej wyceny 500 mm. Niepotwierdzona płatność obecnie chroni pliki — wymaga sprawdzenia wygaszania, nie bezwarunkowego kasowania.

F0: PASS odczyt stanu i HTTP, NOT RUN nowe sandboxy; F1 frontend i Rust wykonywane równolegle przez workerów implementacyjnych. AGENTS.md i result.json pozostają cudzymi zmianami.
