# Workflow

Obecnie finalizacja według PLAN/MILESTONES: implementacja i test całego przepływu. Krótkie regresje w iteracjach, rzeczywiste E2E/integracje przy zamknięciu etapu zgodnie z bieżącą prośbą użytkownika.

Czytaj `PLAN.md`, `MILESTONES.md` i tylko potrzebne fragmenty kodu/docs. Sprawdź Git; chroń cudze zmiany.

Główny agent koordynuje workerów `gpt-6.1-sol` medium do niezależnej implementacji, z rozłącznymi plikami. Bez obowiązkowych testerów i review Astrą. Nie mnoż agentów ani rund komunikacji, jeśli spowalnia to prostą zmianę.

Dostarcz mały działający fragment. Sprawdź tylko zmienioną logikę albo jeden przebieg w UI zgodnie z [testing.md](testing.md). Po wyniku pozytywnym kontynuuj; bez kolejnych audytów, pełnych zestawów i powtarzania tej samej kontroli. Po błędzie popraw i sprawdź konkretną poprawkę.

Krótko aktualizuj kontrakt i postęp, przejrzyj diff, commituj i pushuj bez force-push. Docs-garden według potrzeby, bez obowiązkowego osobnego workera. Sprzeczność wymagań nazwij przed zależną zmianą.
