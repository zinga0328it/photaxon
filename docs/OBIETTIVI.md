# FiberSpider: Stato e Obiettivi

## Chiusura Sessione 2026-11-08

Questa sezione e [ARCHITETTURA.md](ARCHITETTURA.md) descrivono lo stato corrente.
Gli snapshot di sviluppo piu' sotto sono storici, non condizioni ancora attuali.
Nessuna nuova funzionalita' dopo la chiusura; riprendere i pendenti nella prossima sessione.

### Completato e LIVE

- Release ragno-lia-mcp distribuita, profilo privato fiberspider-ragno e
  principal CONTROL_PLANE_ADMIN dal launcher autorizzato; Secure MCP Tunnel
  connesso, nessun listener MCP pubblico o DB locale.
- Migrazioni additive AAA fino a 016 applicate, con backup logico cifrato
  PostgreSQL 17.7 e backup binario/profilo/manifest per rollback.
- 112 tool registrati e visibili dalla vera app ChatGPT, raggruppati nella
  documentazione architetturale; nessun SQL libero o .env esposto.
- Modelli/servizi fisici, WR/delivery locale, prenotazioni/assegnazioni,
  ONE manuale con codice obbligatorio, foto private/catalogo/versioni AAA,
  audit/idempotenza e import legacy.
- Import reale di 51 asset: 3 cabinet, 3 splitter, 44 porte e 1 ROE.
  Provenienza conservata e metadata mancanti LEGACY_INCOMPLETE; dati legacy
  originali invariati, indirizzi/GPS non inventati.
- GPS ROE ADMIN/GPS/AUTHORIZED_IMPORT ammessi; tecnico Telegram ancora vincolato
  alla sorgente TELEGRAM. Messaggio di validazione corretto.
- Artefatto foto MCP-CHECK privo di WR rimosso su autorizzazione dopo backup
  privato. Nessuna foto operativa eliminata o associata a WR inventate.

### Verificato

- Build release, suite Rust, Clippy, runtime Python mock e integrita' dei backup.
- Test AAA sintetico transazionale: rete/fibre/WR/ONT/modem/ONE/foto/import,
  idempotenza/audit, 25 audit, ROLLBACK esplicito e zero residui.
- Dalla vera app ChatGPT: 112 tool, inventario AAA, import legacy,
  get_physical_cabinet CAB-A e dry-run splitter secondario PASS senza persistenza.
- Controlli segreti: env/file privati/traversal/SQL non registrato negati;
  credenziali, tunnel profile, foto e backup fuori dal Git.
- Scansione dell'indice pre-commit: nessuna credenziale reale rilevata;
  unico finding verificato come salt sintetico in un test Rust cfg(test).

### Ancora Pendente

- Digital twin completo: giunzioni/trace ottico end-to-end, lifecycle/delete/
  compensazioni, allineamento completo legacy/fisico e arricchimento sicuro
  metadata di asset referenziati. I modelli tipizzati non equivalgono a queste funzioni.
- Rilievi reali del campo per gli asset importati; verifica live GPS delle
  nuove sorgenti e upload/read/replace di foto operative vere da ChatGPT.
- Squadre/materiali/assurance e delivery avanzati; adapter documentato esterno
  ftth-db tramite WR/reference. Outbox chiusura PENDING, nessun DB-to-DB.
- API ONE automatica non documentata: provider solo MANUAL_RECORDED;
  test sintetici non sono collaudi reali ONE.
- OAuth/MFA admin granulare, annotazioni read-only MCP, autostart/hardening
  systemd del tunnel e OTP del pannello WR; ruolo ALE non verificato.
