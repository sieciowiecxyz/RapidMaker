# Finalizacja Rapidmaker

Cel: spójna, działająca strona od uploadu do realizacji zamówienia. Rozwijamy istniejący frontend Piotra i backend Rust; bez redesignu, nowego stosu i pozornych zapisów. Kolejność i dowody: [MILESTONES.md](MILESTONES.md). Poprzednie plany pozostają w historii Git. Mapa kodu: [ARCHITECTURE.md](ARCHITECTURE.md).

## Efekt dla klienta

1. Dodaje STL, STEP lub 3MF; od razu widzi biały podgląd, bez wybierania materiału/koloru. Zmiana koloru jedynie przemalowuje model. Błąd analizy nie pokazuje przykładowego modelu jako jego pliku.
2. Wybiera rodzinę **PLA, ABS albo TPU** przyciskami jak jakość/wytrzymałość, z krótkim opisem. Bez dropdownu, materiałów podporowych i kilkunastu odmian jednej rodziny. Pełne profile techniczne pozostają po stronie operatora. Kolory danej rodziny są widoczne i wybieralne niezależnie od stanu magazynu; usuwamy stock jako warunek wyboru/wyceny. Bezpieczny profil drukarki i jawna cena zakupu nadal są wymagane do automatycznej kalkulacji. Brak dopuszczonego profilu, np. konkretnego TPU, prowadzi do ręcznej wyceny, nie zmyślonej ceny.
3. Otrzymuje serwerową cenę za rzeczywistą partię i dostawę; wiele części i sztuk, jeden koszt przygotowania koszyka. Wynik zależy od slicera, stawek i pakowania, nie od kwot w przeglądarce.
4. Podaje dane raz, wybiera dostawę, sprawdza podsumowanie i płaci Stripe test na naszej stronie. Login klienta/admina: hasło lub passkey + obowiązkowy email/TOTP zgodnie z [auth.md](auth.md).
5. W koncie widzi własne zamówienia, ich części/podglądy, pliki, płatność, status, dostawę i rozmowę. Admin obsługuje te same rzeczy przez własny panel. Powiadomienia i odpowiedzi email trafiają do właściwej rozmowy, bez duplikatów i wycieku cudzych danych.

## Ręczna wycena i późniejsza zapłata

Ręczna wycena pozostaje awaryjną, pełną ścieżką. Powody: przekroczenie 500 × 500 × 500 mm na którejkolwiek osi, brak wykonalnego ułożenia na aktualnej P1P, niedopuszczony profil, niepewna analiza/slicer lub nietypowe pakowanie. Granica 500 mm nie powiększa stołu P1P. Błędny/złośliwy upload jest odrzucany, nie kierowany do operatora jako poprawny model.

Klient wysyła zgłoszenie z plikami i wymaganiami → admin widzi „Oczekuje na wycenę” → wpisuje cenę brutto PLN i zakres/dostawę → zatwierdza ofertę → transakcja zapisuje wersję ceny i email w outbox. Link prowadzi do własnego zamówienia na naszej stronie; wymaga zalogowania do właściwego konta. Gość przed wysłaniem prośby potwierdza konto i przypisuje własne pliki. Sam adres email lub link nie nadają uprawnień.

Klient widzi cenę i płaci istniejącym Stripe flow. Serwer sprawdza własność, wersję, ważność, walutę i kwotę. Zmiana oferty unieważnia stare potwierdzenie; rozpoczętej/niepewnej/opłaconej płatności nie wolno zastąpić nową ceną. Jedna zapłata, jedna zmiana statusu, jeden email; restart, retry i podwójne kliknięcie nie tworzą dubli. W tym etapie jedna próba płatności na ręczne zamówienie: po utworzeniu próby oferta jest nieedytowalna, a po jej definitywnym wygaśnięciu/niepowodzeniu klient wysyła nowe zgłoszenie. Nie tworzymy równoległych płatności. Domyślna ważność oferty: 7 dni; po terminie nowe zgłoszenie, bez płatności starą kwotą. Wycena dostawy jest jawna: wyliczona albo potwierdzona ręcznie z zakresem usługi; nie darmowa fikcyjna dostawa.

