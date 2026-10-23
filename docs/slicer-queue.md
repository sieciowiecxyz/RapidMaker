# Kolejka slicera i pliki

Modele zostają w prywatnych plikach lokalnych; PostgreSQL przechowuje metadane, własność, zadania i wyniki. Bez nowego brokera ani magazynu obiektowego.

## Przepływ

1. `POST /api/models` zapisuje oryginał i kanoniczną siatkę po izolowanej analizie.
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

`MODEL_STORAGE_DIR`, domyślnie `~/.local/state/rapidmaker/models`: katalog 0700, pliki 0600, nazwy katalogów to serwerowe UUID. HTTP zawsze sprawdza własność. Limit uploadu: 32 MiB. Limity oryginału i siatki łącznie: 20 modeli/256 MiB na właściciela; 1000 modeli/8 GiB globalnie.

Model roboczy żyje 24 h. Przyjęcie zadania zapewnia co najmniej 30 min dostępności modelu; sukces co najmniej ważność wyceny. Aktywne zadanie chroni pliki przed sprzątaniem. Worker sprząta co minutę, upload również uruchamia sprzątanie. DB oznacza wygasły model jako usuwany przed usunięciem plików: nowe zadanie nie może go przejąć. Nieudane usunięcie jest ponawiane, plik nadal liczy się do limitu. Claim gościa przenosi także zadania; działający worker korzysta z aktualnej własności.

Retencja plików opłaconych zamówień i sprzątanie osieroconych katalogów po twardej awarii pozostają osobnym zakresem. Kolejka nie zastępuje backupu plików i DB.

## Uruchamianie

Domyślnie `python backend/run.py` uruchamia API i workera w jednym procesie. Żądania HTTP jedynie zapisują/odczytują zadania.

Osobny proces: API uruchom z `SLICER_WORKER_MODE=external python backend/run.py`, a workera z `python backend/run.py --slicer-worker`. Oba procesy wymagają tego samego PostgreSQL i prywatnego `MODEL_STORAGE_DIR`; obecnie jeden host/wspólny dysk. Worker nie otwiera portu HTTP. SIGINT/SIGTERM kończy bieżące zadanie przed wyjściem; twarda awaria pozostawia zadanie do odzyskania.

## Dowody

Izolowany PostgreSQL: równoległe przejęcie, odzyskanie lease i odrzucenie starego workera, retry, deduplikacja/limity, atomowy snapshot, claim w trakcie pracy i ochrona sprzątania — PASS. Rzeczywisty HTTP/Bambu: szybki 202, odmowa obcemu, restart API, zabicie procesu workera i wznowienie po wymuszonym wygaśnięciu lease, pojedynczy snapshot oraz checkout z ceną serwera — PASS. Test używał osobnej bazy/katalogu i stawek testowych. Nie zmieniał katalogu cen normalnego środowiska ani nie wykonywał płatności/nadania.

Build Rust, fmt, Clippy all-targets, 16 testów jednostkowych API/storage oraz svelte-check/Vite build — PASS. Regresja kolejki na izolowanym PG — PASS; pozostałych 6 ignorowanych testów PG nie powtarzano w tym kroku. Rust backend/crates: 10439 → 11156 (+717) linii kodu razem z testami inline, bez komentarzy/pustych/generowanych plików. Nowe pliki produkcyjne: worker API, moduł storage i migracja; bez nowych zależności.
