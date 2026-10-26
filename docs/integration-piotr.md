# Integracja Piotra — PR #4

Połączenie zatwierdzonego frontendu z rzeczywistym API. Status etapów: [MILESTONES.md](MILESTONES.md). Dawny raport E2E dotyczy main 0db0058; nie jest dowodem tej zmiany.

## Zmiany

- Stałe pięć materiałów, czarny, paleta 178 PLA, dysza, lokalny STL i motywy autora. Katalog służy wycenie, nie zastępuje kontrolek.
- Prawdziwy profil/kolor i slicer; brak konfiguracji to ręczna oferta, bez fikcyjnej ceny albo podmiany materiału. Preferencja koloru zapisywana w ręcznym zamówieniu.
- Formularz inline przekazuje kontakt i fakturę; adres jest zbierany raz przez Stripe. Istniejące dostawy, płatności, zwroty i wznowienia zachowują serwerową autoryzację.
- Notatki, termin, ilości, QC/etapy oraz szkic faktury są trwałe. Szkic nie jest wystawioną fakturą; status płatności potwierdza Stripe. [Kontrakt admina](admin-production.md).
- Naprawione brakujące CSS w głównym wejściu PR oraz overflow nagłówka na 390 px. Mapę Google ładuje świadomy klik.

## Weryfikacja

- PASS: Svelte/TypeScript check (0 błędów i ostrzeżeń), Vite build, Rust fmt i Clippy API/storage (`--all-targets -D warnings`). Kompilator C++ nadal zgłasza ostrzeżenie istniejącego wygenerowanego mostu geometrii; bez błędu builda.
- PASS: 24 jednostkowe testy API i 2 reguł produkcji/szkicu. Test routera z odrębną PostgreSQL potwierdził role, CSRF, konkurencyjny zapis/revision, ilości/QC, prywatność i niezmienność płatności. Oddzielny katalogowy test DB pozostaje domyślnie ignored; katalog i pięć profili sprawdzono przez rzeczywiste E2E/API/slicer.
- PASS: 20 testów kontraktów UI. Korzystają z kontrolowanych odpowiedzi API; nie są dowodem operacji u dostawców.
- PASS: 18 różnych scenariuszy rzeczywistego API/DB/przeglądarki w celowanych przebiegach: konfigurator/paleta i pomiary slicera, opcje zaawansowane, prywatne STL/STEP/3MF/ZIP64, odtworzenie GLB, szkodliwe uploady, retencja, izolacja klientów, ceny/oversize, panel/notatki/QC/szkic/chat i zwroty Stripe oraz zakup z odbiorem, kurierem i Paczkomatem. Zakup z odbiorem dodatkowo 3/3 kolejne przebiegi po poprawce fokusu.
- PASS: dwa rzeczywiste scenariusze logistyczne po nowym checkout — DHL sandbox: etykieta/anulowanie oraz etykieta/przekazanie. Produkcja przechodzi przez ukończone sztuki i QC.
- PASS: ręczna wycena → publikacja admina → iCloud email → płatność Stripe → odpowiedź email widoczna w rozmowie (2,4 min). Łącznie 21 różnych rzeczywistych scenariuszy, z wynikami z celowanych przebiegów.
- Obejrzane zrzuty konfiguratora desktop i 390 px; zachowany projekt, poprawiony overflow nagłówka. Prywatne raporty zostają w ignorowanym `frontend/tests/artifacts/`, bez sekretów w repo.

Pierwszy przebieg miał 14/18 PASS. Naprawiono fokus po animacji i odtwarzanie prywatnego GLB; oczekiwania testów dostosowano do świadomie nowego formularza i koloru. Wszystkie cztery początkowo nieudane scenariusze przeszły ponownie. Nie uruchamiano od nowa całego historycznego zestawu E2E ani operacji produkcyjnych Stripe/Furgonetki. Testowe emaile tylko do void@sieciowiec.xyz.

Rust: **28 171 → 28 966 (+795 LOC)** względem main 0db0058, `tokei --type Rust`: `backend/crates/`, razem kod produkcyjny, testy inline/integracyjne i przykłady; bez pustych linii/komentarzy, wygenerowanego kodu i vendora. Nie rozdzielono inline testów od produkcji. Nowe moduły API/storage produkcji i migracja0021 mają jedną odpowiedzialność; bez nowych zależności.

Granice: koszty/dopuszczenia zwykłej bazy pozostają decyzją operatora; fixture nie zmienia produkcji. Inny kolor dla wybranego filamentu pozostaje jawną preferencją ręcznej oferty. admin-design/checkout-design są oznaczonymi wzorcami. Szkic faktury nie wystawia dokumentu/PDF/KSeF.

## Poprawka odporności wyceny — 2026-10-26

Naprawiono eksport normalnych STL, przygotowanie starszych uploadów i ustawienie geometrii na stole. Slicer automatycznie orientuje i układa części; błąd parsowania modelu kończy zadanie bez powtarzania tej samej próby. Rozmiary źródłowe i prywatne GLB nie zmieniają się.

Jawnie rozszerzony lokalny cennik POC: pięć rodzin i serie PLA z presetów, EUR × NBP 4,4026 z 26.10.2026. Istniejące ceny/polityka zachowane; drugie `--pricing-poc-extend` nie zmieniło niczego. Nie jest to zweryfikowany cennik produkcyjny.

PASS: rzeczywisty plik STEP 160×161×5 mm przez HTTP → geometrię → kolejkę → Bambu → cenę dla PLA/PETG/ASA/PA-CF/TPU. PETG, 1 szt., Standard/3 ściany/20%: 5635 s, 65,65 g, 59,73 zł brutto bez dostawy. Faktyczny frontend localhost pokazał tę cenę dla PETG czarnego i szacunek dla PETG Matcha; zachował pierwotną preferencję odcienia w ręcznym zgłoszeniu, bez podmiany na czarny. Brak błędów JS.

PASS: 3 celowane testy geometrii, 6 reguł slicera, rzeczywisty stary STL i obrócona/przesunięta kopia; odrzucenie błędnego modelu przez Bambu; reguła API dopasowania po obrocie; 2 kontrakty UI ceny/powodu; Svelte check/build, fmt i Clippy zmienionych crate’ów. Pełny zestaw E2E i długi batch 1–1000 nie były powtarzane. Istniejące ostrzeżenie wygenerowanego mostu C++ pozostaje. Rust: +207 LOC względem 946ecfc, łącznie produkcja/testy/przykłady w backend/crates, bez pustych linii i komentarzy (`tokei`). Bez nowych zależności; nowy plik to focused test UI `quote-feedback.spec.ts`.
