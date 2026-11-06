# Cel: rzeczywiste modele i wiarygodne E2E

Przeczytaj gdy: kontynuujesz cel lub oceniasz jego zamknięcie.
Opis: wymagania i następne kroki; wyniki/źródła w [raporcie](../corpus-report.md).

## Wymagania

- Zaakceptowane 41 różnych projektów MakerWorld, lokalnie/prywatnie z pochodzeniem i SHA. Użytkownik zakończył pobieranie; bez kolejnych CAPTCHA i szukania 100 plików. STEP z fixtures.
- Rzeczywiste uploady STL/STEP/3MF, prywatny podgląd, slicer, ilości i niezależny rachunek kosztów; jawne ograniczenia, bez udawanej ceny.
- Wszystkie renderowane obrazy/przykłady, desktop/mobile i oba motywy. Klikalne przepływy z [mapy kontrolek](../e2e-controls.md) wymagają działania, nie tylko obecności: konfigurator, płatność/refundy, dostawa, klient/admin, logowanie/MFA, chat i rzeczywiste emaile. Wygląd Piotra i istniejące interakcje są źródłem prawdy.
- Minimalny Rust/proste API/KISS; bez zbędnych warstw i usuwania kontroli kwot, własności lub zasobów dla zielonego raportu.
- Raport i audyt całego celu w 10 kategoriach; nie utożsamiać zielonej regresji lub oceny małej zmiany z kompletnym 10/10.

Większe pliki: [aktywny plan](large-models.md),9 przypadków wraz z pięcioma413. Diagnostyka:5 pełnych konwersji podglądu,3 potwierdzone odrzucenia topologii i1 sygnał przy granicyCPU; nie jest to nowa obsługa HTTP.

## Etapy i dowody

| Etap | Kryterium / aktualny stan |
| --- | --- |
| C1 Korpus | Zrobione: 41 unikalnych źródeł/hashów, manifest, ignorowane oryginały; 34×3MF/7×STL |
| C2 Modele/ceny | Częściowo: pełne 41 na 8bd6fcc, 36 PASS/5 FAIL/424,3 s =20 cen/11 ręcznych/5 oczekiwanych 413; FAIL:4 limity 422 +1 błędny pakiet 422 |
| C3 UI | Wykonane macierze w raporcie, bez dowodu wszystkich kombinacji/pikseli. SVG:6 PASS/55,2 s,56 kombinacji,246 powtarzanych kontroli i4 duże tła tylko geometrycznie; bez pełnej oceny kontrastu/tożsamości |
| C4 Konta/zakup | Realne MFA/ownership/email/Stripe test/Furgonetka sandbox/refundy/wznowienia; zakresy i dostawcy oddzielone w raporcie |
| C5 Zamknięcie | Otwarte: pozostałe modele/decyzja, luki dowodów całego wymagania i końcowy audyt |

Ostatni pełny `core.json` na242a935 po inicjalizacji runtime przed obsługą HTTP: **33 PASS/312,6 s,0 skipped/flaky**, bez Stripe/Furgonetki/iCloud. Poprzedni przebieg na30cd52a:32 PASS/1 FAIL/333,9s —pierwsze zlecenia6861ms>5000ms przez zimny hash. Bez poluzowania progu, po poprawce test1,5s. `contracts.json`: osobne historyczne **21 PASS/5,7 s** na97c10dd, kontrolowane UI/narzędzia; nie dowód dostawców.

Dalsze regresje: publikacja z dwóch sesji MFA 1 PASS/29,5 s; zerwane odpowiedzi + macierz ofert 2 PASS/29,3 s; stany ofert 1 PASS/25,5 s (72 PNG/28 nowych kart); hash pakietu/ceny 4 PASS/24,5 s. DB/API 20 PASS/0 skipped w osobnych bazach. Historyczny workspace na8bd6fcc: **139 PASS/25 ignored**; 20 pomijanych uruchomiono osobno, pozostałych 5 nie zaliczamy. Dostawcy i paid/refunded/email mają odrębne artefakty oraz źródła w raporcie.

## Granice i decyzje

