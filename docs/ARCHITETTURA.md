# FiberSpider: architettura e stato live

Chiusura sessione 2026-11-08. Runtime Rust, PostgreSQL AAA via Yggdrasil,
MCP privato collegato alla vera app ChatGPT. Questa e' la descrizione corrente,
non una dichiarazione di digital twin o provisioning ottico interamente completati.

## Nodi

| Nodo | Responsabilita' | Verifica corrente |
| --- | --- | --- |
| AAA | PostgreSQL 17.7 autorevole per rete, WR, inventario, audit, collaudi e cataloghi foto | Migrazioni fino a 016; READ e WRITE sintetici con rollback |
| RAGNO | API/bot/MCP Rust, domain services/validatori, cabinet-agent e bytes foto privati | Release live, tunnel connesso, 112 tool visibili da ChatGPT |
| ALEX | Frontend HTTPS/reverse proxy e dashboard GET con scope limitato | Ruolo documentato; non modificato o ricollaudato in questa sessione |
| ALE | Nodo/ruolo citato dall'operatore | Contratto non presente nel workspace: ruolo non verificato, non equiparato ad ALEX |

```text
ChatGPT / Lia -> OpenAI Secure MCP Tunnel (HTTPS outbound da RAGNO)
  -> tunnel-client, profilo privato fiberspider-ragno
  -> ragno-lia-mcp --private-tunnel-admin (stdio, nessun listener pubblico)
  -> servizi Rust -> transazioni/validatori -> Yggdrasil -> AAA

Tecnico -> Telegram privato -> tecnico ACTIVE -> workflow Rust -> AAA
Browser -> HTTPS ALEX -> API Rust RAGNO su Yggdrasil -> AAA
Foto: bytes privati su RAGNO; catalogo/WR/asset/versioni/hash in AAA
```

## Identita' e sicurezza

- Telegram usa identita' privata del tecnico ACTIVE, senza OTP operativo.
  OTP resta previsto per il futuro pannello web WR.
- CONTROL_PLANE_ADMIN e' distinto dal tecnico: il confine approvato per questa
  fase e' il tunnel privato/workspace autorizzato, con opt-in del launcher e
  verifica del padre tunnel-client/profilo. Avvio stdio ordinario senza write.
  Non e' un'identita' individuale di Lia; l'utente locale RAGNO resta fidato.
- Runtime API key solo per il trasporto OpenAI; non credenziale utente MCP.
  Key e credenziali AAA sono locali/ignorate e non vengono stampate o pubblicate.
- Nessun SQL libero, shell, .env, directory privata, traversal o symlink nei
  file tool. ALEX non eredita privilegi admin/MCP.
- Il tecnico non libera porte o clienti. Il cabinet-agent/Codex ragiona su
  dati selezionati e non puo' superare il DENY Rust o scrivere direttamente AAA.

## Tunnel e distribuzione

Client ufficiale OpenAI 0.0.20; profilo locale fiberspider-ragno, key da env,
health/admin su loopback effimero. Un solo runtime per Tunnel ID. Il figlio usa
la release Rust. Il tunnel e' mantenuto aperto in terminale; autostart/hardening
systemd specifici MCP non sono ancora configurati. API e bot hanno unit user proprie.

Deploy: backup binario/profilo e dump AAA cifrato, manifest SHA-256, DDL
compatibile/transazionale, stop vecchio runtime, start nuovo, health e refresh
strumenti nelle impostazioni FiberSpider di ChatGPT. Health o tools/list locale
da soli non provano il plugin aggiornato. Vedere [CHATGPT-MCP.md](CHATGPT-MCP.md).

## Schema AAA

Tabelle legacy mantenute: cabinets, ftth_lines, splitters, ports, roes/ptes,
feeder_cables/allocations, onts, modems, ont_modem_links, customers,
activation_requests, resource_states e resource_leases.

| Migrazione | Ambito |
| --- | --- |
| 007-010 | Workflow Telegram, compatibilita' ONT, guardia GPON e foto WR |
| 011 | Operations/audit/idempotenza admin e attribuzione distinta dal tecnico |
| 012 | Catalogo foto admin e versioni immutabili |
| 013 | Registro asset fisici, dipendenze e assegnazioni |
| 014 | Dettagli WR, squadre/appuntamenti; bozza senza tecnico o ONT |
| 015 | Collaudi ONE, profili precheck e outbox chiusura delivery |
| 016 | Provenienza legacy e metadata_status MEASURED/LEGACY_INCOMPLETE |

