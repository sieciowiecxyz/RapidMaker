# Kontekst dokumentacji

Czytaj [indeks](INDEX.md) i tylko pliki potrzebne do zadania. `PLAN.md` opisuje wizję, zakres i potwierdzony stan produktu; `MILESTONES.md` — kolejność i kryteria; `ARCHITECTURE.md` — technologię, granice bezpieczeństwa i mapę modułów. `AGENTS.md` ustala sposób pracy. Nie twórz drugiego źródła postępu.

Tryb normalny (`Active: none`) oznacza, że realizujemy tylko bieżącą prośbę. Podczas pracy nad etapem czytaj jego kryteria i wyłącznie aktywny checkpoint w `docs/milestones/`. Po utracie kontekstu odczytaj te pliki oraz stan Git i kodu.

Zapisuj krótkie decyzje, ich powód, dowody i ograniczenia. Oddzielaj fakty od zamiarów. Checkpoint zawiera wynik, commit, testy `PASS`/`FAIL`/`NOT RUN`, blokery i następny krok — bez dużych logów i kopii planu. Aktualizuj właściwy plik, gdy zmieni się kontrakt, zachowanie lub architektura. Każdy wpis indeksu ma krótkie pola `przeczytaj gdy:` i `opis:`. Używaj prostego języka i linków zamiast powtórzeń.
