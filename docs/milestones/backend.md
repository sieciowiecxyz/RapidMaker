# Checkpoint backendu

2026-10-23. B1–B5 ukończone jako lokalny POC. Dawny plan M0–M6 i szczegóły kolejnych kroków są w historii Git.

## Stan

- B1: `app → api → moduły`; jeden router, logika obliczeń poza HTTP.
- B2: PostgreSQL/SQLx, 49 materiałów/314 kolorów Bambu, wersjonowany cennik i niezmienne snapshoty. Import zachowuje ceny, magazyn i dopuszczenia operatora.
- B3: STL/STEP/3MF → prywatny oryginał/kanoniczny STL → izolowany Bambu Studio. STEP: pojedyncza część w mm; 3MF: jednostki/transformacje, bez ustawień/G-code klienta. Podpory automatyczne z zaufanego presetu są wliczone. Prywatne pliki 24 h; cookie właściciela, limity liczby/miejsca.
- B4: rzeczywiste partie 1–1000 kopii, pełne płyty + osobno reszta, maks. dwa przebiegi CLI. Jakość/ściany/wypełnienie wpływają na slicer. Wiele różnych modeli ma osobne partie. Koszty części sumowane przed jednym przygotowaniem, stratami, marżą, minimum i VAT. Cena w groszach, bez stałej taryfy/sztukę.
- B5: panel operatora zapisuje DB przez sesję serwerową. Konfigurator/koszyk używają rzeczywistego katalogu i snapshotów. Checkout sprawdza właściciela, ilość/model, ważność 10 min, wersje i dostępność; pakowanie używa wymiarów serwera i ostrożnej masy slicera. Stripe dostaje kwotę serwera. Brak danych/nieobsługiwane wymagania → ręczna ocena bez płatnej oferty.

## Dowody

- W poprzednich krokach: formaty STL, dwa STEP, 3MF i transformowany model w cm; uszkodzenia/ZIP traversal/obce jednostki odrzucone. Slicer 1/2/10/100/1000 kopii, 1000 na 9 płytach; PLA Basic/PETG HF/ABS. Import/restart zachowały konfigurację operatora i snapshoty. Brak/błędny token i obcy Origin odrzucone; browser mock nie nadaje admina.
- Bieżący przebieg HTTP na usuniętej potem bazie testowej: upload → slicer → koszt → koszyk. Dwie kopie 20,12 zł; dwie takie pozycje razem 24,08 zł zamiast 40,24 zł (WYŁĄCZNIE stawki testowe). Pozycje + przygotowanie sumują się dokładnie. Zmieniona ilość, obca sesja, podana masa klienta i zmieniona dostępność odrzucone. Modele przetrwały restart, wygasłe pliki sprzątane; 0700/0600.
- Chromium bez stubów HTTP: upload STL → materiał/kolor/jakość/wytrzymałość → cena slicera → koszyk → prawdziwy Address Element Stripe → podsumowanie → utworzenie Stripe Checkout test. Sesję zamknięto bez płatności. Obejrzano desktop/mobile; bez poziomego overflow. Ładowanie iframe Stripe wymaga widoczności pola.
- Furgonetka sandbox: trzy oferty kuriera dla paczki wyliczonej z wyników slicera. Bez nadania przesyłki.
- PaymentIntent Stripe test: kwota dokładnie równa cenie serwera + opakowanie/dostawa; retry zwraca tę samą płatność. Obcy właściciel nie może tworzyć/czytać/wznawiać; intent anulowano bez obciążenia.
- PASS: Rust build, fmt, Clippy zmienionych crate’ów; 11 testów API, 4 pricing i 4 slicer. Ciężki test slicera pozostawiono ignored; rzeczywisty przebieg sprawdzono przez HTTP. Frontend build/svelte-check: 0 błędów/ostrzeżeń. Test adaptera ze stubami osobno potwierdził 1000 sztuk i cache; nie zastępuje powyższego przebiegu rzeczywistego.

Rust LOC tego kroku: 6266 → 6707 (+441), tokei z baseline `git archive HEAD`, razem z testami inline, bez pustych linii/komentarzy. Nowy `api/cart.rs` odpowiada za własność i składanie wycen; brak nowych zależności.

## Ograniczenia i następny zakres

Ceny zakupu i zatwierdzenie sprzętu muszą pochodzić od operatora; normalnej bazie nie wpisano fikcyjnych cen ani stanów magazynu. 41 profili P1P nie oznacza fizycznej kwalifikacji wszystkich materiałów. Kalibracja kosztów/próbne wydruki przed sprzedażą.