## Upload, retencja i współbieżność

Pliki prywatne, ID serwera, kontrola własności, limit rozmiaru/liczby/dysku, sprawdzanie zawartości i archiwów 3MF, brak traversal/symlink/XXE/SSRF i odczytu zewnętrznych zasobów. Parser i slicer izolowane bez sieci/sekretów, z timeoutem i limitami pamięci/CPU. Szczegóły: [slicer-queue.md](slicer-queue.md).

Porzucony upload/szkic: retencja 24 h. Nieopłacona próba: po wygaśnięciu i potwierdzeniu w Stripe, że nie została opłacona i nie jest przetwarzana, zwalniamy hold; sprzątanie usuwa oryginał, siatkę i GLB. Niepewność płatności blokuje usuwanie do uzgodnienia. Prośba o ręczną wycenę: 14 dni na obsługę; opublikowana oferta: 7 dni na zapłatę. Wygaśnięcie bez płatności zwalnia pliki według retencji; klient widzi termin. Zamówienia opłacone zachowują istniejącą ochronę i 90 dni po completed; snapshoty/historia pozostają po usunięciu plików. Testy używają kontrolowanego czasu, nie czekają dni.

HTTP nie czeka na cały slicer: 202 + status i ograniczone odpytywanie. Wielu klientów wysyła/równolegle korzysta ze strony, a kosztowne zadania mają trwałą kolejkę, lease/retry i ograniczoną liczbę workerów. Kolejka nie oznacza nieskończonej liczby procesów. Restart i przekroczenie limitów mają czytelne, bezpieczne zachowanie.

## Wygląd

Źródło stylu: [frontend/README.md](../frontend/README.md), `frontend/src/storefront/theme.css`, mapowanie `frontend/src/theme.css` i istniejące komponenty. Zachowujemy fonty, limonkę, kontrast i język autora. Ujednolicamy skalę odstępów, szerokości, wysokości pól, przyciski, focus/error/loading oraz podglądy w konfiguratorze, checkout, koncie i adminie. Różnice wynikające z funkcji ekranu są celowe; nie narzucamy jednego paddingu wszystkiemu. Zrzuty desktop/mobile i obejrzenie rzeczywistych ekranów po zmianie.

Usuwamy „strona testowa”, „demo”, lorem ipsum i techniczne instrukcje z normalnych widoków. Nie maskujemy błędów ani nie przedstawiamy sandboxu jako produkcyjnej transakcji; tryb dostawców pozostaje w konfiguracji/adminie i dokumentacji.

## Sprawdzenie końcowe

Playwright na działającym frontendzie + Rust + osobnej bazie/pliku storage: różne modele/formaty, biały podgląd bez koloru, konfiguracja, koszyk, adres/dostawa, Stripe test, konto/admin, ręczna oferta i zapłata, chat/email, sesje i odmowy dostępu. Testy z podstawionym API pozostają szybkimi regresjami UI, ale nie są dowodem integracji. Prawdziwe sandboxy sprawdzamy oznaczonym zestawem bez live płatności/nadawania. Email testowy wyłącznie void@sieciowiec.xyz; bez sekretów/kodów w logach i raportach.

Rust: graniczne ilości/wymiary, koszt partii bez podwójnego mnożenia, minimum/VAT/zaokrąglenia, energia/filament, oferta dostawy/cache i kwoty Stripe. Porównujemy koszty z jawnymi stawkami testowymi; zyskowności produkcyjnej nie deklarujemy bez potwierdzonych kosztów. Sprawdzamy uploady, CSRF, IDOR, role, replay, limity, webhooki i współbieżność na własnym localhost. Bez szerokich testów przy każdej kosmetyce; pełny przebieg przy zamknięciu etapu integracji i po istotnej zmianie granic.

Gotowe dopiero po dowodach PASS; FAIL/NOT RUN i blokery zapisujemy jawnie. Nie zamykamy celu na samym buildzie. Brakujące dane/operator/realny klucz lub problem sandboxa izolujemy i zgłaszamy, pozostałą pracę kontynuujemy. Bez publicznego wdrożenia w tym celu.
