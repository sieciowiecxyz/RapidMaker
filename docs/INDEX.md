# Dokumentacja Rapidmaker

W tym katalogu: [PLAN.md — Cel i decyzje](PLAN.md), zakres i stan; MILESTONES.md — kryteria etapów; ARCHITECTURE.md — stack i granice. Czytaj tylko potrzebne pliki. Dawne briefy, w tym GOALS.md, są w historii Git; ich skrót jest w PLAN.md. Stare statusy nie są dowodami działania.

- [Kontekst](context.md)
  - przeczytaj gdy: zapisujesz decyzję lub wznawiasz pracę.
  - opis: źródła prawdy i krótki zapis dowodów.
- [Workflow](workflow.md)
  - przeczytaj gdy: dzielisz pracę lub zamykasz etap.
  - opis: agenci, integracja i przekazanie.
- [Walidacja](testing.md)
  - przeczytaj gdy: sprawdzasz zmianę.
  - opis: komendy i wymagane dowody działania.
- [Brief frontendu](rapidmaker-frontend-brief.md)
  - przeczytaj gdy: rozwijasz samodzielny prototyp Svelte.
  - opis: widoki, konfigurator FDM, mocki i granice lokalnego checkout API.
- [Backend — checkpoint](milestones/backend.md)
  - przeczytaj gdy: wznawiasz przebudowę backendu.
  - opis: postęp przebudowy API, PostgreSQL i wyceny slicerem.

- [Sekrety](secrets.md)
  - przeczytaj gdy: konfigurujesz lokalne API lub dostęp współpracownika.
  - opis: SOPS, klucze SSH i odszyfrowanie konfiguracji.

- [Backend](../backend/README.md)
  - przeczytaj gdy: rozwijasz backend Rust.
  - opis: mapa crate’ów, ich stan i uruchamianie.

- [Auth — projekt](auth.md)
  - przeczytaj gdy: implementujesz logowanie, sesje lub własność danych.
  - opis: kod email, passkey admina, PostgreSQL i migracja obecnego auth.