Oferty i stan wznowienia płatności API są w RAM. Zamówienia, konta/role, rozmowy i outbox emaili mają już trwały backend (aktualizacje poniżej); produkcyjne webhooki i monitoring pozostają planem; kolejka slicera wdrożona poniżej. Koszyk i panele zamówień są prototypami. Masa pakowania obejmuje odpady/podpory, więc jest ostrożnym szacunkiem, nie dokładną masą gotowej części. Nie ma optymalizacji wspólnej płyty różnych modeli. Publiczne wdrożenie wymaga HTTPS/Secure i osobnego przygotowania.

NOT RUN: realne obciążenie karty, nadanie przesyłki, pełny pentest/obciążenie/odtwarzanie po awarii. Starsze mockowe `flows.spec.ts` wymagają migracji; nie raportowano całego E2E jako PASS. Zachowano cudze dopiski `AGENTS.md` i nieśledzony `result.json`.

## Email i backend klienta — 2026-10-23

E1–E4 ukończone lokalnie; dowody i ograniczenia: [email](../email.md). Backend auth/sesji, zamówień/rozmów, kolejki i preferencji działa; UI jeszcze niepodłączone. Potwierdzenie płatności przez sprawdzony odczyt Stripe; bez produkcyjnego webhooka; passkey dodano w kolejnym kroku poniżej.

## Auth — 2026-10-23

Passkey admina, wspólne sesje/CSRF, cookie produkcyjne i własność zasobów konta wdrożone. Legacy Bearer/operator-cookie wyłączone. Rust/PG/Chromium virtual authenticator PASS; kontrakt i ograniczenia: [auth](../auth.md).

## Kolejka slicera — 2026-10-23

B6 PASS: zadania/snapshot w PostgreSQL, lokalne pliki, lease/retry i polling frontendu. Rzeczywisty restart/odzyskanie workera oraz cena checkoutu sprawdzone na izolowanym środowisku. [Szczegóły i ograniczenia](../slicer-queue.md).

## Podglądy GLB — 2026-10-23

B7 PASS: backend GLB dla STL/STEP/3MF, metry/Y-up, chroniony endpoint i quota z preview_bytes. Rzeczywisty upload/pobranie trzech formatów, restart i odmowy dostępu/uszkodzeń PASS. Rust: 30 testów (geometria/API/storage), izolowana regresja PostgreSQL limitów, fmt/Clippy/build PASS. Pozostałych 7 ignorowanych testów PG nie powtarzano. Frontend GLB do podłączenia; starsze uploady wymagają ponownego przesłania. Rust backend/crates: 11156 → 11391 (+235), razem z testami inline, bez komentarzy/pustych/generowanych plików. Nowa migracja 0010; bez nowych zależności. [Kontrakt](../../backend/README.md#podgląd-glb).

## Retencja zamówień — 2026-10-23

B8 PASS: pliki chronione przed Stripe i przez realizację, 90 dni po completed; pobieranie z zamówienia niezależne od roboczego TTL. Test HTTP: rzeczywisty upload/cleanup, restart, uprawnienia i statusy admina; PostgreSQL: nowa retencja oraz istniejący backend zamówień (2 testy) PASS. Build/fmt/Clippy i 16 testów jednostkowych API/storage PASS; pozostałych 7 ignorowanych testów PG nie powtarzano. Płatności/nadania NOT RUN. Niepewne/porzucone próby zachowują pliki do rozstrzygnięcia; brak automatycznego backfillu starych zamówień. Rust backend/crates: 11391 → 11865 (+474), razem z testami inline, bez komentarzy/pustych/generowanych plików. Nowe order_files.rs i migracja0011 przechowują powiązania/retencję; bez nowych zależności. [Kontrakt](../../backend/README.md#pliki-zamówień).

## Dokumentacja API — 2026-10-24

Własny spis zastąpiony Swagger UI (`/api`) i OpenAPI 3.1 (`/api/openapi.json`): 52 operacje/48 ścieżek, 30 schematów. `utoipa-axum::routes!` wspólnie rejestruje endpoint i dokumentację; `utoipa` generuje schematy rzeczywistych DTO. Usunięte build.rs, parser AST i własny HTML. Zasoby Swagger lokalnie, bez CDN/online validatora i zapisywania autoryzacji w localStorage. Dynamiczne odpowiedzi JSON mają ogólny schemat.

Build/Clippy/fmt, 13 testów API PASS. HTTP: kompletność wobec wcześniejszych tras, unikalne operationId, poprawne referencje schematów, lokalne zasoby i odmowa admina PASS. Swagger „Try it out” dla katalogu: HTTP 200; nie wywoływano zapisów/płatności/emaili. Rust backend/crates: 12213 → 12355 (+142), razem z testami inline, bez komentarzy/pustych/generowanych plików. Nowe zależności: utoipa, utoipa-axum, utoipa-swagger-ui; usunięta bezpośrednia build-dependency syn. Cargo audit nadal zgłasza 9 podatności zastanych zależności; nie jest PASS. Porównanie z poprzednim lockfile nie wykazało nowych zgłoszeń.
