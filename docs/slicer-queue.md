# Kolejka slicera i pliki

Modele zostają w prywatnych plikach lokalnych; PostgreSQL przechowuje metadane, własność, zadania i wyniki. Bez nowego brokera ani magazynu obiektowego.

## Przepływ

1. `POST /api/models` zapisuje oryginał, kanoniczną siatkę i podgląd GLB po izolowanej analizie.
2. `POST /api/quotes/estimate` sprawdza właściciela, opcje, dopuszczenie materiału, kolor i cennik. Zapisuje snapshot wejścia i zwraca **202**: `{id,status,poll_url,result,message,expires_at}`.
3. `GET /api/quotes/jobs/{id}` zwraca ten sam kształt tylko właścicielowi; obcy dostaje 404. Status: `queued`, `running`, `succeeded`, `failed`. Wynik jest dostępny przy `succeeded`; komunikat błędu przy `failed`. Ścieżki, dane właściciela i token lease nie trafiają do odpowiedzi.
4. Worker przejmuje zadanie, uruchamia izolowany Bambu Studio i oblicza cenę. Snapshot wyceny i sukces zadania zapisuje w jednej transakcji. ID zadania jest też ID wyceny dla checkoutu.

Frontend odpytuje co sekundę, maks. 10 min, respektuje anulowanie. Przerwanie odpytywania nie kasuje zadania; powtórny identyczny POST zwraca istniejące aktywne zadanie lub jeszcze ważny wynik. Fingerprint obejmuje model, materiał/kolor, opcje, profile, cenę zakupu i stawki; deduplikacja jest prywatna dla właściciela. Poprzedni cache pomiarów w RAM został zastąpiony deduplikacją zadań w DB.

## Wykonanie i granice

- Jedno aktywne zadanie globalnie, także przy kilku procesach. Krótka transakcja/advisory lock oraz `FOR UPDATE SKIP LOCKED`; transakcja nie trwa podczas slicowania.
- Lease 180 s, twardy timeout zadania 150 s. Zapis sukcesu/błędu wymaga aktualnego tokena i niewygasłego lease. Stary worker nie nadpisze nowej próby.
- Maks. 3 próby. Błędy chwilowe wracają do kolejki z opóźnieniem 5/10 s; błędne wejście lub zmieniona konfiguracja kończą zadanie. Utracony worker jest odzyskiwany po wygaśnięciu lease.
- Zadanie czeka maks. 30 min. Limity: 4 aktywne/właściciela, 64 globalnie, 1000 zachowanych wpisów. Pełna kolejka daje 429. Wygasłe wpisy są usuwane przy przyjmowaniu kolejnych zadań. Wynik wyceny jest ważny 10 min od ukończenia.
- Wykonanie może się powtórzyć po awarii, lecz tylko aktualny lease może zatwierdzić wynik. Nie obiecujemy wykonania dokładnie raz.

## Pliki

`MODEL_STORAGE_DIR`, domyślnie `~/.local/state/rapidmaker/models`: katalog 0700, pliki 0600, nazwy katalogów to serwerowe UUID. HTTP zawsze sprawdza własność. Limit uploadu: domyślnie 32 MiB; lokalny profil large 96 MiB ([budżety](plans/large-models.md)). Limity oryginału, siatki i podglądu łącznie: 20 modeli/256 MiB na właściciela; 1000 modeli/8 GiB globalnie.

Odbieranie body uploadu trwa maks. 60 s (408); limit nie obejmuje zapisu do bazy, aby nie przerwać commit. Zajęty slot uploadu daje 429. Prywatne źródło/GLB/siatka zamówienia są streamowane po 64 KiB; maks. cztery aktywne transfery na proces, pozostałe dostają 503. Slot jest zwalniany przy EOF lub porzuceniu odpowiedzi. Nagłówki prywatności i kontrola właściciela/admina pozostają obowiązkowe.

Model roboczy żyje 24 h. Przyjęcie zadania zapewnia co najmniej 30 min dostępności modelu; sukces co najmniej ważność wyceny. Aktywne zadanie chroni pliki przed sprzątaniem. Worker sprząta co minutę, upload również uruchamia sprzątanie. DB oznacza wygasły model jako usuwany przed usunięciem plików: nowe zadanie nie może go przejąć. Nieudane usunięcie jest ponawiane, plik nadal liczy się do limitu. Claim gościa przenosi także zadania; działający worker korzysta z aktualnej własności.

