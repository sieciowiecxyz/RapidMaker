# Modele i E2E — checkpoint 2026-11-05

Cel nadal aktywny. Pobrano **41/100** różnych projektów MakerWorld: 34 × 3MF, 7 × STL; bez STEP z MakerWorld. Oryginały, warianty nazw i manifest SHA-256 pozostają lokalnie w ignorowanym `backend/test-models/makerworld/`. Licencje nie są w pełni zweryfikowane; nie publikujemy plików.

## Rzeczywisty korpus

`corpus41-after-fixes`: 41 przypadków, 152 s, bez pominięć.

| Wynik | Liczba | Co potwierdza |
| --- | ---: | --- |
| Automatyczna cena | 13 | Upload, GLB, prawdziwy slicer, dodatnie pomiary i całkowitoliczbowe koszty |
| Ręczna wycena | 5 | Podgląd działa; gabaryt przekracza politykę automatycznej wyceny/P1P |
| Za duży upload | 5 | Prawidłowe HTTP 413 powyżej 32 MiB |
| Limit geometrii | 18 | HTTP 422 `model_resource_limit`; pełna wycena nie została wykonana |

Runner zwrócił **23 PASS / 18 FAIL**, nie „41 działa”. Osiemnaście testów nadal oczekuje automatycznej obsługi, mimo ochronnego odrzucenia. Pozostają jawnie oznaczone `resource-limit`; nie zmieniono asercji, żeby uzyskać zielony raport. Limity: 1 mln rozwiniętych wierzchołków, ZIP 50 MiB łącznie / 25 MiB wpis / 256 wpisów, GLB 25 MiB. Zwykły frontend dodatkowo ogranicza plik do 25 MiB. Duże poprawne projekty MakerWorld mogą być nieobsługiwane.

Przed poprawkami było 19 PASS / 22 FAIL: 13 cen, 4 ręczne, 2 prawidłowe 413, 17 limitów, 3 błędy proxy oraz 2 niewyjaśnione odrzucenia parsera. Model 2 otrzymuje **wycenę ręczną**, nie automatyczną cenę.

## Poprawki i dowody

- Bambu `paint_*`/`face_property`: pomijane tylko jako ustawienia trójkątów w kopii geometrycznej. Wierzchołki, transformacje, oryginał i pełna walidacja pakietu pozostają. 21 testów geometrii PASS; model 28 przechodzi do ręcznej wyceny, model 36 dochodzi do limitu wierzchołków. Znaczenie pól sprawdzono w [źródle Bambu v02.08.02.61](https://github.com/bambulab/BambuStudio/blob/v02.08.02.61/src/libslic3r/Format/bbs_3mf.cpp).
- Model 4 wcześniej przekraczał 60 s CPU w 6,45 s czasu rzeczywistego. Slicer ograniczony do dwóch dozwolonych CPU przeszedł dla PLA/PETG i przez API. Nie podniesiono limitów; polityka `cpu2` unieważnia stare snapshoty.
- Proxy Vite zwraca 413 zamiast 500 dla odtworzonego EPIPE przy zbyt dużych uploadach. Trzy regresje PASS. Obsługa nie obejmuje EPIPE bez deklarowanego Content-Length; zwykłe błędy proxy nie są maskowane.
- Poprawiono zakres CSS etykiet dostawy i fałszywe ostrzeżenie o braku analizy mimo oszacowanych paczek. Zachowano styl autora. Test wyboru kurierów kończy przewijanie przed normalnym kliknięciem, bez force/retry.

## Pozostałe sprawdzone przepływy

| Raport prywatny w `frontend/tests/artifacts/` | Wynik i zakres |
| --- | --- |
| `security-current.json` | 2 PASS: błędne dane, zmiana hasła, passkey/TOTP, sesje |
| `auth-email-current.json` | 1 PASS: prawdziwy iCloud do void, hasło/email, wirtualny passkey/email, admin TOTP i klient → admin 403 |
| `commerce-current.json` | 5 PASS: Stripe test, dostawa sandbox, panele/chat/pliki, częściowy/pełny/nadmierny zwrot, cennik |
| `checkout-final-current.json` | 3 PASS: dokładna kwota oferty/przycisku/płatności, odbiór/paczkomat/kurier; obejrzane podsumowania desktop/mobile |
| Test `assets.spec.ts` | 2 PASS: obrazki `<img>` strony głównej i oba motywy; głównie logo, nie wszystkie grafiki serwisu |

To izolowane bazy i jawne koszty testowe. Nie dowodzi to produkcyjnej rentowności, fizycznego passkey, wszystkich przewoźników ani nadania przesyłki. Raportów zawierających dane formularzy nie publikujemy.

## Otwarte

- Pozostałe 59 pobrań: zalogowana przeglądarka nie zwraca plików; oczekujemy informacji o ręcznym pobraniu.
- Świadoma polityka obsługi dużych modeli zamiast bezwarunkowego zwiększania limitów; trwała procedura odtworzenia korpusu.
- Fingerprint rzeczywistego runtime Bambu, niezależna kontrola kosztów i ilości, wszystkie grafiki/ekrany, współbieżność i pozostałe E2E.
- `cargo audit`: 0 podatności/ostrzeżeń. `npm audit`: trzy wpisy low pochodzące z jednego `deepmerge` w narzędziach deweloperskich; [brak opublikowanej poprawki](https://github.com/advisories/GHSA-gcqx-w48m-w2h4). Nie jest to dowód podatności przepływu uploadu.
- Przeglądy Sol xhigh pozostają poniżej 10/10. Cel nie jest zakończony.
