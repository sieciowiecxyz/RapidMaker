# Checkpoint backendu

2026-10-23. Aktywny B3/B4; nowy plan zastępuje M0–M6. Historia starych opisów jest w Git. Baza przed zmianą: `ee4208a`.

## Zrobione

- B1 PASS: osobny crate `api`, `app` tylko uruchamia serwer i tryb workera. Zależności jednokierunkowe; dotychczasowe 9 testów API przechodzi.
- B2 PASS: PostgreSQL 16 na localhost:18434, SQLx/migracje, import 49 materiałów/314 kolorów. Powtórny import zachowuje koszt/stock/dopuszczenie; reconnect i niezmienność ID wyceny sprawdzone na prawdziwej bazie.
- B3 częściowo: izolowany Bambu Studio, STL, PLA Basic/PETG HF, P1P hardened 0,4 + Textured PEI + standard 0,20. Rzeczywisty cube20mm: PLA około 1263 s/3,72 g, PETG około 1465 s/3,67 g. Niedozwolony profil i błędny wynik są odrzucane; uszkodzony STL sprawdzony przez HTTP.
- B4 częściowo: koszt netto/VAT/brutto z czasu i masy; cena zakupu/rates jawne; przygotowanie raz, obsługa/czas na sztukę. Cache pomiaru 64 wpisy/10 min, zapis snapshotu w DB. Każda sztuka na osobnej płycie, bez optymalizacji partii.

## Dowody

- PASS build i Clippy zmienionych crate’ów; 13 testów jednostkowych (API, wycena, katalog i parser wyniku slicera). Workerzy: unit/import/reconnect PostgreSQL oraz prawdziwy sandbox slicera dla obu materiałów.
- PASS HTTP na osobnej tymczasowej bazie: katalog 49/314 bez cen zakupu, brak dopuszczenia → 422, STL → pomiar/cena/zapis, uszkodzony STL → 422, cztery wyceny pozostają po restarcie API.
- Stawki WYŁĄCZNIE testowe, koszt zakupu 80 zł netto/kg: cube20mm dla 1/2/10/100 sztuk dał 19,53/22,85/49,49/349,14 zł brutto. To test arytmetyki i przebiegu, nie zatwierdzony cennik ani dowód rentowności. Tymczasowa baza została usunięta.
- Normalny API uruchomiony ponownie z `.env`/PostgreSQL. Nie zapisano fikcyjnych cen, stanów ani stawek produkcyjnych; nowa wycena bez nich zwraca ręczną ocenę/503.
- NOT RUN: produkcyjna kalibracja, frontend/checkout nowego silnika, screenshoty (UI bez zmian), pełny pentest/E2E.

Rust LOC: 3830 → 4749 (+919), bez pustych linii i komentarzy, łącznie z testami inline. Nowy crate `api`; SQLx dla PostgreSQL i SHA-256 dla cache.

## Dalej

1. Bezpieczna siatka produkcyjna STEP/3MF i zachowanie modeli/zadań; analiza geometrii działa już osobno.
2. Rzeczywiste partie/wspólne płyty i kontrolowane opcje druku; kolejne zatwierdzone profile.
3. Konfiguracja operatora i autoryzowany zapis admina, frontend i checkout nowej kwoty (B5).
4. Potrzebne rzeczywiste ceny zakupu oraz zatwierdzenie kosztów i sprzętu; nie zastępować ich domyślnymi wartościami slicera.

Niezależne dopiski użytkownika w `AGENTS.md` oraz nieśledzony `result.json` zachować.
