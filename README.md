# Photaxon — FiberSpider

### Gestione FTTH assistita da intelligenza artificiale

**FiberSpider** è una piattaforma sperimentale per la gestione di attività FTTH, inventario di rete e lavorazioni sul campo.

Il progetto unisce workflow operativi, digital twin della rete, controllo deterministico in Rust, database PostgreSQL distribuito e assistenza tramite ChatGPT/MCP.

L'obiettivo è semplice: rappresentare e seguire una lavorazione FTTH **dalla rete fino al cliente finale**, mantenendo stato, storico, misure e prove dell'intervento.

---

## Cosa permette di gestire

FiberSpider modella la catena FTTH:

```text
ARLO / CRO
   ↓
Splitter
   ↓
Cavi, tubetti e fibre
   ↓
Muffole / giunti
   ↓
ROE / PTE
   ↓
Porte e montanti
   ↓
Segmento di terminazione
   ↓
Borchia ottica / OTO
   ↓
ONT
   ↓
Modem / Router
   ↓
Cliente / WR / Collaudo
```

Per gli elementi fisici possono essere registrati codice operativo, indirizzo, posizione GPS, stato, relazioni con gli altri asset e storico delle modifiche.

---

## Un intervento, in pratica

Un flusso operativo può comprendere:

1. apertura o acquisizione della **WR**;
2. identificazione di armadio, splitter e ROE/PTE;
3. registrazione di indirizzo e **GPS**;
4. verifica della porta disponibile;
5. associazione di cavo, tubetto e fibra;
6. posa del segmento fino alla borchia ottica;
7. registrazione e installazione **ONT**;
8. registrazione o sostituzione del **modem**;
9. inserimento delle misure ottiche;
10. collaudo tramite **ONE**, con codice collaudo obbligatorio quando previsto;
11. associazione delle fotografie dell'intervento;
12. chiusura della lavorazione con storico e audit.

Le operazioni critiche vengono validate prima della scrittura: ad esempio una porta occupata non viene riutilizzata automaticamente.

---

## ChatGPT come interfaccia operativa

FiberSpider dispone di un control plane MCP privato collegato a ChatGPT.

Nella versione corrente del laboratorio, il branch `pc-ragno` espone **112 strumenti MCP** per consultare e amministrare le funzioni disponibili.

Esempi di richieste:

- “Mostrami la topologia dell'armadio.”
- “Verifica se questa porta è libera.”
- “Registra una ROE con questo indirizzo e GPS.”
- “Associa l'ONT alla WR.”
- “Mostrami il collaudo della pratica.”
- “Elenca le fotografie collegate alla lavorazione.”

ChatGPT non riceve accesso SQL libero: le operazioni passano attraverso servizi Rust, validazioni, transazioni e audit.

---

## Tecnico sul campo

Il tecnico può operare attraverso workflow dedicati, incluso Telegram nel laboratorio.

Il sistema distingue il ruolo del tecnico dal control plane amministrativo.

Tra i dati gestibili rientrano:

- WR e appuntamenti;
- ROE/PTE e apparati;
- seriali ONT e modem;
- GPS;
- misure ottiche;
- foto dell'intervento;
- stato e motivazioni di KO;
- collaudo e chiusura.

---

## Foto e prove dell'intervento

Le immagini operative non vengono pubblicate nel repository.

I file restano privati sul nodo RAGNO, mentre nel database vengono conservati i riferimenti necessari alla lavorazione:

- WR;
- asset associato;
- tipo di prova;
- versione;
- hash;
- data;
- autore e storico.

Questo permette di mantenere separata la documentazione tecnica pubblica dai dati reali degli interventi.

---

## Architettura del laboratorio

```text
                    ChatGPT / Lia
                         │
                 Secure MCP Tunnel
                         │
                       RAGNO
               Rust API / MCP / Bot
                         │
                     Yggdrasil
                         │
                        AAA
                  PostgreSQL state

Browser ──> ALEX ──> servizi FiberSpider
```

### Nodi principali

