# Cel: rzeczywiste modele i wiarygodne E2E

Przeczytaj gdy: wznawiasz goal lub oceniasz jego zamknięcie.
Opis: kryteria, aktualny checkpoint i dalsza praca; historia w [raporcie](../corpus-report.md).

## Wymagania

- 41 prywatnych projektów MakerWorld z pochodzeniem/SHA (34×3MF, 7×STL); użytkownik zakończył pobieranie. STEP z fixtures.
- Rzeczywisty upload, prywatny podgląd, slicer i niezależny rachunek ceny. Ręczna ścieżka/odmowa nie oznacza automatycznej wyceny lub obsługi geometrii.
- Obrazy, opcje i interakcje Piotra bez redesignu: konfigurator, zakup/refundy, dostawa, klient/admin, MFA, chat i rzeczywisty email. [Mapa dowodów](../e2e-coverage.md), [kontrolki](../e2e-controls.md).
- Minimalny Rust, proste API/KISS; zachować kwoty, własność, izolację i limity. Nie zmieniać oczekiwań dla zielonego raportu.
- Po etapie Sol xhigh w dziesięciu kategoriach: poprawność, bezpieczeństwo, kwoty, zasoby, błędy, współbieżność, dowody, prostota, utrzymanie, wygląd. Naprawiać konkretne problemy, nie wymuszać 10/10.

## Checkpoint

| Zakres | Zweryfikowany stan |
| --- | --- |
| Korpus/API | corpus41-large-current: 41 scenariuszy; 24 native ceny, 12 ręcznych, 4 wadliwe, 1 limit; 36 GLB. corpus12-manual-geometry osobno uzasadnia ręczne ścieżki |
| Korpus/UI | 37 PASS/4 FAIL harnessu, poprawione 4 PASS osobno; 144 unikalne ujęcia 1440/390 px i oba motywy obejrzane. Nie pojedynczy czysty przebieg 41 |
| Większe modele | Bezstratny kompaktowy/spatial GLB i jawne uproszczenie wyłącznie podglądu z zachowaniem pełnego STL. [Budżety i dowody](large-models.md). Zespół 41 nadal ponad limitem |
| Pełny zestaw 78d5802 | 136 PASS/0 FAIL/0 flaky/1 SKIP w siedmiu osobnych grupach. Kod wrappera 2 przez pominięcie opt-in large; ten scenariusz osobno 1 PASS. 312 hashów przed/po zgodnych |
| Naprawione przepływy | Tożsamość line_id/model_id/quote_id, zakup gościa → konto, PI decline/leave/resume. Dwa warianty jednego uploadu z prawidłowymi kwotami w obu panelach; duplikacja szkicu to jawny szew, brak przycisku powielania |
| Osiem klientów | eight-clients-final: 1 PASS/3,8 s. 16 równoczesnych estimate → 8 zadań/8 snapshotów/max prób 1; własne ceny i odmowa obcych zasobów; 4 próbki responsywności, nie szeroki load test |
| Localhost | Gałąź robocza na 18430/18432, large tylko w środowisku procesu. .env i zmiany głównego checkoutu bez edycji; backup przed migracjami. Model 8: 78,4 MB → podgląd → DELETE 204 |

## Dalsza praca i granice

1. Ulepszać konkretnie wskazane problemy, nie uruchamiać ponownie pełnych zestawów bez nowej przyczyny. Po zmianie tylko potrzebna regresja i scoped review.
2. Sprawdzić prostotę i zbędne zależności kodu; pozostałe słabiej udowodnione obszary to szersze obciążenie i zużycie zasobów. Osiem klientów nie wyznacza przepustowości produkcyjnej.
3. Jawne ograniczenia: wadliwe/ponadlimitowe źródła, szew duplikacji, koszty fixture zamiast rentowności firmy, wirtualny passkey i emulowany viewport. Cały goal nadal otwarty; review nie potwierdziło 10/10.

## Uruchamianie

```sh
python frontend/tests/run-all.py --group core --no-build
E2E_REPORT_NAME=eight-clients-current python frontend/tests/run-integration.py concurrency.spec.ts
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-large-current python frontend/tests/run-integration.py --large-models corpus.spec.ts
```

Używaj nowej nazwy raportu, by zachować wcześniejsze wyniki. Konfiguracja w backend/README.md; sekrety poza raportami. Pełne zestawy i testy providerów tylko celowo. Historia etapów, błędów harnessu i review jest w corpus-report.md; starsze checkpointy także w Git.
