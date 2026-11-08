# Telegram, Commesse e Pannello WR

## Decisione Corrente

Il bot non richiede, genera o verifica OTP nel percorso operativo. L'identita'
arriva da Telegram (`message.from.id`), non da un ID digitato dall'utente.
L'accesso richiede chat privata e tecnico ACTIVE gia' censito in AAA. Nel
modello attuale `technicians.technician_id` coincide con il Telegram user ID.
Username e nome visualizzato non sono credenziali e non autorizzano il tecnico.

`/start` e `/menu` aprono direttamente il menu del tecnico autorizzato. La
sessione operativa e' persistita in AAA, rivalida lo stato del tecnico a ogni
messaggio e conserva i draft tra riavvii. Non imposta un timestamp OTP per
simulare un'autenticazione web. Una WR deve appartenere allo stesso tecnico;
il campo legacy `activation_requests.otp_verified_at` non e' un requisito del bot.

Il tecnico crea e installa risorse e sostituisce il modem sulla stessa ONT e
linea. Non puo' liberare porte, cancellare assegnazioni o scollegare clienti.
Una porta non libera, assegnata o su topologia errata viene bloccata da Rust.
Il control plane amministrativo MCP resta un'identita' distinta: puo' creare
reti e gestire clienti/assegnazioni attraverso servizi Rust, transazioni e
audit. Non eredita la sessione Telegram del tecnico. Il nuovo MCP amministrativo
non e' implementato da questa modifica.

Le primitive OTP e le tabelle legacy non vengono cancellate insieme ai dati
esistenti. Non sono richiamate dal dispatcher Telegram: sono disponibili per
la futura implementazione web, che dovra' avere un confine e challenge separati.

## Futuro, Non Implementato

1. Il bot elenca le commesse attive assegnate al tecnico leggendo AAA.
2. Il tecnico seleziona una WR e accetta la commessa con un pulsante.
3. Rust registra l'accettazione in modo transazionale, auditato e idempotente,
   verificando che la commessa sia ancora attiva e assegnata al tecnico.
4. Il bot genera un link HTTPS al pannello web della WR accettata.
5. Il tecnico apre il pannello e completa la verifica OTP per accedere alla
   pratica. La challenge puo' essere consegnata dal bot alla chat privata
   gia' verificata; il codice viene inserito e verificato nel pannello, non
   nel menu operativo Telegram.
6. Il pannello emette una sessione web sicura limitata al tecnico e alla WR.

Il link usa un nonce casuale, breve scadenza e verifica server-side: non
contiene password, OTP o dati cliente. Conoscere il link non autorizza l'accesso.
La challenge web ha hash salted, TTL, limite tentativi, rate limit e consumo
unico; non riusa una sessione Telegram come prova di OTP. Cookie Secure,
HttpOnly e SameSite, protezione CSRF e rivalidazione di tecnico/assegnazione
sono obbligatori. Annullamento, riassegnazione o chiusura della commessa revocano
l'accesso relativo alla WR.

## Pratica e Allegati

AAA conserva catalogo WR, risorse, topologia, storico e audit. Le foto originali
restano private su Ragno sotto `secrets/field-media/<WR>/`, fuori da Git e da
qualsiasi webroot, con directory 0700 e file 0600. Il catalogo collega media,
WR, workflow, tecnico, tipo risorsa e SHA-256. Il futuro pannello deve mostrare
foto ROE/ONT/modem e la catena cabinet, cavo, splitter, ROE, monofibra/porta,
ONT, modem, applicando le autorizzazioni a ogni accesso.

OCR/visione Codex produce dati proposti, non inventa assegnazioni e non
sostituisce i controlli deterministici Rust. Il rilevamento automatico di
guasti richiede una sorgente reale di eventi rete; l'alert su porta occupata
durante l'installazione non dimostra da solo telemetria GPON operativa.

## Verifica

Questa modifica richiede test di identita' privata, tecnico non autorizzato,
menu senza OTP, sessioni/draft persistiti e ownership WR. Elenco commesse,
accettazione, link e pannello OTP restano futuri: non vengono esposti come
pulsanti operativi prima di avere handler completi.