- **AAA** — stato persistente della rete, WR, inventario, audit e collaudi.
- **RAGNO** — servizi Rust, MCP, workflow tecnico e gestione privata delle foto.
- **ALEX** — frontend e punto web del laboratorio.
- **Cabinet agent** — componenti sperimentali per il ragionamento distribuito sugli armadi.

---

## Digital twin FTTH

FiberSpider dispone di modelli per:

- ARLO/CRO;
- splitter primario e secondario;
- muffole;
- cavi;
- tubetti;
- fibre;
- ROE/PTE;
- morsettiere e porte;
- montanti e derivazioni;
- segmenti di terminazione;
- borchie ottiche;
- ONT;
- modem.

Gli asset legacy del laboratorio possono essere importati nel registro fisico mantenendo la provenienza dei dati. Se indirizzo o GPS non sono conosciuti, il sistema li mantiene come mancanti invece di inventarli.

---

## Sicurezza e tracciabilità

Il progetto adotta alcuni principi precisi:

- nessun SQL libero esposto all'assistente;
- validazione Rust prima delle operazioni;
- transazioni e idempotenza;
- audit delle modifiche;
- separazione tra tecnico e amministratore;
- segreti esclusi dal repository;
- foto operative escluse dal repository;
- accesso amministrativo MCP tramite tunnel privato.

---

## Stato attuale

### Verificato nel laboratorio

- backend Rust;
- PostgreSQL AAA;
- rete privata Yggdrasil;
- MCP privato collegato a ChatGPT;
- 112 strumenti MCP live sul branch `pc-ragno`;
- gestione WR;
- modelli fisici FTTH;
- protezione delle porte occupate;
- registrazione ONT e modem;
- workflow ONE manuale con codice collaudo;
- catalogo foto e versioni;
- audit e idempotenza;
- import degli asset legacy;
- test transazionali con rollback.

### Ancora in sviluppo

- tracciamento ottico e giunzioni end-to-end completi;
- lifecycle completo di tutti gli asset;
- integrazione automatica con API ONE, se documentata e disponibile;
- adapter completo verso gestionali delivery esterni;
- ulteriori funzioni assurance e materiali;
- sperimentazione hardware GPON/OLT.

**FiberSpider è un laboratorio e proof of concept operativo, non un sistema dichiarato pronto per deployment GPON di produzione.**

---

## Documentazione tecnica

Per sviluppatori e revisori tecnici:

- [Backend RAGNO e stato operativo](https://github.com/zinga0328it/photaxon/tree/pc-ragno)
- [Architettura aggiornata](https://github.com/zinga0328it/photaxon/blob/pc-ragno/docs/ARCHITETTURA.md)
- [Stato e obiettivi](https://github.com/zinga0328it/photaxon/blob/pc-ragno/docs/OBIETTIVI.md)
- [MCP ChatGPT](https://github.com/zinga0328it/photaxon/blob/pc-ragno/docs/CHATGPT-MCP.md)
- [Modello di sicurezza](https://github.com/zinga0328it/photaxon/blob/pc-ragno/docs/SECURITY-MODEL.md)

La documentazione per agenti e sviluppo resta separata dalla presentazione del prodotto.

---

## Ricerca e collaborazioni

Photaxon / FiberSpider è un progetto di ricerca e sviluppo orientato a:

- automazione FTTH;
- gestione intelligente della rete;
- digital twin;
- agenti LLM applicati alle infrastrutture;
- reti distribuite;
- workflow tecnici assistiti;
- sperimentazione GPON e smart cabinet.

Il repository può essere utilizzato come materiale dimostrativo per collaborazioni tecniche, ricerca e valutazione del progetto.

---

## Licenza

Copyright © Photaxon / FiberSpider.

All rights reserved.

Il repository è pubblicamente visibile a fini di ricerca, dimostrazione e valutazione. Nessuna licenza open source è concessa automaticamente. Uso commerciale, distribuzione o integrazione richiedono autorizzazione scritta del titolare del progetto.