Nessun DB locale o secondo DB di test. Deroga: solo ID sintetici univoci,
transazioni esplicite, lock timeout 2s/statement 8s, ROLLBACK e zero residui
verificati. I wrapper normali committano; i test usano le stesse implementazioni
transaction-scoped. DDL di test e' annullato nella medesima transazione.

## Digital twin FTTH

Nuovi asset: ID interno e codice operativo distinti, stato, via/civico/CAP/
comune/provincia/note, GPS opzionale con accuratezza/data/sorgente, autori,
timestamp, revisione e audit. Dati di posizione assenti restano sconosciuti.

| Tipo | Dati/legami implementati |
| --- | --- |
| ARLO/CRO | Tipo armadio, centrale/area, codice e localizzazione |
| Splitter | Livello primario/secondario 1/2, 1:N, contenitore/posizione, attenuazione; CRO/CNO/ROE/PTE/MUFFOLA/OTHER |
| Muffola | Modello, collocazione, numero vassoi |
| Cavo | Feeder/secondario/distribuzione/drop/montante, FO, estremi, metri/percorso |
| Tubetto/fibra | Numeri/colori, cavo/tubetto e capacita' |
| ROE/PTE | Codice, armadio, edificio, scala/posizione e capacita' |
| Morsettiera/porta | Proprietario, numero, direzione, fibra e capacita' |
| Montante/derivazione | Edificio/scala/piani, cavo verticale, fibra ingresso e capacita' |
| Terminazione/drop | Porta ROE, unita' destinazione, cavo/fibra/metri/percorso/canalina/ostacoli |
| OTO | WR/cliente, stanza, connettore, fibra |
| ONT/modem fisici | Seriale/vendor/modello, linea/OTO/WR/cliente e VLAN se prevista |

Create/get/update/dry-run per i 16 tipi: validazione identificativi/revisioni,
relazioni/cicli/capacita'/numeri. Asset assegnati o referenziati non si modificano
arbitrariamente. Reserve/assign/release porte/fibre sono servizi separati;
release attuale annulla solo prenotazioni senza cliente, mai una porta occupata.

Catena target: ARLO -> splitter -> cavi/fibre/muffole -> ROE/PTE/morsettiera/
porta -> montante/drop -> OTO -> ONT/modem -> cliente/WR -> foto/collaudo.
Modelli disponibili non equivalgono a splice, tracciamento ottico completo,
delete/compensazioni o provisioning GPON: queste operazioni restano incomplete.

## Import legacy e compatibilita'

Import singolo/batch parent-first (massimo 1000), query statiche, audit e
idempotenza. Nessuna riga legacy sovrascritta. Live: 51 asset importati,
3 cabinet + 3 splitter + 44 porte + 1 ROE, verificati da ChatGPT e direttamente AAA.

ID/capacita'/provenienza mantenuti; dati sorgente in legacy_source. Campi
mancanti espliciti, metadata_status LEGACY_INCOMPLETE. Non si inventano GPS,
indirizzi o installazioni reali. Il dry-run secondario collegato a CAB-A passa.
Resta da completare arricchimento sicuro di asset referenziati e piena unificazione
dei lifecycle inventory/registro fisico; i due modelli non sono sinonimi.

## ONT e modem

Servizi inventory transazionali: seriali univoci, lock GPON, porta FREE senza
assegnazione cliente/ONT, topologia valida, resource-state e lease coerenti.
Indice unico ONT attiva per GPON. Replace ONT conserva linea/porta/cliente/modem
e storico senza liberare la porta. Replace modem conserva ONT e marca il vecchio
link REPLACED con data fine e link sostitutivo. Non tutti i servizi inventory
hanno gia' tool dedicati: sotto sono elencati solo quelli davvero registrati.

## ONE, misure e chiusura

Installazione -> misura/precheck -> collaudo reale ONE -> esito/codice ->
registrazione -> chiusura. Provider ONE manuale separato: nessuna API, endpoint,
credenziale o risposta simulata. Il server valida quanto registra l'operatore,
non certifica l'origine tramite API.

Profili RX per operatore/vendor/modello solo precheck: PRECHECK_PASS non e'
ONE_PASS. Test strutturato con WR/ONT/seriale/operatore/provider/tipo/date/tecnico,
RX/TX/attenuazione/precheck, stato/codici ONE, KO/note/riferimento e autori/audit.
ONE_PASS richiede codice valido/univoco. FAIL o WR/ONT incoerenti negano chiusura.
Il normale update WR non puo' impostare COMPLETED: serve la chiusura ONE che
verifica ultimo risultato/codice e linea cliente con ONT/modem attivi.