Pliki nowych zamówień mają osobną trwałą ochronę przed płatnością, przez realizację i 90 dni po zakończeniu. Szczegóły: [pliki zamówień](../backend/README.md#pliki-zamówień). Sprzątanie osieroconych katalogów po twardej awarii pozostaje osobnym zakresem. Kolejka nie zastępuje backupu plików i DB.

## Uruchamianie

Domyślnie `python backend/run.py` uruchamia API i workera w jednym procesie. Żądania HTTP jedynie zapisują/odczytują zadania.

Osobny proces: API uruchom z `SLICER_WORKER_MODE=external python backend/run.py`, a workera z `python backend/run.py --slicer-worker`. Oba procesy wymagają tego samego PostgreSQL i prywatnego `MODEL_STORAGE_DIR`; obecnie jeden host/wspólny dysk. Worker nie otwiera portu HTTP. SIGINT/SIGTERM kończy bieżące zadanie przed wyjściem; twarda awaria pozostawia zadanie do odzyskania.

## Dowody

Izolowany PostgreSQL: równoległe przejęcie, odzyskanie lease i odrzucenie starego workera, retry, deduplikacja/limity, atomowy snapshot, claim w trakcie pracy i ochrona sprzątania — PASS. Rzeczywisty HTTP/Bambu: szybki 202, odmowa obcemu, restart API, zabicie procesu workera i wznowienie po wymuszonym wygaśnięciu lease, pojedynczy snapshot oraz checkout z ceną serwera — PASS. Test używał osobnej bazy/katalogu i stawek testowych. Nie zmieniał katalogu cen normalnego środowiska ani nie wykonywał płatności/nadania.

Build Rust, fmt, Clippy all-targets, 16 testów jednostkowych API/storage oraz svelte-check/Vite build — PASS. Regresja kolejki na izolowanym PG — PASS; pozostałych 6 ignorowanych testów PG nie powtarzano w tym kroku. Rust backend/crates: 10439 → 11156 (+717) linii kodu razem z testami inline, bez komentarzy/pustych/generowanych plików. Nowe pliki produkcyjne: worker API, moduł storage i migracja; bez nowych zależności.

## Przygotowanie do slicera

Nowe STL mają normalne zgodne z kolejnością wierzchołków. Starsze uploady są poprawiane w prywatnej kopii slicera: kontrola binarnego formatu/rozmiaru/liczb, normalne i przesunięcie na Z=0. Oryginał i GLB nie zmieniają się. Bambu wykonuje `--orient 1 --ensure-on-bed --arrange 1 --allow-rotations`; ograniczenia stołu i finalny wynik slicera pozostają wiążące. Zmiana profilu `orient1-fit2` unieważnia poprzednie wyceny.

## Tożsamość runtime

Zadanie i snapshot wyceny zawierają SHA-256 całego drzewa `/opt/bambustudio-bin`: launchera `AppRun`, `bin/bambu-studio`, zasobów, profili i bibliotek dostarczonych wewnątrz tego katalogu. Hash obejmuje posortowane względne ścieżki, typy i uprawnienia wpisów oraz zawartość plików z prefiksami długości. Nie zależy od ścieżki katalogu głównego ani kolejności utworzenia wpisów. Wchodzi do klucza deduplikacji; worker i checkout odrzucają inną lub brakującą tożsamość. Starsze wyceny wymagają przeliczenia.

Hash jest liczony raz na proces. API z bazą inicjalizuje go przed obsługą HTTP, więc zimny odczyt pakietu wydłuża start, zamiast pierwszego zlecenia klienta. Brak działającego slicera zapisuje ostrzeżenie i blokuje wyceny; pozostałe API może działać. Osobny worker inicjalizuje hash przy pierwszym użyciu. Hashowanie i skanowanie działają w `spawn_blocking`, poza zadaniami async HTTP. Odczyt sprawdza metadane przed otwarciem, na otwartym pliku i po odczycie oraz porównuje całe drzewo przed/po hashowaniu. Każde użycie oraz kontrola przed/po slicingu ponownie skanują ścieżki, typy i metadane całego drzewa, bez ponownego odczytu zawartości. To koszt O(liczby wpisów) operacji metadanych i sortowania O(n log n), również dla trafienia deduplikacji; zimny start dodatkowo czyta wszystkie bajty bundla. Nie zmierzono tu opóźnienia rzeczywistego bundla pod obciążeniem.

Limity: 16 384 wpisy łącznie z katalogami i korzeniem, 512 MiB na plik, 1 GiB sumy plików, 16 poziomów poniżej korzenia, 4096 bajtów ścieżki względnej. Skanowanie nakłada limity przed dodaniem wpisu i dalszym zejściem, bez nieograniczonego zbierania zawartości katalogu. Brak wymaganych zwykłych wykonywalnych plików `AppRun`/`bin/bambu-studio`, dowiązanie symboliczne (także korzenia), plik specjalny, błąd odczytu lub przekroczenie limitu blokują tożsamość.

Po zmianie bundla zrestartuj API i osobne workery. Wykryta zmiana blokuje kolejne kontrole w procesie do restartu, także po przywróceniu pliku. Kontrole już w toku nie są atomową blokadą wdrożenia. Mechanizm nie obejmuje systemowych bibliotek i innych zależności spoza bundla (np. `/usr`), nie zastępuje niezmiennego wdrożenia ani nie potwierdza jego autentyczności. Kontrole metadanych nie chronią przed celową podmianą podczas skanowania lub wykonania; bundle powinien pozostawać niezmienny przez czas działania procesu.
