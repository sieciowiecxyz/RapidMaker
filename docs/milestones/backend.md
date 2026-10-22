# Checkpoint backendu

2026-10-23. Aktywny B5. Baza tego kroku: `573f1af`; stary plan M0–M6 pozostaje w historii Git.

## Stan

- B1/B2 PASS: osobny crate API; PostgreSQL/SQLx, katalog 49 materiałów i 314 kolorów, trwałe snapshoty wycen.
- B3 PASS POC: STL/STEP/STP/3MF → kanoniczny STL w izolowanym workerze → izolowany Bambu Studio. STEP wymaga pojedynczej części w mm; 3MF zachowuje jednostki/transformacje i pomija ustawienia/G-code klienta.
- B4 PASS POC: rzeczywiste układanie kopii, pomiar pełnej płyty, powtarzanie identycznych płyt i osobny pomiar reszty. Ilość 1–1000, draft/standard/fine, 2–8 ścian, wypełnienie 5–100%. Koszt opiera się na zmierzonych sumach i liczbie płyt, przygotowanie raz.
- Profile materiałów wybierane z zaufanego snapshotu: 41 ma profil P1P 0,4 mm po wykluczeniu podpór. Nie oznacza to fizycznej kwalifikacji wszystkich materiałów; cena, magazyn i zatwierdzenie sprzętu są obowiązkiem operatora. Faktycznie uruchomiono PLA Basic/PETG HF/ABS.
- B5 częściowo: chronione Bearer API operatora tworzy/edytuje materiały, kolory i wersjonowane stawki. Porównanie sekretu w stałym czasie. Import zachowuje zmiany operatora; cennik PostgreSQL ma pierwszeństwo przed `.env` bez restartu.

## Dowody

- PASS build, Clippy zmienionych crate’ów i fmt. Testy API/pricing/storage przechodzą; geometria 14 testów; rzeczywisty slicer potwierdził ustawienia i partie, w tym 1000 kopii na 9 płytach (8 × 120 + 40), dwa uruchomienia CLI.
- PASS HTTP na osobnej tymczasowej bazie: STL, dwa STEP, 3MF oraz transformowany model w cm. Dodane ustawienia i G-code w 3MF nie zmieniły pomiaru. Uszkodzony STL, ZIP traversal, niedozwolone jednostki STEP i infill101% są odrzucane.
- PASS API operatora: brak/błędny token → 401; dodawanie materiału/koloru, edycja kosztu i magazynu; błędny kolor → 400; zmiana ceny bez nowej wersji → 409. Nowy cennik zmienia nowe wyceny, nie poprzednie snapshoty.
- PASS restart/import: 12 wycen, własny materiał i zmiany operatora przetrwały restart; ceny zakupu są ukryte w publicznym katalogu.
- Przykład WYŁĄCZNIE na stawkach testowych, koszt zakupu 80 zł netto/kg: cube20mm 1/2/10/100 sztuk około 19,53/20,10/25,98/96 zł brutto; wszystkie te partie mieszczą się na jednej płycie. Układanie slicera może dać nieco inny czas przy nowym przebiegu; wydana wycena pozostaje snapshotem. To nie zatwierdzony cennik ani dowód rentowności. Tymczasowa baza została usunięta.
- Normalny API restartowany z `.env` i PostgreSQL. Losowy sekret operatora jest lokalnie w `.env`/SOPS, bez ujawniania wartości. Nie zapisano fikcyjnych cen, magazynu ani cennika produkcyjnego.
- NOT RUN: kalibracja rzeczywistych wydruków, całościowy pentest, frontend nowej wyceny i checkout; UI nie zmieniano.

Rust LOC: 4749 → 5555 (+806), łącznie z testami inline, bez komentarzy i pustych linii. Baseline zmierzony z `git archive HEAD`. Jedyna nowa bezpośrednia zależność: `subtle` do porównania sekretu w stałym czasie (już obecna w lockfile).

## Dalej — B5

1. Sesja operatora i formularze panelu bez sekretu w bundle/localStorage; browser mock nigdy nie autoryzuje zapisu DB.
2. Konfigurator korzysta z katalogu i nowego silnika zamiast starych mocków/taryfy; przechowuje pliki prywatnie z jasną własnością i limitami.
3. Wynik wyceny wiąże ilość/ustawienia/model z właściwą masą/wymiarami do pakowania, ofertą Furgonetki i kwotą Stripe. Zmiana koszyka unieważnia ofertę.
4. Potrzebne rzeczywiste ceny zakupu i zatwierdzenie kosztów/sprzętu przed sprzedażą; nie zastępować ich domyślnym kosztem slicera.

Niezależne dopiski użytkownika w `AGENTS.md` i nieśledzony `result.json` zachować.
