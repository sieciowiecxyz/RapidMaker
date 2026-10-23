# Rapidmaker — backend i automatyczna wycena

Mode: goal
Status: B1–B5 zrealizowane jako lokalny POC
Active: brak — backend email E1–E4 i auth ukończone

## Email i backend klienta

[Email i powiadomienia](email.md): E1–E4 PASS w lokalnym backendzie. Kod logowania przez iCloud, sesje PostgreSQL, trwałe zamówienia/rozmowy, outbox i preferencje mają API. Dowody i ograniczenia w jednym dokumencie. UI nie jest jeszcze podłączone; auth/passkey są zaimplementowane, produkcyjne webhooki pozostają planem.

## Cel

Klient dodaje STL, STEP lub 3MF, wybiera materiał, kolor, jakość i ilość. Serwer wylicza cenę z wyniku rzeczywistego slicera. Prosty frontend korzysta z jednego API; kwota i dostępność zawsze należą do backendu.

## Decyzje

- Rust: `app → api → moduły`. `app` uruchamia serwer; `api` zbiera trasy, walidację i uprawnienia. Obliczenia pozostają w `geometry`, `slicer`, `pricing`, `packing`; SQL w `storage`. Bez mikroserwisów, Redis i brokera.
- PostgreSQL + SQLx: materiały, kolory, konfiguracja cen i niezmienne wyniki wycen; trwała kolejka zadań w późniejszym zakresie. Prywatne modele/presety na dysku; w bazie identyfikatory i wersje. Nie odtwarzamy stanu z przeglądarki.
- Źródło katalogu: lokalne Bambu Studio, snapshot w `backend/data/bambu/`. 49 materiałów i 314 kolorów. Import zachowuje własne ceny, magazyn i dopuszczenia; nie uznaje kosztu slicera za cenę netto PLN.
- Sprzęt: Bambu Lab P1P, dysza hardened steel 0,4 mm. Profile deklarujące zgodność wymagają zatwierdzenia dla faktycznego ekstrudera/obudowy. Materiały podporowe nie są osobnym produktem.
- Slicer: kontrolowany proces Bambu Studio, nasze profile po rozwiązaniu dziedziczenia i dołączeń; bez sieci i sekretów, z limitami czasu/pamięci/plików oraz ograniczoną równoległością. Ustawienia klienta i G-code nie sterują procesem.
- Cena: przygotowanie + filament (także podpory/purge/odpady) + czas maszyny + energia + praca + straty, następnie dodatnia marża i podatki. Kwoty w groszach, jawne zaokrąglanie; dostawa/opakowania oddzielnie, bez podwójnego naliczania. Parametry testowe nie dowodzą rentowności.
- Ilość: rzeczywiste rozmieszczenie na płytach i kolejne partie; nie mnożymy całej opłaty przygotowania przez sztuki. Do potwierdzenia partii dopuszczalny jest jawnie ostrożny wariant, bez udawania optymalizacji.
- Brak profilu, ceny zakupu, zgodności lub wyniku slicera oznacza ręczną ocenę, nigdy wymyśloną cenę. Wersja modelu, profili i kosztów należy do zapisanego wyniku.

## Kolejność

[B1–B5 i kryteria](MILESTONES.md). Najpierw działający backend i jeden rzeczywisty przebieg PLA, potem kolejne zatwierdzone materiały, admin i checkout. Nie przepisujemy teraz frontendu.

Dalszy zakres: podłączenie UI do backendu kont/zamówień/rozmów, publiczne wdrożenie auth i produkcyjne webhooki Stripe, logi/metryki Grafany i publiczne wdrożenie. Istniejące sandboxy Stripe/Furgonetki zachowujemy; nie są gotową sprzedażą produkcyjną.

## Sposób pracy

KISS, minimum kodu i zależności. Workerzy implementują niezależne moduły; główny agent integruje. Krótki build i sprawdzenie zmienionej funkcji, bez Astry i szerokich testów przy każdej zmianie. Nie omijamy izolacji, autoryzacji ani kontroli kwot. Stan i dowody: [checkpoint](milestones/backend.md).

## Potwierdzenie POC

W poprzednim przebiegu potwierdzono katalog/panel operatora, prywatne modele, slicer, koszt wielu części z jednym przygotowaniem oraz konfigurator → adres Stripe → podsumowanie → Stripe Checkout test. Furgonetka zwróciła trzy oferty; testowa płatność miała kwotę serwera. Build, wybrane testy i desktop/mobile PASS. Bez realnej opłaty/nadania. Po zmianie auth panel wymaga podłączenia nowego logowania. Nie potwierdzono kalibracji, produkcyjnych zamówień/webhooków ani publicznego bezpieczeństwa. Dowody i ograniczenia: [checkpoint](milestones/backend.md).

## Auth — 2026-10-23

Klient: email; admin: email + passkey. Sesje PostgreSQL, CSRF/Origin, cookie HTTPS, lokalny bootstrap/recovery i własność konta działają. Stary Bearer/operator-cookie usunięte. Dowody: [auth](auth.md). Frontend wymaga podłączenia; fizyczny klucz i publiczny TLS nie były testowane.