Le note includono il codice e gli extra_fields sono fusi. FiberSpider e ftth-db
delivery esterno restano separati tramite WR/external reference, non DB-to-DB.
Outbox PENDING: note_chiusura_append + extra_fields_patch (stato/codice/test/RX).
Consegna esterna non dichiarata riuscita finche' non esiste adapter documentato.

## Foto RAGNO e catalogo AAA

Bytes privati, directory 0700/files 0600, mai Git/webroot. Upload richiede WR
unica esistente, asset legacy/fisico, tipo prova e operation_id. Max 7 MiB per
compatibilita' tunnel. Catalogo/versioni/MIME/size/hash/autore su AAA; read verifica
hash/size, replace crea nuova versione conservando l'originale.

catalog_existing_ragno_image associa un file gia' presente a WR/asset dichiarati
senza riscriverlo; non inferisce tecnico o WR dal nome. list_ragno_images distingue
CATALOGED/AMBIGUOUS/UNASSOCIATED_REQUIRES_WR_AND_ASSET. list_work_order_photos unisce
foto Telegram e versioni admin correnti. Commit non confermato conserva il file
per riconciliazione: FS/DB non sono una transazione distribuita atomica.
L'unico artefatto MCP-CHECK senza WR e' rimosso su autorizzazione dopo backup;
nessuna foto operativa eliminata. Foto operative vere da ChatGPT ancora da collaudare.

## Elenco MCP: 112 tool

64 tool fisici: create_, get_, update_, dry_run_ per ciascun suffisso:
physical_cabinet, physical_splitter, closure, fiber_cable, tube, fiber,
physical_roe, physical_pte, terminal_block, physical_port, riser,
floor_distribution, termination_segment, oto, physical_ont, physical_modem.

| Gruppo | Nomi effettivi |
| --- | --- |
| Inventory/validazione legacy (8) | get_network_inventory, validate_roe, dry_run_roe, validate_ont, dry_run_ont, validate_modem, dry_run_modem, replace_ont |
| Risorse (6) | reserve_port, assign_port, release_port, reserve_fiber, allocate_fiber, release_fiber |
| Delivery locale (6) | create_work, get_work, update_work, validate_work, get_work_calendar, get_work_statistics |
| ONE (11) | get_ont_test_requirements, validate_ont_for_test, record_optical_measurement, start_ont_acceptance_test, record_one_test_result, get_one_test_result, list_one_tests_for_work, dry_run_one_test_result, validate_work_closure, close_work_with_test_result, set_optical_precheck_profile |
| Audit (2) | read_audit, get_operation_history |
| Import (3) | dry_run_legacy_import, import_legacy_asset, import_legacy_inventory |
| Foto (6) | list_ragno_images, list_work_order_photos, read_ragno_image, upload_ragno_image, replace_ragno_image, catalog_existing_ragno_image |
| File/log/ordini (6) | list_project_files, read_project_file, tail_project_log, read_lia_orders, write_lia_orders, read_mcp_audit |

112 tool mostrati dal plugin vero, con READ, import legacy e dry-run live.
La UI li raggruppa come Write: annotazioni read-only complete ancora da rifinire;
la semantica effettiva resta imposta dal servizio Rust.

## Verifiche e lavoro residuo

Verificati: test Rust/Clippy, release/checksum, backup cifrati/manifest,
migrazioni live, tunnel/discovery ChatGPT, READ AAA, import e dry-run live.
Test sintetici AAA: rete/WR/ONE/foto/versioni/audit/idempotenza, 25 audit,
ROLLBACK e zero residui. Non sono prove di GPON fisico o collaudi reali ONE.

Pendenti: metadati dal campo, lifecycle legacy/fisici completi, splice/trace
ottico completo, delete/compensazioni, editing sicuro asset referenziati,
squadre/materiali/assurance completi, API ONE e adapter delivery esterno,
foto operative reali da ChatGPT, OTP pannello WR, autostart/hardening MCP,
identita' admin OAuth granulare e ruolo ALE.

Cargo audit: RUSTSEC-2023-0071 (rsa 0.9.10 nel lockfile, non nell'albero attivo
corrente della release) e RUSTSEC-2026-0173 (proc-macro-error2 non mantenuto).
Non ignorati/soppressi. Audit non dichiarato PASS; nessuna modifica dipendenze
introdotta durante questa chiusura.