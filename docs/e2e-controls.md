# Inwentaryzacja kontrolek E2E

Wygenerowano: `node frontend/tests/inventory-controls.mjs`. To mapa źródeł, nie wynik testów. Jedna kontrolka szablonu może tworzyć wiele opcji/wierszy; wartości i pokrycie opisują pliki e2e-*-inventory.md oraz raport. Nie klikamy automatycznie wszystkiego bez sprawdzenia skutków.

Kontrolki szablonów: **447**, pliki: **45**.

## frontend/src/App.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 21 | a | Spróbuj ponownie |

## frontend/src/Workspace.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 166 | a | Przejdź do treści |
| 171 | button | Spróbuj ponownie |
| 175 | a | <a {href} aria-current={path.startsWith(href) ? 'page' : undefined}> |
| 188 | a | Zaloguj się jako administrator |
| 189 | a | Moje zamówienia |
| 209 | a | Wróć na stronę główną |
| 214 | a | rapidmaker |
| 217 | a | Regulaminy i prywatność |

## frontend/src/admin-design/Admin.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 45 | svelte:window | <svelte:window onkeydown={keyboard}/> |
| 49 | a | RapidMaker |
| 52 | button | <button class:active={view===item.name} onclick={()=> |
| 55 | a | Panel z danymi serwera |
| 55 | a | Otwórz stronę |
| 58 | button | <button class="icon" onclick={theme} aria-label={light?'Włącz ciemny motyw':'Włącz jasny motyw'}> |
| 60 | button | Eksport CSV |
| 64 | button | Sprawdź kolejkę |
| 65 | button | <button class:chosen={filter===f} aria-pressed={filter===f} onclick={()=> |
| 65 | input | <input bind:value={query} placeholder="Szukaj numeru, klienta, pliku…" aria-label="Szukaj zamówień"/> |
| 65 | button | <button class="icon" onclick={()=> |
| 67 | button | <button class="order-link" onclick={(e)=> |
| 67 | button | <button class="icon" onclick={(e)=> |
| 68 | button | Wyczyść filtry |
| 71 | button | <button class="order-link" onclick={(e)=> |
| 71 | button | <button class="secondary" onclick={()=> |
| 71 | button | Cofnij etap |
| 75 | button | <button class="message-row" onclick={async(e)=> |
| 81 | button | <button class="icon" aria-label="Zamknij szczegóły" onclick={close}> |
| 83 | select | <select value={selected.status} onchange={(e)=> |
| 83 | input | <input type="date" bind:value={selected.due} onchange={()=> |
| 84 | button | <button class:chosen={tab===t} aria-pressed={tab===t} onclick={()=> |
| 88 | input | <input type="number" min="0" step="0.01" value={m.price} onchange={(e)=> |
| 88 | input | <input type="number" min="0" max={m.qty} value={m.done} onchange={(e)=> |
| 89 | button | <button class="secondary" onclick={()=> |
| 103 | button | Pobierz |
| 103 | button | Bambu Studio |
| 103 | input | <input type="file" accept=".stl,.step,.stp,.3mf" aria-label={'Przypnij plik do '+m.name} onchange={(e)=> |
| 107 | textarea | <textarea rows="5" bind:value={selected.draft} placeholder="Napisz odpowiedź dotyczącą realizacji…"> |
| 107 | button | Zapisz szkic |
| 109 | form | <form onsubmit={(e)=> |
| 109 | textarea | <textarea rows="4" bind:value={note} placeholder="Np. sprawdzić tolerancję otworu po wydruku" required> |
| 109 | button | Dodaj notatkę |
| 112 | button | <button class="primary" disabled={selected.status==='Zakończone'} onclick={()=> |

## frontend/src/admin-design/Finance.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 17 | button | <button class="secondary" onclick={()=> |
| 23 | form | <form onsubmit={(e)=> |
| 24 | input | <input bind:value={draft.buyer} required maxlength="200"/> |
| 25 | textarea | <textarea bind:value={draft.address} required rows="2" maxlength="500"> |
| 26 | input | <input bind:value={draft.nip} maxlength="20"/> |
| 29 | button | Aktualizuj z zamówienia |
| 30 | button | Zapisz szkic |
| 30 | button | Wystaw fakturę |
| 34 | button | Utwórz szkic faktury |
| 42 | button | <button class:chosen={filter===f} aria-pressed={filter===f} onclick={()=> |
| 42 | input | <input aria-label={'Szukaj: '+mode} bind:value={search} placeholder="Szukaj zamówienia lub klienta…"/> |
| 44 | button | <button class="order-link" onclick={(e)=> |
| 45 | button | Wyczyść filtry |

## frontend/src/admin/Admin.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 383 | svelte:window | <svelte:window onkeydown={keyboard} /> |
| 388 | a | RapidMaker |
| 393 | button | <button class:active={view === item.name} onclick={() => |
| 406 | a | Materiały i ceny |
| 407 | a | Ustawienia wyceny |
| 408 | a | Bezpieczeństwo konta |
| 409 | button | Wyloguj |
| 410 | a | Otwórz stronę |
| 422 | a | Konto |
| 423 | button | Odśwież |
| 423 | button | <button class="icon" onclick={theme} aria-label={light ? 'Włącz ciemny motyw' : 'Włącz jasny motyw'} > |
| 447 | button | Eksport CSV |
| 461 | button | Sprawdź kolejkę |
| 467 | button | <button class:chosen={filter === f} aria-pressed={filter === f} onclick={() => |
| 475 | input | <input bind:value={query} placeholder="Szukaj numeru, klienta, pliku…" aria-label="Szukaj zamówień" /> |
| 479 | button | <button class="icon" onclick={() => |
| 498 | button | <button class="order-link" onclick={(e) => |
| 539 | button | <button class="icon" onclick={(e) => |
| 552 | button | Wyczyść filtry |
| 577 | button | <button class="order-link" onclick={(e) => |
| 591 | button | <button class="secondary" disabled={busy \|\| !nextStatus(o)} onclick={() => |
| 599 | button | Cofnij etap |
| 611 | button | <button class="message-row" aria-label={'Rozmowa z ${o.client}, zamówienie ${o.id}'} onclick={async (e) => |
| 642 | button | <button class="icon" aria-label="Zamknij szczegóły" onclick={close} > |
| 652 | select | <select value={productionEditable(selected) ? 'production:' + productionFor(selected).stage : selected.state} disabled={busy \|\| !productionEditable(selected)} onchange={(e) => |
| 666 | input | <input type="date" value={selected.due} disabled={busy} onchange={(e) => |
| 675 | button | <button class:chosen={tab === t} aria-pressed={tab === t} onclick={() => |
| 686 | ManualOffer | <ManualOffer orderId={selected.id} offer={selected.manualOffer} adminMode onchange={load} /> |
| 727 | input | <input type="number" value={m.price} placeholder="Indywidualnie" disabled title="Cena z zapisanej wyceny" /> |
| 735 | input | <input type="number" min="0" max={m.qty} step="1" value={m.done} disabled={busy \|\| !productionEditable(selected)} onchange={(e) => |
| 753 | input | <input type="checkbox" checked={productionFor(selected).qc_passed} disabled={busy} onchange={(e) => |
| 776 | Refunds | <Refunds orderId={selected.id} amountGrosze={selected.amountGrosze} paid={selected.paid} onchange={load} /> |
| 806 | a | Pobierz |
| 810 | button | Zmień plik |
| 830 | button | Pokaż kolejne |
| 835 | form | <form onsubmit={(e) => |
| 842 | textarea | <textarea rows="5" maxlength="4000" bind:value={selected.draft} placeholder="Napisz odpowiedź dotyczącą realizacji…"> |
| 847 | button | Wyślij wiadomość |
| 854 | form | <form onsubmit={(e) => |
| 861 | textarea | <textarea rows="6" maxlength="4000" bind:value={noteDraft} disabled={busy} > |
| 864 | button | Zapisz notatkę |
| 867 | Shipment | <Shipment orderId={selected.id} onchange={load} /> |
| 876 | button | <button class="primary" disabled={busy \|\| !nextStatus(selected)} onclick={() => |

## frontend/src/admin/Finance.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 123 | form | <form onsubmit={(e) => |
| 130 | input | <input bind:value={draft.buyer} required maxlength="200" disabled={busy} /> |
| 133 | textarea | <textarea bind:value={draft.address} rows="2" maxlength="500" disabled={busy}> |
| 140 | input | <input bind:value={draft.nip} maxlength="10" inputmode="numeric" disabled={busy} /> |
| 148 | input | <input type="date" bind:value={draft.created} required disabled={busy} /> |
| 168 | button | Aktualizuj z zamówienia |
| 172 | button | Zapisz szkic |
| 172 | button | Wystaw fakturę |
| 192 | button | Utwórz szkic faktury |
| 230 | button | <button class:chosen={filter === f} aria-pressed={filter === f} onclick={() => |
| 237 | input | <input aria-label={invoiceMode ? 'Szukaj dokumentów' : 'Szukaj płatności'} bind:value={search} placeholder="Szukaj zamówienia lub klienta…" /> |
| 255 | button | <button class="order-link" onclick={(e) => |
| 277 | button | <button class="text-button" onclick={(e) => |
| 290 | button | Wyczyść filtry |

## frontend/src/admin/Refunds.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 133 | button | Odśwież |
| 161 | button | Ponów to zlecenie |
| 165 | form | <form onsubmit={(event) => |
| 172 | input | <input inputmode="decimal" autocomplete="off" bind:value={amount} placeholder="0,00" disabled={!enabled} required /> |
| 181 | button | Cała pozostała kwota |
| 188 | button | Zleć zwrot |

## frontend/src/admin/Shipment.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 280 | button | Odśwież |
| 311 | button | Sprawdź / ponów tę samą operację |
| 316 | form | <form onsubmit={(e) => |
| 336 | input | <input type="number" min="1" step="1" required bind:value={p.outer_mm[axis]} oninput={changed} /> |
| 346 | input | <input type="number" min="1" step="1" required bind:value={p.weight_g} oninput={changed} /> |
| 356 | button | Usuń paczkę |
| 367 | button | + Dodaj paczkę |
| 377 | input | <input type="checkbox" bind:checked={reviewed} onchange={changed} disabled={busy} /> |
| 385 | button | Zapisz paczki |
| 386 | button | Sprawdź koszt przewozu |
| 412 | input | <input type="checkbox" bind:checked={accepted} disabled={busy} /> |
| 416 | button | Zamów przewóz za |
| 435 | input | <input type="checkbox" value={p.id} bind:group={selected} disabled={busy \|\| !!intent \|\| activeAction \|\| !p.provider_id \|\| p.state === 'canceled'} onchange={() => |
| 454 | button | Etykieta PDF |
| 462 | button | Potwierdź przekazanie kurierowi |
| 474 | summary | Podjazd kuriera |
| 476 | input | <input type="date" bind:value={readyDate} onchange={() => |
| 487 | input | <input type="checkbox" bind:checked={repeat} onchange={() => |
| 497 | button | Pokaż dostępne terminy |
| 501 | select | <select bind:value={chosen} disabled={busy} > |
| 506 | button | Zamów podjazd |
| 518 | summary | Anulowanie zaznaczonych przesyłek |
| 523 | input | <input type="checkbox" bind:checked={cancelConfirmed} disabled={busy} /> |
| 525 | button | Zleć anulowanie |

## frontend/src/components/AccountSecurity.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 154 | button | Odśwież stan |
| 155 | button | Wyloguj wszystkie urządzenia |
| 165 | a | Zaloguj się ponownie |
| 175 | form | <form onsubmit={(event) => |
| 184 | input | <input type="password" autocomplete="new-password" bind:value={password} maxlength="512" required disabled={busy} /> |
| 193 | input | <input type="password" autocomplete="new-password" bind:value={repeat} maxlength="512" required disabled={busy} /> |
| 203 | button | Zapisz hasło |
| 218 | a | Otwórz w aplikacji uwierzytelniającej |
| 222 | form | <form class="totp-form" onsubmit={(event) => |
| 230 | input | <input inputmode="numeric" autocomplete="one-time-code" pattern={'[0-9]{6}'} maxlength="6" bind:value={code} required disabled={busy} /> |
| 239 | button | Potwierdź włączenie |
| 243 | button | Wyłącz aplikację |
| 249 | button | Włącz aplikację |
| 265 | button | Zapisałem kody — ukryj |
| 273 | button | Spróbuj ponownie |

## frontend/src/components/CheckoutSteps.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 16 | a | <a href="/wycena?restore=cart"> |
| 18 | button | <button type="button" onclick={() => |

## frontend/src/components/DemoTools.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 26 | summary | <summary > |
| 35 | select | <select bind:value={scenario} onchange={change} > |
| 40 | button | Przywróć demo |

## frontend/src/components/Header.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 8 | a | <a class="brand" href="/" aria-label="Rapidmaker — strona główna"> |
| 12 | a | Wycena online |
| 13 | a | Kontakt |
| 16 | a | <a class="account-link" aria-label={session?.role === 'admin' ? 'Panel admina' : session ? 'Moje konto' : 'Zaloguj się'} href={session?.role === 'admin' ? session.adminVerified ? ' |
| 35 | button | Wyloguj |

## frontend/src/components/LegalNotice.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 12 | a | Regulamin |
| 13 | a | Polityka prywatności |

## frontend/src/components/ManualOffer.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 190 | button | Odśwież |
| 215 | form | <form onsubmit={(event) => |
| 223 | input | <input inputmode="decimal" autocomplete="off" bind:value={total} disabled={!editable} required /> |
| 231 | input | <input inputmode="decimal" autocomplete="off" bind:value={shipping} disabled={!editable} required /> |
| 241 | textarea | <textarea rows="3" maxlength="4000" bind:value={scope} disabled={!editable} required placeholder="Co wykonamy, termin, metoda i zakres dostawy…"> |
| 254 | button | <button class="action" disabled={busy} > |
| 271 | a | Przejdź do konfiguratora |
| 272 | ManualPayment | <ManualPayment {payment} {orderId} {onchange} /> |
| 279 | button | <button class="action" disabled={busy} onclick={pay} > |

## frontend/src/components/ManualPayment.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 103 | button | <button class="button primary" disabled={!ready \|\| busy} onclick={confirm} > |

## frontend/src/components/ManualQuoteRequest.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 185 | a | piotr@rapidmaker.pl |
| 189 | a | Zaloguj się i wróć do zapytania |
| 194 | form | <form class="stack" onsubmit={(event) => |
| 204 | input | <input autocomplete="shipping name" bind:value={contact.name} maxlength="120" required /> |
| 210 | input | <input type="email" value={user.email} readonly /> |
| 211 | input | <input type="tel" autocomplete="shipping tel" bind:value={contact.phone} maxlength="30" required /> |
| 225 | input | <input type="radio" name="manual-delivery" value={method.id} bind:group={delivery} onchange={() => |
| 237 | input | <input autocomplete="shipping postal-code" pattern={'[0-9]{2}-[0-9]{3}'} bind:value={contact.postalCode} oninput={() => |
| 250 | input | <input autocomplete="shipping address-level2" bind:value={contact.city} maxlength="120" required /> |
| 259 | input | <input autocomplete="shipping street-address" bind:value={contact.street} maxlength="200" required /> |
| 267 | button | <button type="button" class="button secondary" disabled={pointBusy} onclick={searchPoints} > |
| 278 | button | <button type="button" class="point" aria-pressed={contact.pickupPoint === point.code} onclick={() => |
| 290 | input | <input type="checkbox" bind:checked={invoice} /> |
| 292 | input | <input bind:value={contact.company} maxlength="200" required /> |
| 294 | input | <input inputmode="numeric" pattern={'[0-9]{10}'} bind:value={contact.nip} maxlength="10" required /> |
| 304 | textarea | <textarea rows="4" bind:value={notes} maxlength="4000" placeholder="Ważne powierzchnie, zastosowanie, termin…"> |
| 316 | button | <button class="button primary" disabled={busy} > |

## frontend/src/components/ModelPreview.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 141 | button | · |
| 155 | button | ← |
| 159 | button | → |

## frontend/src/components/OrderChat.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 40 | Button | Oznacz    jako przeczytane |
| 56 | form | <form onsubmit={send} class="stack"> |
| 57 | textarea | <textarea id="message" bind:value={text} rows="3" maxlength="4000" placeholder="Napisz, w czym możemy pomóc…" required> |
| 65 | Button | <Button type="submit" disabled={busy \|\| !text.trim()} > |

## frontend/src/components/OrderDetail.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 133 | a | ← Wszystkie zamówienia |
| 146 | Button | Spróbuj ponownie |
| 180 | summary | Parametry wydruku |
| 199 | Button | Zamów ponownie ↗ |
| 213 | a | Pobierz oryginał ↗ |
| 222 | ManualOffer | <ManualOffer orderId={order.id} offer={order.manualOffer} {adminMode} onchange={async () => |
| 272 | Button | Przejdź do płatności ↗ |
| 274 | Button | Sprawdź płatność |
| 324 | a | Otwórz produkcję i kontrolę jakości ↗ |
| 331 | select | <select id="order-status" bind:value={status} disabled={!nextStatus \|\| busy} > |
| 339 | input | <input id="lead-time" value={leadTime} disabled /> |
| 343 | textarea | <textarea id="private-notes" value={notes} rows="3" disabled> |
| 347 | Button | Zapisz status |
| 355 | Button | Zleć zwrot |
| 361 | Button | Przygotuj etykietę |
| 375 | OrderChat | <OrderChat {order} role={adminMode ? 'admin' : 'client'} onchange={assign} /> |

## frontend/src/components/ParcelPlan.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 66 | summary | <summary> |

## frontend/src/components/Passkeys.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 115 | a | Zaloguj się ponownie |
| 130 | button | Usuń klucz |
| 138 | a | Potwierdź logowanie ponownie |
| 141 | form | <form class="key-form" onsubmit={(e) => |
| 149 | input | <input bind:value={name} required maxlength="64" autocomplete="off" placeholder="Np. klucz zapasowy" /> |
| 156 | button | <button class="button primary" disabled={busy \|\| keys.length > |
| 167 | form | <form class="stack" onsubmit={(e) => |
| 175 | input | <input bind:value={name} required maxlength="64" autocomplete="off" /> |
| 177 | input | <input type="password" bind:value={bootstrap} required autocomplete="off" /> |
| 183 | button | <button class="button primary" disabled={busy} > |
| 188 | a | Zaloguj się |

## frontend/src/components/ui/Button.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 20 | button | <button {type} {onclick} {disabled} class="button {variant} {className}" > |

## frontend/src/components/ui/Input.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 9 | input | <input bind:value {placeholder} /> |

## frontend/src/pages/Legal.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 37 | a | ← |
| 44 | a | <a href="/regulamin" > |
| 47 | a | <a href="/prywatnosc" > |
| 54 | a | piotr@rapidmaker.pl |
| 60 | button | Spróbuj ponownie |
| 64 | button | Pobierz dokument (.txt) ↓ |

## frontend/src/pages/Login.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 146 | form | <form class="stack" onsubmit={(e) => |
| 154 | input | <input type="password" bind:value={password} minlength="12" maxlength="128" autocomplete="new-password" required disabled={busy} /> |
| 165 | input | <input type="password" bind:value={repeated} minlength="12" maxlength="128" autocomplete="new-password" required disabled={busy} /> |
| 176 | button | Ustaw hasło i przejdź dalej |
| 194 | form | <form class="stack" onsubmit={(e) => |
| 203 | input | <input id="login-code" type="text" inputmode={recovery ? 'text' : 'numeric'} autocomplete="one-time-code" pattern={recovery ? undefined : totp ? '[0-9]{6}' : '[0-9]{8}'} minlength= |
| 216 | button | <button class="button primary" disabled={busy} > |
| 221 | button | <button class="button ghost" disabled={busy} onclick={() => |
| 230 | button | Wyślij kod ponownie |
| 241 | button | Zmień konto |
| 244 | form | <form class="stack" onsubmit={(e) => |
| 252 | input | <input id="email" type="email" bind:value={email} autocomplete="email" maxlength="254" required disabled={busy} /> |
| 263 | button | Utwórz konto — potwierdź email |
| 269 | button | Mam już konto |
| 278 | form | <form class="stack" onsubmit={(e) => |
| 286 | input | <input id="email" type="email" bind:value={email} autocomplete="username webauthn" maxlength="254" required disabled={busy} /> |
| 297 | input | <input id="password" type="password" bind:value={password} autocomplete="current-password" maxlength="128" required disabled={busy} /> |
| 307 | button | <button class="button primary" disabled={busy} > |
| 310 | button | Zaloguj passkey |
| 321 | button | Utwórz konto lub zabezpiecz dotychczasowe |
| 330 | a | Możesz też zamówić jako gość → |

## frontend/src/pages/account/Orders.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 34 | Button | Nowy wydruk ↗ |
| 39 | Button | Spróbuj ponownie |
| 44 | Button | Dodaj model |
| 76 | a | Szczegóły ↗ |
| 81 | a | Wiadomości |

## frontend/src/pages/account/Settings.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 70 | Button | Spróbuj ponownie |
| 72 | form | <form onsubmit={save} class="stack"> |
| 86 | input | <input type="checkbox" disabled={busy} bind:checked={profile.notifications.message} /> |
| 92 | input | <input type="checkbox" disabled={busy} bind:checked={profile.notifications.status} /> |
| 107 | Button | <Button type="submit" disabled={busy}> |
| 110 | AccountSecurity | <AccountSecurity onchange={() => |
| 119 | a | piotr@rapidmaker.pl |
| 122 | a | polityką prywatności |

## frontend/src/pages/admin/Materials.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 168 | input | <input bind:value={search} type="search" placeholder="Nazwa lub identyfikator, np. PLA" /> |
| 179 | Button | Spróbuj ponownie |
| 181 | summary | <summary> |
| 182 | form | <form class="editor" onsubmit={(event) => |
| 185 | input | <input bind:value={newMaterial.id} required maxlength="64" placeholder="BAMBU_CUSTOM" /> |
| 192 | input | <input bind:value={newMaterial.name} required maxlength="200" /> |
| 194 | input | <input bind:value={newMaterial.density} inputmode="decimal" required /> |
| 201 | input | <input bind:value={newMaterial.price} inputmode="decimal" placeholder="Nieustalona" /> |
| 212 | Button | Dodaj materiał |
| 222 | summary | <summary> |
| 239 | form | <form class="stack" onsubmit={(event) => |
| 241 | input | <input bind:value={item.name} required maxlength="200" /> |
| 243 | input | <input bind:value={item.density} inputmode="decimal" required /> |
| 250 | input | <input bind:value={item.price} inputmode="decimal" placeholder="Nieustalona" /> |
| 258 | input | <input type="checkbox" bind:checked={item.enabled} /> |
| 266 | Button | Zapisz materiał |
| 271 | form | <form class="color-row" onsubmit={(event) => |
| 275 | input | <input bind:value={color.name} required maxlength="200" /> |
| 282 | summary | Kolor |
| 283 | input | <input bind:value={color.rgba[index]} required pattern={'#[0-9A-Fa-f]{8}'} maxlength="9" /> |
| 291 | Button | Zapisz kolor |
| 303 | summary | Dodaj kolor |
| 304 | form | <form class="stack" onsubmit={(event) => |
| 309 | input | <input bind:value={item.newColor.code} required maxlength="64" /> |
| 312 | input | <input bind:value={item.newColor.name} required maxlength="200" /> |
| 315 | input | <input bind:value={item.newColor.rgba[0]} required pattern={'#[0-9A-Fa-f]{8}'} maxlength="9" placeholder="#FFFFFFFF" /> |
| 325 | Button | Dodaj kolor |

## frontend/src/pages/admin/Orders.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 53 | input | <input bind:value={search} type="search" placeholder="Numer, klient, model…" /> |
| 55 | select | <select bind:value={status} > |
| 60 | input | <input type="checkbox" bind:checked={attention} /> |
| 66 | Button | Spróbuj ponownie |
| 83 | Button | Otwórz ↗ |

## frontend/src/pages/admin/Production.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 57 | Button | Spróbuj ponownie |
| 78 | a | Szczegóły → |
| 81 | Button | <Button disabled={busy === order.id \|\| (stage.next === 'completed' && order.delivery !== 'pickup')} onclick={() => |

## frontend/src/pages/admin/Settings.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 164 | Button | Spróbuj ponownie |
| 170 | form | <form class="panel settings-list" onsubmit={save}> |
| 179 | input | <input id="rates-version" bind:value={version} required maxlength="128" placeholder="np. 2026-10-23.1" /> |
| 191 | input | <input id={field.key} bind:value={values[field.key]} inputmode={field.integer ? 'numeric' : 'decimal'} required aria-label={'${field.label} (${field.unit})'} /> |
| 202 | Button | <Button type="submit" disabled={busy}> |

## frontend/src/pages/checkout/Checkout.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 468 | button | Spróbuj ponownie |
| 477 | button | Sprawdź ponownie |
| 478 | a | Moje zamówienia |
| 490 | a | Utwórz konto przez email → |
| 494 | a | Moje zamówienia |
| 501 | a | Przejdź do wyceny → |
| 516 | a | Wróć do rozpoczętej płatności → |
| 521 | form | <form class="checkout-layout work-surface" novalidate oninput={saveDraft} onchange={saveDraft} onsubmit={(e) => |
| 541 | button | Popraw kontakt lub fakturę → |
| 557 | input | <input required autocomplete="shipping name" id="checkout-name" aria-invalid={!!fieldErrors.name} aria-describedby={fieldErrors.name ? 'error-name' : undefined} bind:value={contact |
| 568 | input | <input required type="email" autocomplete="shipping email" id="checkout-email" aria-invalid={!!fieldErrors.email} aria-describedby={fieldErrors.email ? 'error-email' : undefined} b |
| 581 | input | <input required type="tel" autocomplete="shipping tel" id="checkout-phone" aria-invalid={!!fieldErrors.phone} aria-describedby={fieldErrors.phone ? 'error-phone' : undefined} bind: |
| 597 | StripeCheckout | <StripeCheckout bind:this={stripeForm} collectAddress={cart.delivery === 'courier'} publishableKey={stripeKey} amount={Math.round((cart.quote?.total ?? 1) * 100)} {contact} payment |
| 622 | input | <input required pattern={'[0-9]{2}-[0-9]{3}'} placeholder="00-001" autocomplete="shipping postal-code" id="checkout-postalCode" aria-invalid={!!fieldErrors.postalCode} aria-describ |
| 635 | input | <input required autocomplete="shipping address-level2" id="checkout-city" aria-invalid={!!fieldErrors.city} aria-describedby={fieldErrors.city ? 'error-city' : undefined} bind:valu |
| 648 | input | <input required autocomplete="shipping street-address" id="checkout-street" aria-invalid={!!fieldErrors.street} aria-describedby={fieldErrors.street ? 'error-street' : undefined} b |
| 663 | input | <input type="checkbox" bind:checked={invoice} /> |
| 665 | input | <input required id="checkout-company" aria-invalid={!!fieldErrors.company} aria-describedby={fieldErrors.company ? 'error-company' : undefined} bind:value={contact.company} autocom |
| 676 | input | <input required pattern={'[0-9]{10}'} maxlength="10" placeholder="10 cyfr" id="checkout-nip" aria-invalid={!!fieldErrors.nip} aria-describedby={fieldErrors.nip ? 'error-nip' : unde |
| 701 | input | <input type="radio" name="delivery" value={method.id} checked={cart.delivery === method.id} disabled={busy} onchange={() => |
| 728 | button | Zmień punkt |
| 741 | button | Szukaj w innym miejscu |
| 752 | input | <input placeholder="Miasto, ulica lub nazwa Paczkomatu" bind:value={pointSearch} onkeydown={(event) => |
| 763 | button | Szukaj |
| 777 | button | <button type="button" class:chosen={contact.pickupPoint === point.code} disabled={busy} onclick={() => |
| 808 | a | Wróć do konfiguracji modelu → |
| 814 | button | Spróbuj ponownie |
| 823 | input | <input type="radio" name="carrier" checked={selectedOffer === offer.id} onchange={() => |
| 843 | button | Spróbuj ponownie |
| 860 | a | Zmień konfigurację |
| 864 | summary | <summary > |
| 891 | button | Zmień |
| 912 | summary | Szczegóły pakowania |
| 974 | button | <button type="submit" class="button primary" disabled={busy \|\| quoteBusy \|\| (stage === 4 && !stripeForm) \|\| (stage === 3 && cart.quote.manual) \|\| (stage === 3 && !cart.quote.manual |
| 998 | button | ← Wróć do podsumowania |
| 1005 | button | ← Wróć do dostawy |
| 1007 | a | ← Wróć do konfiguracji druku |

## frontend/src/pages/checkout/PaymentReturn.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 55 | a | Wróć do płatności → |
| 64 | a | <a class="button secondary" href="/wycena"> |
| 66 | a | <a class="button secondary" href={order.ownerId ? '/konto/zamowienia/' + encodeURIComponent(order.id) : '/logowanie?register=1&email=' + encodeURIComponent(order.contact.email)} > |

## frontend/src/pages/checkout/RetryPayment.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 106 | button | <button class="button primary" disabled={!ready \|\| busy} onclick={confirm} > |

## frontend/src/pages/configure/AdvancedOptions.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 35 | summary | Więcej ustawień i wymagania |
| 41 | select | <select aria-label="Jednostki pliku STL" disabled={busy} value={model.units} onchange={(e) => |
| 57 | select | <select aria-label={field.label} aria-describedby={'${field.key}-hint-${model.id}'} disabled={busy} value={model.advanced[field.key]} onchange={(e) => |
| 72 | input | <input type="number" min="0" max="100" step="1" value={model.advanced.insertCount} disabled={busy} onchange={(e) => |
| 82 | select | <select aria-label="Rozmiar insertów" value={model.advanced.insertSize} disabled={busy \|\| model.advanced.insertCount === 0} onchange={(e) => |
| 93 | textarea | <textarea rows="3" disabled={busy} value={model.notes} placeholder="Ważne wymiary, miejsca styku, kierunek obciążenia…" maxlength="4000" onchange={(e) => |
| 102 | input | <input type="file" accept=".pdf,application/pdf" disabled /> |
| 106 | button | Usuń rysunek |

## frontend/src/pages/configure/Configure.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 111 | a | ← Wróć do konfiguratora |
| 117 | button | Spróbuj ponownie |
| 138 | input | <input aria-label="Wybierz modele" type="file" multiple accept=".stl,.step,.stp,.3mf" disabled={busy} onchange={async (event) => |
| 145 | input | input = event.currentTarget; await files(input.files); input.value = ''; }} /> |
| 146 | input | input.files); input.value = ''; }} /> |
| 147 | input | input.value = ''; }} /> |
| 151 | button | Wypróbuj przykład |
| 168 | button | <button class="part-row" class:active={selected?.id === model.id} aria-pressed={selected?.id === model.id} disabled={busy} onclick={() => |
| 196 | button | Usuń |
| 235 | input | <input type="number" min="1" max="1000" step="1" disabled={busy} value={model.options.quantity} onchange={(event) => |
| 250 | PrintChoices | <PrintChoices {model} {materials} catalog={printOptions} {busy} onchange={(changes) => |
| 257 | AdvancedOptions | <AdvancedOptions bind:open={advancedOpen} {model} catalog={printOptions} {busy} onchange={(changes) => |
| 295 | button | <button class="button primary" disabled > |
| 297 | a | Dalej: wysyłka → |

## frontend/src/pages/configure/PrintChoices.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 62 | button | <button class="material-option" class:chosen={item.id === model.options.materialId} aria-pressed={item.id === model.options.materialId} disabled={busy} onclick={() => |
| 82 | button | <button class="color-option" class:chosen={model.options.color === color.name \|\| model.options.color === color.code} aria-pressed={model.options.color === color.name \|\| model.optio |
| 105 | button | <button class="choice" class:chosen={quality.value === model.options.quality} disabled={busy} aria-pressed={quality.value === model.options.quality} onclick={() => |
| 135 | button | <button class="choice" class:chosen={strength.value === model.strength} disabled={busy} aria-pressed={strength.value === model.strength} onclick={() => |
| 172 | button | Własne ściany i wypełnienie |
| 182 | input | <input type="number" min="2" max="8" step="1" disabled={busy} value={model.advanced.wallCount} onchange={(event) => |
| 196 | input | <input type="number" min="10" max="100" step="1" disabled={busy} value={model.advanced.infill} onchange={(event) => |
| 210 | select | <select disabled={busy} value={model.advanced.pattern} onchange={(event) => |

## frontend/src/storefront/Storefront.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 300 | a | Przejdź do treści |
| 303 | a | <a class="brand" href="#main" aria-label="RapidMaker, początek strony"> |
| 304 | button | Wróć do konfiguracji |
| 304 | a | Oferta |
| 304 | a | Wycena |
| 304 | a | Moje zamówienia |
| 304 | a | Kontakt |
| 305 | button | <button class="icon-button" onclick={changeTheme} aria-label={theme === 'dark' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw'}> |
| 315 | Button | Spróbuj ponownie |
| 340 | input | <input bind:this={fileInput} type="file" class="sr-only" tabindex="-1" aria-label="Dodaj modele STL, STEP lub 3MF" accept=".stl,.step,.stp,.3mf" multiple onchange={event => |
| 341 | button | <button type="button" class:dragging class="dropzone" onclick={() => |
| 344 | a | Zasady prywatności |
| 345 | Button | <Button variant="text" onclick={() => |
| 345 | Button | Pokaż przykład: 10 STL |
| 353 | button | <button class="model-select" aria-pressed={selectedId===model.id} aria-label={'Konfiguruj ${model.name}'} onclick={()=> |
| 358 | button | <button class="remove" aria-label={'Usuń ${model.name}'} onclick={()=> |
| 362 | Button | Następny model |
| 363 | Button | Ponów wysłanie modelu |
| 367 | input | <input type="radio" bind:group={delivery} value="pickup" /> |
| 367 | input | <input type="radio" bind:group={delivery} value="shipping" /> |
| 368 | input | <input type="checkbox" bind:checked={manual} /> |
| 380 | Button | Uzupełnij dla wszystkich |
| 383 | Button | Uzupełnij ustawienia |
| 386 | Button | Ponów przeliczenie |
| 390 | Button | <Button class="quote-contact" disabled={!canContact \|\| preparing} onclick={prepareContact}> |
| 403 | a | 537 320 816 |
| 403 | a | piotr@rapidmaker.pl |
| 403 | Button | Wczytaj mapę Google |
| 403 | a | Wyznacz trasę |
| 405 | summary | Narzędzia podglądu |
| 405 | Button | Zmień motyw |
| 405 | Button | Symuluj błąd wyceny |
| 407 | a | Regulaminy i prywatność |
| 407 | a | Wróć na górę |

## frontend/src/storefront/components/OrderPage.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 75 | button | Wróć do konfiguracji |
| 83 | form | <form bind:this={form} onsubmit={review} oninput={save} onchange={save}> |
| 87 | input | <input class="input" bind:value={data.name} name="name" autocomplete="name" required maxlength="120" pattern=".*\S.*" placeholder="Jan Kowalski"/> |
| 88 | input | <input class="input" bind:value={data.email} name="email" type="email" autocomplete="email" required maxlength="254" placeholder="jan@przyklad.pl"/> |
| 89 | input | <input class="input" bind:value={data.phone} name="phone" type="tel" autocomplete="tel" required pattern={'[+0-9 .\\(\\)\\-]{7,25}'} title="Podaj numer telefonu, np. +48 123 456 78 |
| 93 | input | <input type="checkbox" bind:checked={invoice}/> |
| 94 | input | <input class="input" bind:value={data.company} name="company" autocomplete="organization" required pattern=".*\S.*" maxlength="180"/> |
| 94 | input | <input class="input" bind:value={data.taxId} name="taxId" required pattern="[0-9]{10}" maxlength="10" inputmode="numeric" placeholder="Numer identyfikacji podatkowej"/> |
| 95 | textarea | <textarea class="input" bind:value={data.notes} name="notes" rows="4" maxlength="2000" placeholder="Zastosowanie części, ważne wymiary, oczekiwany termin…"> |
| 104 | Button | <Button type="submit" disabled={preparing}> |
| 104 | a | Zasady prywatności |
| 114 | button | Zmień modele lub ustawienia |

## frontend/src/storefront/components/PrintScene.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 76 | button | <button onclick={()=> |
| 76 | button | <button onclick={()=> |
| 76 | button | <button onclick={()=> |
| 76 | button | <button onclick={()=> |
| 76 | button | <button onclick={()=> |

## frontend/src/storefront/components/PrintTransition.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 43 | button | Pomiń animację |

## frontend/src/storefront/components/ProductionControls.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 22 | input | <input class="choice-input" type="radio" name="material" value={item.id} bind:group={material} aria-describedby="material-description" /> |
| 37 | input | <input class="choice-input" type="radio" name="color" value={item.id} bind:group={color} disabled={!item.available} aria-label={item.name} /> |
| 59 | input | <input class="choice-input" type="radio" name="quality" value={item.id} bind:group={quality} /> |
| 81 | input | <input class="choice-input" type="radio" name="strength" value={item.id} bind:group={strength} aria-describedby="strength-hint" /> |

## frontend/src/storefront/components/RadialPalette.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 35 | button | <button type="button" class:active={series===option} aria-pressed={series===option} onclick={()=> |
| 36 | button | <button type="button" disabled={page===0} aria-label="Poprzednie kolory" onclick={()=> |
| 36 | button | <button type="button" disabled={page> |
| 47 | input | <input class="choice-input" type="radio" name="custom-shade" value={shade.id} bind:group={value} aria-label={shade.name}/> |

## frontend/src/storefront/components/ui/Button.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 6 | button | <button class={'button button-${variant} ${className}'} {...rest}> |

## frontend/src/storefront/components/ui/Input.svelte

| Linia | Typ | Etykieta / początek deklaracji |
| --- | --- | --- |
| 5 | input | <input class={'input ${className}'} bind:value {...rest} /> |
