# Rapidmaker — backend i automatyczna wycena

Mode: goal
Active: B3

## Cel

Klient dodaje STL, STEP lub 3MF, wybiera materiał, kolor, jakość i ilość. Serwer wylicza cenę z wyniku rzeczywistego slicera. Prosty frontend korzysta z jednego API; kwota i dostępność zawsze należą do backendu.

## Decyzje

- Rust: `app → api → moduły`. `app` uruchamia serwer; `api` zbiera trasy, walidację i uprawnienia. Obliczenia pozostają w `geometry`, `slicer`, `pricing`, `packing`; SQL w `storage`. Bez mikroserwisów, Redis i brokera.
- PostgreSQL + SQLx: materiały, kolory, konfiguracja cen, zadania i niezmienne wyniki wycen. Prywatne modele/presety na dysku; w bazie identyfikatory i wersje. Nie odtwarzamy stanu z przeglądarki.
- Źródło katalogu: lokalne Bambu Studio, snapshot w `backend/data/bambu/`. 49 materiałów i 314 kolorów. Import zachowuje własne ceny, magazyn i dopuszczenia; nie uznaje kosztu slicera za cenę netto PLN.
- Sprzęt: Bambu Lab P1P, dysza hardened steel 0,4 mm. Profile deklarujące zgodność wymagają zatwierdzenia dla faktycznego ekstrudera/obudowy. Materiały podporowe nie są osobnym produktem.
- Slicer: kontrolowany proces Bambu Studio, nasze profile po rozwiązaniu dziedziczenia i dołączeń; bez sieci i sekretów, z limitami czasu/pamięci/plików oraz ograniczoną równoległością. Ustawienia klienta i G-code nie sterują procesem.
- Cena: przygotowanie + filament (także podpory/purge/odpady) + czas maszyny + energia + praca + straty, następnie dodatnia marża i podatki. Kwoty w groszach, jawne zaokrąglanie; dostawa/opakowania oddzielnie, bez podwójnego naliczania. Parametry testowe nie dowodzą rentowności.
- Ilość: rzeczywiste rozmieszczenie na płytach i kolejne partie; nie mnożymy całej opłaty przygotowania przez sztuki. Do potwierdzenia partii dopuszczalny jest jawnie ostrożny wariant, bez udawania optymalizacji.
- Brak profilu, ceny zakupu, zgodności lub wyniku slicera oznacza ręczną ocenę, nigdy wymyśloną cenę. Wersja modelu, profili i kosztów należy do zapisanego wyniku.

## Kolejność

[B1–B5 i kryteria](MILESTONES.md). Najpierw działający backend i jeden rzeczywisty przebieg PLA, potem kolejne zatwierdzone materiały, admin i checkout. Nie przepisujemy teraz frontendu.

Po tym zakresie pozostają: trwałe zamówienia i webhooki Stripe, konta/role, chat i email, logi/metryki do Grafany oraz publiczne wdrożenie. Istniejące sandboxy Stripe/Furgonetki zachowujemy; nie są gotową sprzedażą produkcyjną.

## Sposób pracy

KISS, minimum kodu i zależności. Workerzy implementują niezależne moduły; główny agent integruje. Krótki build i sprawdzenie zmienionej funkcji, bez Astry i szerokich testów przy każdej zmianie. Nie omijamy izolacji, autoryzacji ani kontroli kwot. Stan i dowody: [checkpoint](milestones/backend.md).
