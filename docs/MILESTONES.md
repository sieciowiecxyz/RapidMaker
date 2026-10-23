# Milestones — backend Rapidmaker

Stan 2026-10-23: B1–B5 PASS jako lokalny POC; dowody i ograniczenia w [checkpoincie](milestones/backend.md). Kalibracja przed sprzedażą pozostaje wymagana.

Kolejność B1 → B2 → B3 → B4 → B5. Każdy etap kończy krótki check działania, bez dodatkowego review/testujących agentów. Niepotwierdzone kryterium pozostaje otwarte.

## B1 — Osobny crate API

- Wydziel `app/src/api` do `crates/api`; `app` tylko konfiguracja i start serwera/workera.
- Zachowaj endpointy, walidację i izolację analiz. Moduły nie zależą od API.
- Gotowe gdy: build i obecne testy zmienionego API przechodzą; nie znikają zabezpieczenia.

## B2 — PostgreSQL i katalog Bambu

- SQLx i migracje: materiały, kolory, zapis wycen. Import lokalnego snapshotu z wersją źródła.
- Cena zakupu netto PLN/kg, dostępność kolorów i dopuszczenie produkcyjne są danymi operatora; ponowny import ich nie nadpisuje.
- Publiczny odczyt katalogu bez danych prywatnych; zapis admina dopiero z autoryzacją backendu.
- Gotowe gdy: migracja od zera i powtórny import działają; katalog ma 49 materiałów/314 kolorów, dane przetrwają nowe połączenie.

## B3 — Rzeczywisty slicer

- P1P hardened steel 0,4 mm, zatwierdzony profil materiału i druku; najpierw PLA Basic/standard.
- Rozwiąż `inherits`/`include`; kontrolowany proces bez sekretów i sieci, z timeoutem i ograniczeniami. Odrzuć obce profile/G-code.
- Odczytaj czas i masę z rzeczywistego wyniku. Błąd oznacza brak automatycznej ceny. Przeanalizowany model pozostaje prywatny.
- Gotowe gdy: prawdziwy model daje dodatni czas i gramy; uszkodzony plik i niedozwolony profil nie dają ceny.

## B4 — Automatyczna wycena

- Czysty `pricing` liczy koszty z pomiarów slicera i jawnego cennika; wynik API ma rozbicie netto/VAT/brutto i wersje danych.
- Zweryfikowane ustawienia jakości/wytrzymałości wpływają na slicer. Nie oferuj opcji, których silnik nie stosuje.
- Uwzględnij ilość, przygotowanie i partie. Cache po modelu/ustawieniach/profilach; ograniczony rozmiar i czas życia. Zapisz niezmienny wynik w PostgreSQL.
- Gotowe gdy: API wycenia prawdziwy model; większe zużycie podnosi koszt; brak ceny/profilu wymusza ocenę ręczną. Sprawdź 1/2/10/100 sztuk i pokaż ograniczenia szacowania partii.
- Przed sprzedażą Kuba zatwierdza rzeczywiste ceny zakupu, koszty pracy/energii/maszyny, marżę i porównanie z pomiarami wydruków.

## B5 — Admin, frontend i checkout

- Admin dodaje/edytuje materiały, kolory, koszty i dopuszczenia przez autoryzowane API. Import nie niszczy zmian operatora.
- Prywatne oryginały/siatki i własność są zapisane; nowa wycena używa ID modelu. Konfigurator pobiera rzeczywisty katalog i wynik wyceny; pokazuje loading, błąd i ręczną ocenę. Zachowaj minimalistyczny wygląd.
- Pakowanie/Furgonetka korzystają z właściwej masy/wymiarów; Stripe otrzymuje wyłącznie ważną kwotę serwerową. Zmiana koszyka unieważnia ofertę.
- Gotowe gdy: jeden przebieg konfiguracja → wycena → sandbox checkout działa, a obcy zapis admina jest odrzucony. Krótki przegląd zmienionych ekranów na desktop/mobile.

## Później

[Email E1–E4](email.md): PASS jako backend lokalny — SMTP, kody i sesje klienta, outbox, preferencje, trwałe zamówienia i rozmowy. Kryteria, dowody oraz granice przejścia do produkcji w podlinkowanym planie. UI pozostaje do podłączenia.

Podłączenie paneli do nowych API, publiczne wdrożenie auth i produkcyjne webhooki płatności, IMAP/email → chat oraz logi/metryki Grafany. Publiczne uruchomienie wymaga osobnego przygotowania; obecne środowisko pozostaje lokalnym POC.

Auth backend PASS: klient email, admin passkey, trwałe sesje, HTTPS cookie i własność modeli/wycen/zamówień. Weryfikacja: Rust, PostgreSQL, Chromium z wirtualnym kluczem. [Kontrakt i bootstrap](auth.md).
