# Dokumentacja Rapidmaker

- [Regulamin i prywatność](legal.md)
  - przeczytaj gdy: zmieniasz warunki, akceptację lub prawa klienta.
  - opis: podstrony, wersje, kopie email i braki przed publikacją.

Nadawanie: S0–S4 wdrożone i sprawdzone w sandboxie, łącznie z PDF, trackingiem i wysyłką emaila. Anulowanie L2 sprawdzone w sandboxie; podjazdy L1 zaimplementowane, potwierdzenie rezerwacji blokuje `carrierConnectionError` DPD. Rozmowy przez stronę i email: CH0–CH4 wdrożone i sprawdzone lokalnie. Checkout, adres i dostawa działają jako lokalny backend. ARCHITECTURE.md opisuje faktyczny stack i granice. Czytaj tylko potrzebne pliki. Dawne plany, w tym GOALS.md, są w historii Git; dowody wcześniejszych etapów są w milestones/backend.md. Stare statusy nie są dowodami działania.

- [Plan integracji Piotra](PLAN.md)
  - przeczytaj gdy: sprawdzasz całą stronę i integracje.
  - opis: połączenie PR #4 z rzeczywistym backendem.
- [Milestones integracji](MILESTONES.md)
  - przeczytaj gdy: realizujesz bieżący plan testów.
  - opis: etapy I0–I5, kryteria i bieżący checkpoint.
- [Raport E2E przed integracją](E2E-REPORT.md)
  - przeczytaj gdy: chcesz poznać wyniki i ograniczenia testów.
  - opis: wyniki na main 0db0058; bieżąca integracja ma osobny raport.
- [Integracja Piotra — kontrakt](frontend-contract.md)
  - przeczytaj gdy: zmieniasz wygląd lub adaptery.
  - opis: stały UI, prawdziwe profile i granice API.
- [Integracja Piotra — wyniki](integration-piotr.md)
  - przeczytaj gdy: weryfikujesz merge PR #4.
  - opis: wykonane testy i pozostałe ograniczenia.
- [Mapa kontrolek](e2e-controls.md)
  - przeczytaj gdy: zmieniasz interakcje strony.
  - opis: źródła przycisków/formularzy; mapy scenariuszy obok.
- [Rozmowy i email](chat-email.md)
  - przeczytaj gdy: rozwijasz chat i odpowiadanie z poczty.
  - opis: API historii, SMTP/IMAP, konfiguracja i dowody CH0–CH4.
- [Kontekst](context.md)
  - przeczytaj gdy: zapisujesz decyzję lub wznawiasz pracę.
  - opis: źródła prawdy i krótki zapis dowodów.
- [Workflow](workflow.md)
  - przeczytaj gdy: dzielisz pracę lub zamykasz etap.
  - opis: agenci, integracja i przekazanie.
- [Bezpieczeństwo](security.md)
  - przeczytaj gdy: zmieniasz granice uprawnień lub zależności.
  - opis: poprawki, regresje i otwarte zgłoszenia.
- [Walidacja](testing.md)
  - przeczytaj gdy: sprawdzasz zmianę.
  - opis: komendy i wymagane dowody działania.
- [Metryki](metrics.md)
  - przeczytaj gdy: konfigurujesz Prometheus/Grafanę.
  - opis: prywatny eksport, token, metryki i zapytania.
- [Logi](logging.md)
  - przeczytaj gdy: diagnozujesz backend lub zbierasz logi.
  - opis: JSON, poziomy, korelacja i ochrona danych.
- [Aktualny lokalny frontend Piotra](../frontend-design/README.md)
  - pełny prototyp z portu 5173, zatwierdzone wykończenia, wzornik i efekty WebGL; osobny od integracji backendu.

- [Brief frontendu](rapidmaker-frontend-brief.md)
  - przeczytaj gdy: rozwijasz samodzielny prototyp Svelte.
  - opis: widoki, konfigurator FDM, mocki i granice lokalnego checkout API.
- [Backend — checkpoint](milestones/backend.md)
  - przeczytaj gdy: wznawiasz przebudowę backendu.
  - opis: postęp przebudowy API, PostgreSQL i wyceny slicerem.

- [Sekrety](secrets.md)
  - przeczytaj gdy: konfigurujesz lokalne API lub dostęp współpracownika.
  - opis: SOPS, klucze SSH i odszyfrowanie konfiguracji.

- [Panel produkcji i szkice](admin-production.md)
  - przeczytaj gdy: rozwijasz dane operatora.
  - opis: wersjonowanie, QC, postęp i granice szkicu faktury.
- [Backend](../backend/README.md)
  - przeczytaj gdy: rozwijasz backend Rust.
  - opis: mapa crate’ów, ich stan i uruchamianie.

- [Auth](auth.md)
  - przeczytaj gdy: implementujesz logowanie, sesje lub własność danych.
  - opis: hasło/passkey + email/TOTP, ustawienia, sesje i bootstrap.

- [Email — plan](email.md)
  - przeczytaj gdy: implementujesz wysyłkę lub powiadomienia.
  - opis: iCloud, logowanie, kolejka, zamówienia i preferencje; stan E1–E4.

- [Kolejka slicera i pliki](slicer-queue.md)
  - przeczytaj gdy: rozwijasz upload, zadania lub uruchamiasz osobnego workera.
  - opis: API, lease/retry, limity, retencja i lokalne uruchomienie.

- [Checkout — kontrakt](checkout.md)
  - przeczytaj gdy: używasz adresów, dostawy lub płatności.
  - opis: trwały checkout, webhooki, odzyskiwanie prób i zwroty płatności.