- Cargo audit: rsa 0.9.10 nel lockfile (non nell'albero attivo corrente),
  RUSTSEC-2023-0071 senza upgrade risolutivo; proc-macro-error2 non mantenuto,
  RUSTSEC-2026-0173. Finding riportati, non ignorati e audit non dichiarato PASS.

## Regola di Lavoro

Leggere questa nota prima di iniziare una nuova fase e aggiornarla dopo ogni
modifica verificata. Non dichiarare completate funzioni solo compilate o
simulate. La memoria persistente Copilot replica i vincoli principali, ma
questa nota e' il riferimento condiviso del progetto.

## Vincoli Concordati

- Rust e' il runtime FiberSpider. Non duplicare Codex nel bot Telegram: usare
  il cabinet agent selezionato e un output strutturato validato da Rust.
- Usare soltanto il PostgreSQL AAA assegnato tramite Yggdrasil. Nessun nuovo
  database locale o di test; non inserire fixture nei dati operativi senza
  autorizzazione separata.
- Telegram usa l'identita' privata del tecnico gia' autorizzato ACTIVE in AAA.
  Non richiede OTP. Il tecnico costruisce rete, installa apparati e sostituisce
  modem sulla stessa ONT/linea, ma non libera porte o scollega clienti.
- Porta assegnata, non FREE o su topologia errata: Rust blocca l'operazione,
  registra l'audit e avvisa il tecnico. Codex non puo' superare questo blocco.
- Foto private conservate su Ragno e catalogate per WR; niente foto, credenziali,
  database o configurazioni private in Git o webroot.
- Amministratore MCP separato dal tecnico: crea reti e gestisce clienti,
  contratti/assegnazioni tramite servizi Rust, transazioni, vincoli e audit.
  Nessuna OTP Telegram o conferma tecnico per questo principal amministrativo.

## Avvio Automatico

Configurato sull'utente Ragno:

- `photaxon-api.service`: API Rust, gia' enabled e running.
- `photaxon-telegram.service`: bot Rust release, enabled e gestito da systemd.
- Linger utente attivato: servizi user avviabili al boot senza login interattivo.
- Yggdrasil risulta enabled e active; configurazione non modificata.
- Bot terminale precedente arrestato per evitare getUpdates concorrenti.

La unit Telegram usa restart-on-failure, umask 0077, NoNewPrivileges, PrivateTmp,
ProtectSystem strict e HOME read-only; scritture ammesse per foto private e
stato Codex. ProtectKernelModules non e' applicabile al gestore user su questo
host: provocava 218/CAPABILITIES. Le altre protezioni restano attive.

Controlli operatore:

```sh
systemctl --user status photaxon-api.service photaxon-telegram.service --no-pager
systemctl --user is-enabled photaxon-api.service photaxon-telegram.service
loginctl show-user ragno -p Linger
journalctl --user -u photaxon-telegram.service --no-pager -n 30
```

Non e' stato eseguito un reboot: autostart configurato e stato corrente
verificabile non equivalgono a una prova di riavvio fisico completata.
L'invocazione reale Codex dalla sandbox systemd resta da verificare durante
un workflow autorizzato, non durante un health check.

## Prossima Fase: MCP ChatGPT

In corso, non completata. Obiettivo:

```text
ChatGPT -> connettore MCP FiberSpider su Ragno -> Yggdrasil -> PostgreSQL AAA
```

- Definire trasporto compatibile con il connettore ChatGPT, endpoint HTTPS
  raggiungibile e autenticazione del principal CONTROL_PLANE_ADMIN.
- Non confondere l'MCP stdio legacy, legato al tecnico, con il futuro
  connettore remoto amministrativo.
- Lettura rete reale; create/update/delete ROE; ONT/modem; feeder/splitter;
  porte/assegnazioni; audit. Mutazioni solo mediante domain services Rust.
- Autorizzazione, idempotenza, transazione e audit devono essere verificati.
- Nessuna apertura porte o pubblicazione gateway senza autorizzazione specifica.
- Quando pronto, il servizio MCP remoto dovra' avere autostart e hardening
  propri. Non creare oggi un servizio placeholder che sembra operativo.
- Consegna finale della fase MCP: endpoint, auth, tools, stato e collegamento
  a ChatGPT; distinguere funzionalita' reali da quelle future.

### Snapshot Admin iniziale 2026-11-08 (storico)

- Schema AAA verificato via Yggdrasil in sola lettura: PostgreSQL 17.7,
  database `fiberspider`, schema `public`; `customers` esiste e contiene
  `customer_id`, `status` e timestamp. Non e' stata inventata una tabella clienti.
- Migrazioni AAA registrate fino a `010_field_media_and_work_orders.sql`.
  `011_control_plane_admin.sql` e' locale e NON applicata.
- Il backup cifrato e' stato tentato ma NON completato: pg_dump locale 16.15
  rifiuta PostgreSQL 17.7; il client 17 non e' nei repository configurati.
  Nessun dump in chiaro e nessuna migrazione sono stati prodotti/applicati.
- Binario di verifica `rust/target/debug/ragno-lia-mcp`: initialize,
  tools/list (17 strumenti), inventory AAA reale, validate/dry-run ROE e ONT
  verificati. Nessuna sostituzione del binario release live.
- MODEM validate passa; il dry-run nega l'associazione perche' l'inventory
  non contiene ONT. Non sono state create ONT operative per forzare un PASS.
- Foto sintetica privata: upload/read/replace e audit locale COMMITTED
  verificati via MCP di verifica; nessuna modifica di foto operative esistenti.
- Servizi admin ROE/ONT/modem compilano; idempotenza e audit usano una sola
  transazione. Le scritture restano negate senza identita' admin verificata.
  Il solo nome CONTROL_PLANE_ADMIN o il trasporto tunnel non autentica Lia.
- Test locali del gate admin e dei validatori/digest passano; Clippy passa.
- CRUD completo, audit/idempotenza DB e AAA WRITE NON verificati/completati.
  Deroga autorizzata il 2026-11-08: test write sul solo AAA operativo con
  transazioni esplicite, ID sintetici univoci, ROLLBACK obbligatorio,
  lock/statement timeout prudenziali e verifica dell'assenza dei record dopo
  il rollback. Vietati nuovi database, COMMIT dei test e modifiche ai dati reali.
  Il backup cifrato e l'autenticazione admin restano prerequisiti obbligatori.
- ChatGPT -> Secure Tunnel -> nuovo MCP Admin -> AAA NON verificato.
  Il tunnel live non e' stato riavviato per questo slice.

Prerequisiti mancanti in quello snapshot: client pg_dump 17 compatibile e backup cifrato verificato,
identita'/autorizzazione admin con un provider autenticato, perimetro di test
AAA sintetico conforme alla deroga e servizi transaction-scoped, accesso al plugin ChatGPT per la
prova end-to-end. Non dichiarare MCP ADMIN READY sulla sola compilazione.

## Prima Verifica Rollback AAA 2026-11-08 (storico)

- Deroga test documentata in AGENTS.md e rust/src/db/AGENTS.md: nessun
  database aggiuntivo, solo ID sintetici e ROLLBACK obbligatorio.
- pg_dump 17.7 compilato dai sorgenti ufficiali verificati SHA-256.
  Backup logico cifrato age completato e catalogo pg_restore verificato;
  archivio privato in secrets/aaa-backups/, nessun dump in chiaro su disco.
- Test opt-in aaa_services_create_update_rollback PASS su AAA via Yggdrasil:
  servizi Rust create ROE/ONT/modem e update sostituzione modem, replay
  idempotente e quattro audit atomici. Timeout lock 2s / statement 8s.
- ROLLBACK esplicito seguito da zero righe sintetiche in cabinet, linee,
  splitter, porte, resource_states, ROE, feeder, allocazioni, ONT, modem e
  collegamenti. Schema admin introdotto solo nel test annullato anch'esso.
  Migrazione 011 NON applicata persistentemente.
- Opt-in admin del launcher compila e richiede il parent tunnel-client
  del profilo fiberspider-ragno; il profilo live NON e' ancora aggiornato.
- AAA WRITE PASS riguarda la prova Rust transazionale autorizzata, non
  ChatGPT end-to-end: nessuna chiamata ChatGPT admin verificata in questa fase.

## Fasi Successive

### Modello completo per Lia e tecnico

Il requisito comprende gestione operativa WR e digital twin fisico, non un
numero prefissato di tool. Un tool e' completo solo con modello persistito,
servizio Rust, permessi, validazione, audit e verifica reale.

- Identita' asset: ID interno separato dal codice/numero operativo di ARLO/CRO,
  ROE/PTE, morsettiere, porte e borchie. Non imporre prefissi interni ai codici
  esterni (esempio codice ROE UDIN123456).
- Localizzazione comune agli oggetti fisici: latitude, longitude, accuracy_m,
  captured_at, captured_by, source, street_name, street_number, postal_code,
  city, address_note. Coordinate assenti non sono coordinate 0,0.
- Splitter: livello primario/secondario, rapporto, contenitore fisico distinto
  dal collegamento ottico, porte IN/OUT e posizione. Tipi di posizione richiesti:
  CRO, CNO, ROE, PTE, MUFFOLA, OTHER. OTHER richiede descrizione; il livello
  secondario non implica automaticamente un contenitore CRO.
- Cavi: feeder/secondario/drop, origine/destinazione, capacita', lunghezza,
  percorso; tubetti e fibre numerati/colorati, allocazioni e giunzioni esplicite.
- Rete edificio: ROE/PTE, morsettiere e porte, montante, derivazioni di piano,
  segmento di terminazione/drop e borchia ottica/OTO.
- Rete cliente: ONT, modem, collegamenti, assegnazioni, misure ottiche,
  collaudo e guasti con evidenze prima/dopo. Modifiche di porta/ONT richiedono
  verifiche dipendenze e occupazione; mai liberare implicitamente una porta.
- Pratiche WR: operatore, cliente e contatti, indirizzo/GPS cliente,
  appuntamenti, tecnico/squadra, stato, note, KO, materiali e documenti.
  Verificare le entita' reali delivery prima di integrarle; i 38 comandi e le
  609 pratiche riferiti per ftth-db non sono stati verificati in questa sessione.
- Foto: WR esistente obbligatoria, media_id, asset, scopo, autore reale,
  timestamp, MIME, dimensione e SHA-256 nel catalogo AAA; bytes privati su RAGNO.
  Upload e sostituzione MCP devono aggiornare il catalogo/versioni e audit:
  una directory con il nome della WR NON sostituisce il legame persistente.
  Il catalogo corrente richiede un workflow/tecnico Telegram: per Lia serve
  attribuzione admin distinta, non un tecnico fittizio.
- Estensioni future: campi extra tipizzati/versionati e validati per tipo asset,
  senza sostituire con JSON libero i legami, lo stato, l'indirizzo o le capacita'.
- Catena da validare e tracciare: ARLO/CRO -> splitter primario -> eventuale
  splitter secondario -> cavo/fibre -> muffole/giunti -> ROE/PTE -> morsettiera/
  porta -> montante/drop -> borchia -> ONT -> modem -> WR/cliente -> foto/collaudo.
  Il tracciamento deve seguire collegamenti reali, rilevare cicli e riportare
  elementi mancanti, senza inventare segmenti per ottenere una catena completa.

Fonte verificata: Piano Italia 1 Giga, lotto 4, servizi minimi: catena CRO,
splitter primario/secondario, cavi secondari, ROE/morsettiera, terminazione,
borchia. Il testo consultato descrive entrambi gli splitter nel CRO; le
varianti distribuite restano requisito progettuale, non verificato dalle
due pagine consultate. Il modello completo sopra e' obiettivo NON completato.

### Fasi applicative

## Piano Digital Twin FTTH

Questo piano recepisce il requisito completo di Lia. Nessuna fase e' completa
per il solo numero di tool, la presenza di una migrazione o una compilazione.
L'ordine segue le dipendenze: ogni fase deve superare il proprio gate prima
di abilitare le relative scritture nel plugin. I tool elencati sono contratti
richiesti, non una dichiarazione che siano gia' registrati o funzionanti.

### Contratto comune

- Tutti gli asset fisici hanno ID interno e codice operativo distinti, tipo,
  stato, indirizzo strutturato (via, civico, CAP, comune, provincia), note,
  GPS opzionale (latitude, longitude, accuracy_m, geolocated_at, source),
  created_at/updated_at, autore reale e collegamenti alle foto. La posizione
  non nota resta assente; non convertirla in coordinate inventate.
- Le relazioni hanno estremi tipizzati, validita' temporale e storico.
  Codici, seriali, porte IN/OUT, fibre e riferimenti delivery non sono note
  libere. Rapporto splitter = 1:N, con N e capacita' effettiva validati.
- Campi futuri: estensioni versionate, tipizzate e validate per categoria;
  niente JSON libero per sostituire stato, capacita', identita' o topologia.
- READ paginati, WRITE tramite servizi Rust, DRY-RUN senza persistenza.
  Idempotency key obbligatoria per mutazioni; riuso con payload diverso negato.
  Audit con principal, richiesta, prima/dopo, esito e correlation ID;
  commit e audit atomici per dati AAA. Nessun SQL o segreto nei tool.
- Unlink/release/delete richiedono una politica esplicita di dipendenze e
  autorizzazione: mai cancellare storico o liberare porte ancora assegnate.
  rollback_operation e' una compensazione validata/auditata, non un SQL UNDO
  generico e non una garanzia di annullamento di azioni fisiche.

### Fasi e gate

| Fase | Ambito e risultato | Criterio misurabile di completamento | Stato |
| --- | --- | --- | --- |
| 0 | Schema AAA, delivery e baseline | Catalogo tabelle/colonne/FK reale; mapping ID/WR; backup cifrato con catalogo verificato; baseline conteggi/hash e manifest delle migrazioni | Parziale: AAA e backup verificati; delivery da verificare |
| 1 | Principal admin, transazioni e audit | Avvio normale senza write; avvio dal profilo privato autorizzato; test positivi/negativi; replay identico senza duplicati; payload diverso negato; audit prima/dopo; timeout e zero residui dopo rollback | Parziale: servizi e rollback diretto verificati; plugin admin non aggiornato |
| 2 | Identita', indirizzi, GPS e riferimenti | CRUD asset con codice esterno distinto; indirizzo completo/provincia; GPS valido/assente/errato testati; autore e storico; estensioni validate; nearby ordinato per distanza con limiti | Da implementare |
| 3 | ARLO/CRO/CNO e splitter | Cabinet CRUD; primario/secondario e 6 location type; porte IN/OUT; rapporti/capacita'/attenuazione; sostituzione con storico; test cross-cabinet, ciclo e capacita' insufficiente negati | Da completare |
| 4 | Cavi, muffole, vassoi, tubetti e fibre | Feeder/secondario/distribuzione/drop; estremi e percorso; tray/splice; allocazione/rilascio; trace fibra; test doppia allocazione, giunzione incompatibile e superamento capacita' negati | Da implementare oltre feeder base |
| 5 | ROE/PTE, edificio, morsettiere e porte | Codice ROE libero da prefissi interni; indirizzo/GPS; catena splitter/cavo/fibra; terminal block; reserve/assign/release con storico; due richieste concorrenti non assegnano la stessa porta | Da completare |
| 6 | Montanti, piani, drop e OTO | Riser e floor box con scala/piano/fibre; drop origine/destinazione/UI/lunghezza/percorso/ostacoli; borchia stanza/connettore/WR; trace continuo e dipendenze protette | Da implementare |
| 7 | ONT, modem, consegna e installazione | Register/install/replace/move/deactivate/unlink con guardie; ONT-OTO-porta-cliente-WR; PON/LOS/potenza/VLAN se prevista; sostituzioni senza perdita storico o riuso porte occupate | Parziale: register/replace testati direttamente; resto da implementare |
| 8 | Misure, collaudi, assurance e materiali | RX/TX/dBm/attenuazione, strumento/punto/tecnico/data; OTDR documentato; profili validate_power configurati senza soglie inventate; PON/LOS/IP/navigazione/speed test; fault lifecycle e materiali con audit | Da implementare |
| 9 | Delivery ftth-db senza duplicazione | Mapping reale WR/cliente/tecnico/squadra/appuntamenti/documenti/assegnazioni/consegne/notifiche; external reference unica; test lookup, ricerca, calendario/statistiche e indisponibilita' servizio | Da verificare e integrare |
| 10 | Foto e prove collegate a WR/asset | Upload/read/replace/attach/list su WR esistente; catalogo AAA coerente con file privati RAGNO; asset di tutte le categorie; GPS/timestamp/autore/hash/note; versione originale preservata e errori FS/DB testati | Parziale: bot catalogato e lettura WR; upload MCP ancora solo file |
| 11 | Topologia completa e plugin ChatGPT | tools/list effettivo confrontato con matrice; trace_customer_path e validate_topology per tratta completa; READ/DRY-RUN/WRITE/PHOTO tramite ChatGPT -> Tunnel -> RAGNO -> AAA con evidenze e rollback verificato | NON verificato |

### Matrice funzionale richiesta

| Modulo | Campi specifici oltre al contratto comune | Tool/operazioni da coprire |
| --- | --- | --- |
| WR/pratica | Operatore, cliente/telefono, appuntamento, tecnico/squadra, stato, KO, note | create/get/update/close_work, assign_technician, reschedule_work |
| ARLO/CRO | Codice, centrale/area, tipo, splitter installati, porte/capacita' | create/get/update/delete_cabinet, find_nearby_cabinets |
| Splitter | Livello, N, contenitore, ingresso/uscite, attenuazione, sostituito_da | create/get/update_splitter, primary/secondary, attach/replace, list_splitter_ports |
| Muffole/giunti | Tipo/modello, pozzetto/palo/facciata, vassoi, cavi IN/OUT | create/get/update_closure, add_splice_tray, list_splices |
| Cavi FO | Tipo, FO totali, lunghezza, estremi e percorso | create/get/update_feeder_cable, create_fiber_cable, connect/split_cable, get_cable_capacity |
| Tubetti/fibre | Numeri e colori, stato, estremi, giunzioni/porte | allocate/release/splice_fiber, trace_fiber |
| ROE/PTE | Codice, ARLO, splitter primario/porta e secondario/porta/posizione, cavo, edificio/scala/piano, capacita' | create/get/update_ROE, attach_roe_to_cabinet, find_nearby_roes |
| Morsettiere/porte | Numero, posizione, fibra/output, WR/cliente/ONT, stato e storico | create_terminal_block, create/list/update_ROE_ports, check/reserve/assign/release_port |
| Montanti | Edificio/scala, cavo verticale, fibre, scatole piano | create_riser, add_floor_box, trace_riser |
| Derivazioni | Piano/scatola, IN/OUT cliente e fibra | create_floor_distribution, assign_floor_fiber |
| Drop/terminazione | ROE/porta origine, UI destinazione, cavo/fibra/metri, canalina/tubazione/ostacoli | create/update_termination_segment, update_drop_path, close_drop_installation |
| OTO/borchia | Codice, WR/cliente, stanza, posizione, connettore e fibra | create/get/update/replace_oto |
| ONT | Seriale/vendor/modello, ROE/porta/OTO/WR/cliente, PON/LOS/potenza/VLAN | register/install/replace/move/deactivate_ont |
| Modem/router | Seriale/vendor/modello, ONT/WR/cliente, consegna/installazione/config non segreta | register/install/replace/unlink_modem, get_link_history |
| Misure | RX/TX/dBm/attenuazione, power meter/OTDR, punto/data/tecnico | record_optical_measurement, validate_optical_power, attach_otdr_test |
| Collaudo | PON/LOS/IP/navigazione/speed test, esito e note chiusura | run_activation_check, record_service_test |
| GPS | Coordinate, accuratezza, data/sorgente/autore e indirizzo associato | set/update_asset_location, validate_location, find_nearby_assets |
| Foto/prove | media_id, filename/storage_key, tipo prova, WR/asset, GPS/autore/data/hash/note | upload/read/replace/attach_photo, list_photos, list_work_order_photos |
| Materiali | Magazzino, tipo/seriale, assegnazione e reso | register/assign/return_material |
| Assurance | Guasto/LOS/rottura, asset/porta, misure prima/dopo e riparazione | open_fault, diagnose_path, replace_failed_asset, close_fault |
| Delivery | WR, tecnici, squadre, appuntamenti, calendario, documenti, consegne/notifiche/statistiche | Adattatori ai servizi reali ftth-db via ID/WR/external_reference, ricerca e riepiloghi |
| Topologia | Legami fisici/ottici validi, dipendenze, tratto cliente | get_topology, trace_customer_path, validate_topology, dry_run_topology_change |
| Admin/audit | Principal, operation_id, richiesta/hash, before/after, esito | read_audit, get_operation_history, validate_change, compensazione validata |

### Scenario di accettazione sul campo

La richiesta "ROE numero X, ARLO Y, splitter secondario Z OUT-08, cavo
48 FO, tubetto verde/fibra rossa, Via Roma 27, GPS, porta ROE 7, OTO,
ONT e modem sulla WR" deve produrre ID e collegamenti persistiti verificabili.
Prima si risolvono codici e colori nel contesto del cavo; se ambigui o mancanti
si chiede il dato, non si inventa. Verificare lookup tramite codice e ID,
traccia ottica continua, capacita', unicita' assegnazioni e catalogo foto WR.

### Evidenze obbligatorie

- Per ogni operazione: test valido, input errato, relazione mancante,
  conflitto/dipendenza, dry-run senza diff dati, replay idempotente e audit.
- WRITE-test AAA solo secondo la deroga: ID sintetici univoci, transazione
  esplicita con ROLLBACK, lock/statement timeout e zero residui controllati.
  Non basta chiamare un wrapper che esegue COMMIT su un'altra connessione.
- Migrazioni: schema reale preflightato, backup cifrato verificato, nessun
  DB locale, nessuna duplicazione delivery, nessuna cancellazione dati esistenti.
- Foto: catalogo/hash/bytes coerenti; accesso privato; errore non produce un
  falso successo; sostituzione/versione e autore consultabili nell'audit.
- Protocollo: tools/list raccolto dal server finale e rinfrescato nel plugin;
  nomi e schemi confrontati con la matrice, senza tool placeholder.
- End-to-end: almeno una lettura, validazione, dry-run, write con rollback
  e operazione foto chiamati dalla vera app ChatGPT. Test diretto Rust o
  health del tunnel non valgono come CHATGPT END-TO-END PASS.
- Rapporto per fase con esiti PASS/FAIL/NON VERIFICATO e prove sanificate;
  MCP COMPLETO solo quando tutti gli ambiti richiesti e il gate 11 passano.

### Stato foto corrente

Nello snapshot precedente la patch di upload foto admin proposta NON risultava
applicata. Questo stato e' superato da admin_photo_service e migrazione
012_admin_photo_catalog.sql ora LIVE: upload/replace/catalog_existing sono
catalogati su AAA, con versioni e hash. Resta valida la regola di non dichiarare
una foto associata sulla sola presenza della directory con il nome della WR.

## Fasi successive del prodotto

- Commesse attive nel bot, accettazione WR e link sicuro al pannello web.
- OTP soltanto sul pannello web della pratica: sessione distinta da Telegram.
  Dettagli in [Telegram, Commesse e Pannello WR](TELEGRAM-WEB-WR.md).
- Autoattivazione del modem spedito: richiesta futura, non da implementare ora.
- Telemetria reale dei distacchi: serve una sorgente evento GPON; l'alert su
  tentativo di usare porta occupata non dimostra monitoraggio fisico autonomo.

## Pubblicazione

### Collaudo ONE: snapshot verifica precedente al deploy

- Provider ONE separato in modalita' MANUAL_RECORDED: nessuna API, endpoint,
  credenziale o risposta ONE simulata. Solo registrazione del risultato reale
  dichiarato dall'operatore autorizzato; il server non ne certifica l'origine API.
- Modello strutturato test/WR/ONT/seriale/operatore, misure e profili RX locali;
  PRECHECK_PASS non e' ONE_PASS. Codice ONE obbligatorio e univoco per PASS.
- Update WR generico non puo' impostare COMPLETED: deve usare la chiusura ONE,
  che verifica linea cliente/ONT/modem e ultimo risultato con codice.
- Chiusura locale include codice e campi ONE strutturati; delivery esterno
  riceve una patch append-note/merge-fields nella outbox PENDING. Nessun
  collegamento DB-to-DB e nessuna consegna esterna dichiarata riuscita.
- Test locali, Clippy e prova AAA rollback PASS: FAIL nega chiusura positiva,
  codice assente/duplicato, WR/seriale incoerenti negati; replay idempotente,
  22 audit, campi extra preservati e zero residui sintetici dopo rollback.
- tools/list del binario debug verificato: 108 tool, inclusi tutti i dieci
  ONE richiesti piu' read_audit/get_operation_history e profili precheck.
- Migrazione 015_one_acceptance_tests.sql verificata SOLO nella transazione
  di test annullata. Non applicata persistentemente; release/plugin live
  non aggiornati per questa fase. ChatGPT end-to-end resta non verificato.

Le modifiche Git non sono ancora pubblicate. Restano da risolvere perimetro delle
modifiche staged pregresse e advisory cargo-audit su rsa nel lockfile (non nel
target attivo), oltre all'avviso proc-macro-error2 non mantenuto. Mai ignorare
segreti o advisory per dichiarare un gate superato.