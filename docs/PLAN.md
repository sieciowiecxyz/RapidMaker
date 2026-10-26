# Plan: integracja frontendu Piotra — PR #4

Cel: zachować zatwierdzony wygląd i interakcje PR, połączyć je z rzeczywistym backendem i zmergować #4 do main. Bez fikcyjnych cen, płatności i zapisów. Dotychczasowe E2E: [raport](E2E-REPORT.md).

1. Połączyć aktualne main z PR w osobnym worktree; zachować lokalne AGENTS.md i result.json użytkownika.
2. Stałe PLA/PETG/ASA/PA-CF/TPU, czarny i paleta 178 PLA pozostają lokalnym UI. Adapter wybiera rzeczywisty profil/kolor; cena wyłącznie z serwera. Brak ceny/dopuszczenia lub inna preferencja koloru oznacza rzeczywistą ręczną ofertę, nie najbliższy zamiennik.
3. Lokalny STL działa także bez API; upload/geometria i prywatny GLB dla STEP/3MF korzystają z backendu. Zachować koło, przekrój, dyszę, 10 modeli przykładowych i oba motywy.
4. Podłączyć autorski formularz inline do danych/dostawy/Stripe bez ponownego wpisywania danych i bez lokalnego sukcesu zamówienia. Zachować sesje, CSRF, autoryzację oraz wznowienie płatności.
5. Uzupełnić backend i rzeczywisty panel o notatki, termin, postęp części, etapy wewnętrzne/kontrolę jakości i szkic faktury. Płatności potwierdza Stripe; szkic nie jest wystawioną fakturą. Cofnięcie etapu nie cofa zapłaty ani nadania.
6. Celowane Rust/TypeScript i rzeczywiste E2E zmienionych przepływów oraz wygląd desktop/mobile. Uaktualnić kontrakt/mapę, raport wyników, push do gałęzi PR, merge na GitHub i bezpiecznie zaktualizować lokalne main.

Kryteria: [MILESTONES.md](MILESTONES.md). Bez pełnego audytu każdej iteracji; błędy produktu naprawiamy, oczekiwania testów zmieniamy wyłącznie dla świadomie przyjętego nowego kontraktu UI.