- **Brak odpowiedzi użytkownika**: osobny profil dużych modeli czy pozostanie przy STL 50 MiB. Nie zakładać zgody na dalsze zwiększanie zasobów.
- Obecne limity: ZIP 96 MiB łącznie/na wpis i512 wpisów (rozszerzenie zaakceptowane); 1 mln unikalnych wierzchołków,1 048 574 trójkąty z budżetu STL 50 MiB, GLB25 MiB, API upload32 MiB/UI25 MiB, worker2 GiB/25 s CPU/30 s wall. Dev optymalizuje geometry/lib3mf/zlib-rs/quick-xml bez wyłączania sprawdzeń.
- Aktualnie 24 ma podgląd/ręczną `printer_fit`,39 cenę; historyczny exit137 modelu24 nie opisuje obecnego API. 14/19/20/40 potrzebują po rozwinięciu instancji 74–193 MB STL; nie ma nieużywanych trójkątów do usunięcia. 32 pozostaje nieprawidłowym pakietem; bez zgadywania namespace/obchodzenia walidatora. Preflight tylko przyspiesza odrzucenie.
- Tożsamość od dd24620 obejmuje całe drzewo pakietu Bambu, także zasoby/uprawnienia. Hashowanie przez `spawn_blocking`: zimno6,905 s,10 ciepłych kontroli101,117 ms; biblioteki systemowe poza pakietem nadal poza dowodem.
- Rachunek z testowych100 PLN netto/kg potwierdza arytmetykę, nie produkcyjną rentowność. Kwoty, auth, hold/retencja i limit sumy zwrotów pozostają obowiązkowe.
- Rzeczywisty SMTP tylko do void@sieciowiec.xyz. Outbox nie dowodzi doręczenia ani exactly-once SMTP; provider test/sandbox nie dowodzi fizycznego przewozu/produkcji/wszystkich przewoźników. Passkey wirtualny, fizyczny niesprawdzony; większe obciążenie niewykazane.
- Audyt: Rust0 podatności/1 notice `pin-utils` unmaintained; npm3 low z jednego niezałatanego upstream `deepmerge`, bez wyciszeń/forka: [security.md](../security.md).
- Modele/JSON/zrzuty prywatne i ignorowane, bez sekretów auth/MFA. Pełny chronologiczny raport zachowany w Git `30cd52a:docs/corpus-report.md`; nie kasować surowych dowodów/testów.

## Następne kroki i zamknięcie

1. Core33/33 zapisany; następne sprawdzenia kierować na zmiany lub wykazane luki. Bez powtarzania zielonych zestawów bez powodu i bez sumowania osobnych przebiegów w pełny PASS.
2. Po decyzji użytkownika ustalić politykę pozostałych dużych modeli. Zachować jawny FAIL wadliwego32; raportować limity i skutki dla UI zamiast udawać obsługę/cenę.
3. Uzupełniać wykazane luki całej mapy interakcji, grafik/przykładów i stanów/dialogów w obu motywach/rozmiarach, zachowując styl Piotra; nie ograniczać pierwotnego wymagania do istniejących testów.
4. Końcowy review GPT-6-Sol xhigh: poprawność, uprawnienia/bezpieczeństwo, kwoty, zasoby, błędy, współbieżność, dowody testowe, prostota, utrzymanie, zgodność wizualna/kontrakt. Naprawiać konkretne uwagi, bez wymuszania ocen. Cel otwarty przy nieudowodnionych wymaganiach lub ocenach poniżej żądanego poziomu.

Bieżąca regresja uploadów: `topology-models-ui-final.json`,6 PASS/5,5s. Naprawiono klasyfikację topologii3MF bez akceptowania wadliwej siatki; duże14/21/37 mają potwierdzony problem krawędzi,41 nadal ograniczenie CPU w diagnostyce. Szczegóły w planie większych modeli.

Etap bezstratnego kompaktowego GLB wdrożony; większy profil HTTP nadal oczekuje decyzji. Pięć diagnostycznych dużych modeli ma mniejsze podglądy, trzy mieszczą się w25 MiB. Nie uznajemy tego za nowe zaakceptowane uploady ani ceny. Szczegóły i aktualne testy: raport/plan dużych modeli.

Regresja kont: `pending-mfa-revocation.json`2 PASS/8,5s — realny formularz zmiany hasła odwołuje zarówno kopię sesji zalogowanej, jak niezależną sesję czekającą na2FA. Stare hasło odrzucone, nowe nadal wymaga drugiego składnika; pozostałe istniejące kontrole passkey/TOTP/recovery zachowane. Bez emaila/dostawców i bez zmian produktu.

Backendowy profil dużych modeli zaimplementowany, ale nie włączony domyślnie:3rzeczywiste duże uploady oraz cena modelu20 ze slicera. Frontend nadal wymaga osobnej obsługi limitów/podglądu; całego celu nie zamykamy. Nowa kontrola odczytu i admission po review, dowody i limity: plan dużych modeli.

Frontend dużego profilu:3rzeczywiste uploady/renderowanie w4wariantach; dodatkowo cena20 przez formularz zgodna z serwerem.31kontrolowanych kontraktów oraz regresja standardu z poprawionym starym oczekiwaniem25MB; dokładne przebiegi w planie dużych modeli. Następny konkretny problem: usunięty lokalnie model nadal obciąża kwotę serwera; potrzebne bezpieczne usuwanie bez naruszania kolejki/plików zamówień. Cel nadal otwarty.

M2 usuwania wdrożony:10 realnych E2E/22DB+API/3 zwykłe scenariusze PASS w osobnych przebiegach. Zwalnianie kwoty, spóźnione odpowiedzi, Origin/CSRF i ochrona jobów/zamówień sprawdzone; ID retry tylko w pamięci strony, nieznane odpowiedzi nadal24h retencji. Review bez kolejnego blokera tej zmiany; oceny nie10/10. Dalej19/29/41 oraz nieudowodnione wymagania całego celu; nie zamykamy celu.
