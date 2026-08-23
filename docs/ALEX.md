# ALEX

## ALEX ROLE

ALEX è il nodo web pubblico per il sito Photaxon.

- Frontend pubblico HTML/CSS/JavaScript per la demo FiberSpider.
- Server Apache HTTPS frontend.
- Dashboard e visualizzazione del laboratorio.
- Interfaccia di attivazione tecnico con OTP.
- Reverse proxy verso il backend FiberSpider per le API.
- Non accede direttamente al database.

## ARCHITETTURA CONCETTUALE

Browser
→ ALEX / Apache HTTPS
→ FiberSpider backend
→ Persistent database node

## PRINCIPI

- ALEX serve la presentazione pubblica.
- Il backend gestisce la logica e i dati.
- Il frontend non deve contenere segreti o dati operativi.
- ALEX non deve esporre direttamente IP interni, credenziali o informazioni sensibili.
