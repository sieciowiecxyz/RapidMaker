# Większe modele — aktywny plan

Przeczytaj gdy: rozwijasz duże uploady i prywatne podglądy.
Opis: aktualne granice, dowody i następne kroki; szczegóły w [raporcie](../corpus-report.md).

## Cel i decyzje
- Pełna geometria STL/STEP/3MF dla slicera; ograniczony, poprawny podgląd. Nie liczymy413/422 jako obsługi modelu ani błędu konwersji jako udanej ręcznej wyceny.
- Zachować wygląd Piotra, własność, kwoty dysku, izolację, holdy zamówień i dokładne kwoty płatności. Cały cel stron/auth/email/wycen pozostaje w [corpus-quality.md](corpus-quality.md).
- Profil `large` jest przygotowany/testowany, **niewłączony na serwerze użytkownika**; wymaga jawnego `AUTH_MODE=localhost`. Pytanie o budżet/default nadal bez odpowiedzi. `.env` nie zmieniamy bez niej.
- 41 projektów wystarcza; bez nowych pobrań. STEP z fixtures. Dane/zrzuty prywatne i ignorowane.

## Budżety
| Limit | standard | large — tylko opt-in |
| --- | ---: | ---: |
| API/klient upload |32MiB|96MiB|
| ZIP łącznie/na wpis; liczba |96MiB;512|512MiB;512|
| Kanoniczny STL / unikalne vertices |50MiB/1M|200MiB/4M|
| Trójkąty z budżetu STL |1 048 574|4 194 302|
| GLB / renderer |25MiB/2Mtri|25MiB/2Mtri|
| Proces geometrii |2GiB/25CPU/30wall|bez zmian|
| Bambu na partię |4GiB/60CPU/65wall|4GiB/120CPU/125wall|

Całe zadanie150s/lease180s; dwie duże partie mogą nie zmieścić się w deadline. Kwoty20modeli/256MiB na właściciela i1000/8GiB globalnie obejmują oryginał/STL/GLB. Lokalny STL nadal25MiB/1Mtri; błąd metadata API daje awaryjne25MiB. Wierzchołki renderera mogą zająć144MB +72MB pozycji tymczasowych +GLB/kopia trim/GPU; viewport390px nie dowodzi działania fizycznego telefonu.

## Etapy
1. **Diagnoza — wykonana:** dziewięć dużych przypadków zmierzonych w izolacji, źródłaSHA.14/21/37 mają potwierdzoną wadę topologii,32 niepoprawny pakiet; nie obchodzimy walidatora.41 nadal zatrzymuje pełna walidacja SDK przy25CPU; przyczyna SIGKILL nieoznaczona jako OOM.
2. **Konwersja — częściowo:** kompaktowe u16 i backendowy profil;29 odzyskany przestrzenną kolejnością samych trójkątów, bez zmiany f32/winding/liczności, bez mutacji STL. Dotychczasowy pasujący zapis pozostaje bajtowo identyczny. Sort tylko po porażce obu dotychczasowych zapisów; minimalny rozmiar referencjonowanych pozycji+indeksów pomija niemożliwe próby. Nadal64bloki/64KiBJSON/25MiBGLB.
3. **API/UI — częściowo:**8/20/29/40 mają rzeczywisty upload/privateGLB/render/zoom w1440/390 i obu motywach.20/29 również rzeczywisty slicer/koszyk/cenę UI na jawnych stawkach fixture; nie jest to dowód produkcyjnej rentowności. Dla29 dokładnie porównano każdy bit wszystkich1 993 410 zorientowanych trójkątów z kanonicznym STL, także powtórzenia.
4. **Usuwanie — wykonane w sprawdzonym zakresie:**10E2E17,5s/22DB+API/3zwykłe scenariusze20,9s. Chronione DELETE:204 po fizycznym usunięciu,404 cudzy/brak,409 runninglease/livehold; dokładnyOrigin/CSRF, trwały mark/lockkolejki, retry po błędzie. Sprzątane spóźnione zaakceptowane odpowiedzi i błędy advanced po201. Token gościa przy koncie dotyczy tylko nieprzejętego rekordu, bez niejawnego claimu. Nieznana odpowiedź/zamknięta strona nadal24hretencji; retry ID w pamięci strony.
5. **Audyt/integracja — otwarte:** Solxhigh w10kategoriach po zmianie, konkretne uwagi naprawiane; bez wymuszania10/10. Brak pełnego nowego korpusu/core/providerów po każdej zmianie. Nie sumować osobnych przebiegów w jedenPASS.

## Bieżące dowody modelu29
- `large-model-probe-spatial-minimum/results.json`:29 GLB24 171 984B zamiast29 224 268B;13,975swall/~449MiBRSS, te same996139vertices/1993410tri/wymiary.19 nadalOutputTooLarge11,497s; sam minimalny pełny zapis przekracza25MiB. Program diagnostyczny nie generuje STL ani transformu metrów/Y-up.
- `spatial29-exact-api-price.json`:1PASS46,1s — pełny multizbiór bitów zorientowanych faces/STL i niezależny rachunek fixture; przed dodaniem wyłącznie dolnej granicy rozmiaru. `spatial29-ui-price.json`:1PASS72,2s po tej kontroli;1993410 widocznych tri/zeroArea0,4ujęcia obejrzane, UI4178gr =koszyk serwera. Wcześniejszy osobny pomiar4180gr nie jest stałą oczekiwaną ceną.
- `spatial-default-models.json`:8PASS7,9s; geometry42PASS/1opcjonalnyignored; app/example build, Clippygeometry/APIall-targets, fmt/diff PASS. `spatial29-final-exact-api.json`: 1 PASS, 46,1 s na końcowym kodzie — pełne porównanie geometrii i wycena 4178 gr.

## Następne kroki
1. Model 19: wybrać ograniczoną reprezentację podglądu po zmierzeniu pamięci/CPU/kodu; pełny zapis nie zmieści się w25MiB przez samą kolejność. Nie gubić geometrii dla produkcji ani maskować błędów ceną.
2. Model 41: ustalić ważność i koszt pełnej walidacji SDK bez wyłączania kontroli bezpieczeństwa; ewentualny inny profil diagnostyczny wyraźnie oddzielić od runtime/defaultu.
3. Dokończyć dowody nowych cen/ilości/zasobów i audyt całego celu. Weryfikować wybrane zmiany, bez powtarzania zielonych zestawów bez powodu.

Historyczny szczegółowy plan i poprzednie przebiegi: `cf060a3:docs/plans/large-models.md`; surowych modeli/raportów nie usunięto. Komendy:
```sh
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=spatial29-ui-price python frontend/tests/run-integration.py --large-models corpus-ui.spec.ts --grep 'corpus GUI 29:'
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=spatial29-final-exact-api python frontend/tests/run-integration.py --large-models large-profile.spec.ts --grep 'corpus 29:'
```

Review Sol xhigh: brak konkretnego blokera tej zmiany. Oceny /10: poprawność 9, bezpieczeństwo 9, kwoty 9, zasoby 8, błędy 9, współbieżność 9, dowody 9, prostota 8, utrzymanie 8, zgodność wizualna 9. Review bez ponownego uruchamiania testów; nie jest oceną całego celu.
