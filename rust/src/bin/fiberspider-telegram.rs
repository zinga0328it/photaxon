#![forbid(unsafe_code)]

use fiberspider_api::{
    config::telegram,
    db::pool,
    services::{field_operation_service, field_query_service},
    telegram::{
        auth,
        workflow::{WorkflowDraft, WorkflowKind, WorkflowStatus, WorkflowStep},
        workflow_store,
    },
};
use std::{
    error::Error,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use teloxide::{
    net::Download,
    prelude::*,
    types::{ButtonRequest, KeyboardButton, KeyboardMarkup, PhotoSize},
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = telegram::load()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let pool = pool::connect(&config.database).await?;
    let bot = Bot::new(config.bot_token);
    tracing::info!("FiberSpider Telegram bot polling started");
    teloxide::repl(bot, move |bot: Bot, message: Message| {
        let pool = pool.clone();
        async move {
            handle_message(bot, message, pool).await;
            respond(())
        }
    })
    .await;
    Ok(())
}

async fn handle_message(bot: Bot, message: Message, pool: sqlx::PgPool) {
    let Some(user) = message.from.as_ref() else {
        return;
    };
    let user_id = user.id.0.to_string();
    let chat_id = message.chat.id.0.to_string();
    let text = message.text().map(str::trim).unwrap_or_default();
    let session = match auth::authenticate_telegram_user(&pool, &user_id, &chat_id).await {
        Ok(session) => session,
        Err(auth::TechnicianAuthError::Denied) => {
            let _ = bot
                .send_message(message.chat.id, "Accesso negato: usa la chat privata con un account Telegram tecnico autorizzato.")
                .await;
            return;
        }
        Err(_) => {
            let _ = bot
                .send_message(message.chat.id, "Autorizzazione tecnico non disponibile.")
                .await;
            return;
        }
    };
    if text == "/start" || text == "/menu" {
        let _ = bot
            .send_message(message.chat.id, "FiberSpider. Menu operativo.")
            .reply_markup(main_keyboard())
            .await;
        return;
    }
    if text == "/otp" || text.starts_with("/otp ") || text == "🔐 Richiedi OTP" {
        let _ = bot
            .send_message(message.chat.id, "Nel bot non serve OTP. L'OTP sarà riservata all'accesso al pannello web della pratica.")
            .reply_markup(main_keyboard())
            .await;
        return;
    }
    let update_id = i64::from(message.id.0);
    match workflow_store::claim_telegram_update(&pool, &session.session_id, &chat_id, update_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => return,
        Err(_) => {
            let _ = bot
                .send_message(
                    message.chat.id,
                    "Impossibile registrare l'aggiornamento Telegram.",
                )
                .await;
            return;
        }
    }
    if text == "🏠 Menu principale" {
        let _ = workflow_store::cancel_open(&pool, &session.session_id).await;
        let _ = bot
            .send_message(message.chat.id, "Menu principale.")
            .reply_markup(main_keyboard())
            .await;
        return;
    }
    if text == "❌ Annulla" {
        let cancelled = workflow_store::cancel_open(&pool, &session.session_id)
            .await
            .unwrap_or(false);
        let reply = if cancelled {
            "Bozza annullata."
        } else {
            "Nessuna bozza attiva."
        };
        let _ = bot
            .send_message(message.chat.id, reply)
            .reply_markup(main_keyboard())
            .await;
        return;
    }
    if text == "📋 Intervento corrente" {
        match workflow_store::load_open(&pool, &session.session_id).await {
            Ok(Some(draft)) => {
                if draft.kind == WorkflowKind::SearchAsset {
                    if let Some(query) = draft
                        .fields
                        .get("search_query")
                        .and_then(serde_json::Value::as_str)
                    {
                        let report = field_query_service::search(&pool, query).await;
                        let _ = workflow_store::cancel_open(&pool, &session.session_id).await;
                        match report {
                            Ok(report) => send_report(&bot, message.chat.id, report).await,
                            Err(_) => {
                                let _ = bot
                                    .send_message(message.chat.id, "Ricerca non disponibile.")
                                    .await;
                            }
                        }
                    } else {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                "Invia il codice, seriale o indirizzo da cercare.",
                            )
                            .reply_markup(option_keyboard(Vec::new()))
                            .await;
                    }
                    return;
                }
                let message_text = format!(
                    "Tecnico: {}\nIntervento: {}\nStato: {:?}\nPasso: {:?}\nVersione: {}\n\n{}",
                    session.technician_id,
                    workflow_label(draft.kind),
                    draft.status,
                    draft.step,
                    draft.version,
                    review_fields(&draft),
                );
                let keyboard = if draft.status == WorkflowStatus::Review {
                    review_keyboard()
                } else {
                    match keyboard_for_step(&pool, &draft).await {
                        Ok(keyboard) => keyboard,
                        Err(_) => {
                            let _ = bot
                                .send_message(
                                    message.chat.id,
                                    "Intervento caricato, ma le opzioni correnti non sono disponibili.",
                                )
                                .await;
                            return;
                        }
                    }
                };
                let _ = bot
                    .send_message(message.chat.id, message_text)
                    .reply_markup(keyboard)
                    .await;
            }
            Ok(None) => {
                let _ = bot
                    .send_message(message.chat.id, "Nessun intervento aperto.")
                    .reply_markup(main_keyboard())
                    .await;
            }
            Err(_) => {
                let _ = bot
                    .send_message(message.chat.id, "Archivio interventi non disponibile.")
                    .await;
            }
        }
        return;
    }
    let report = match text {
        "🔗 Collegamenti" => Some(field_query_service::connections(&pool).await),
        "🏢 Armadi / Stato armadio" | "📊 Stato armadio" => {
            Some(field_query_service::cabinet_status(&pool).await)
        }
        "🗺 Posizione / Mappa" => Some(field_query_service::roe_locations(&pool).await),
        _ => None,
    };
    if let Some(report) = report {
        match report {
            Ok(report) => send_report(&bot, message.chat.id, report).await,
            Err(_) => {
                let _ = bot
                    .send_message(message.chat.id, "Dati operativi non disponibili.")
                    .await;
            }
        }
        return;
    }
    if text == "🧠 Verifica Codex" {
        match workflow_store::load_open(&pool, &session.session_id).await {
            Ok(Some(draft))
                if draft.kind == WorkflowKind::NearbyRoe
                    && draft.step == WorkflowStep::VerifyCabinetAgent
                    && draft.status == WorkflowStatus::Draft =>
            {
                match field_operation_service::evaluate_nearby_selection(
                    &pool,
                    &session.session_id,
                    draft.version,
                    draft.clone(),
                )
                .await
                {
                    Ok((draft, decision)) => {
                        let status_text = format!(
                            "Cabinet agent {}: {}\nRischio: {:?}\nMotivo: {}\n\n{}",
                            decision.agent_id,
                            codex_decision_label(&decision.decision),
                            decision.risk,
                            decision.reason_code,
                            selected_candidate_report(&draft),
                        );
                        let keyboard = if draft.status == WorkflowStatus::Review {
                            review_keyboard()
                        } else {
                            main_keyboard()
                        };
                        let _ = bot
                            .send_message(message.chat.id, status_text)
                            .reply_markup(keyboard)
                            .await;
                    }
                    Err(_) => {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                "Verifica non disponibile o snapshot cambiato. Controlla l'intervento corrente.",
                            )
                            .reply_markup(main_keyboard())
                            .await;
                    }
                }
            }
            Ok(_) => {
                let _ = bot
                    .send_message(
                        message.chat.id,
                        "Non c'è una selezione ROE in attesa di verifica.",
                    )
                    .reply_markup(main_keyboard())
                    .await;
            }
            Err(_) => {
                let _ = bot
                    .send_message(message.chat.id, "Archivio interventi non disponibile.")
                    .await;
            }
        }
        return;
    }
    if text == "✅ Chiudi intervento" {
        let response = match workflow_store::close_intervention(&pool, &session.session_id).await {
            Ok(true) => "Intervento chiuso e sessione tecnica terminata.".to_owned(),
            Ok(false) => "Intervento già chiuso.".to_owned(),
            Err(workflow_store::WorkflowStoreError::Conflict) => {
                "Chiusura negata: completa o annulla prima l'intervento aperto.".to_owned()
            }
            Err(_) => "Impossibile chiudere l'intervento in questo momento.".to_owned(),
        };
        let _ = bot.send_message(message.chat.id, response).await;
        return;
    }
    if text == "⛓ Scollega modem" {
        let _ = bot
            .send_message(
                message.chat.id,
                "BLOCCATO — scollegamento riservato all'amministratore.",
            )
            .await;
        return;
    }
    if let Some(kind) = workflow_kind(text) {
        match workflow_store::create(&pool, &session.session_id, &session.technician_id, kind).await
        {
            Ok(draft) => match keyboard_for_step(&pool, &draft).await {
                Ok(keyboard) => {
                    let _ = bot
                        .send_message(message.chat.id, prompt_for_step(draft.step))
                        .reply_markup(keyboard)
                        .await;
                }
                Err(_) => {
                    let _ = bot
                        .send_message(message.chat.id, "Elenco delle risorse non disponibile.")
                        .await;
                }
            },
            Err(_) => {
                let _ = bot
                    .send_message(message.chat.id, "Impossibile avviare l'intervento.")
                    .await;
            }
        }
        return;
    }
    if text == "✅ Conferma operazione" {
        let response = match workflow_store::load_open(&pool, &session.session_id).await {
            Ok(Some(draft)) if draft.status == WorkflowStatus::Review => {
                match field_operation_service::confirm_workflow(
                    &pool,
                    &session.session_id,
                    &draft.workflow_id,
                )
                .await
                {
                    Ok(resource_id) => format!("Operazione confermata: {resource_id}"),
                    Err("GPON_PORT_OCCUPIED") => {
                        "ALERT GPON: porta ottica non libera o assegnata. Provisioning bloccato; nessun ONT è stato allineato. Verifica la pratica e la linea del cliente.".to_owned()
                    }
                    Err(_) => {
                        "Operazione non applicata: verifica sessione, stato e dati della bozza."
                            .to_owned()
                    }
                }
            }
            _ => "Non c'è una bozza pronta per la conferma.".to_owned(),
        };
        let _ = bot
            .send_message(message.chat.id, response)
            .reply_markup(main_keyboard())
            .await;
        return;
    }

    match workflow_store::load_open(&pool, &session.session_id).await {
        Ok(Some(mut draft)) => {
            if draft.kind == WorkflowKind::SearchAsset {
                let query = if draft.status == WorkflowStatus::Draft {
                    let expected_version = draft.version;
                    if draft.accept_text(text).is_err()
                        || workflow_store::save(
                            &pool,
                            &session.session_id,
                            &draft,
                            expected_version,
                        )
                        .await
                        .is_err()
                    {
                        let _ = bot
                            .send_message(message.chat.id, "Termine di ricerca non valido.")
                            .await;
                        return;
                    }
                    draft.fields["search_query"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                } else {
                    draft.fields["search_query"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                };
                let report = field_query_service::search(&pool, &query).await;
                let _ = workflow_store::cancel_open(&pool, &session.session_id).await;
                match report {
                    Ok(report) => send_report(&bot, message.chat.id, report).await,
                    Err(_) => {
                        let _ = bot
                            .send_message(message.chat.id, "Ricerca non disponibile.")
                            .await;
                    }
                }
                return;
            }
            if matches!(
                draft.step,
                WorkflowStep::RequestRoePhoto | WorkflowStep::RequestModemPhoto
            ) {
                let Some(photos) = message.photo() else {
                    let _ = bot
                        .send_message(message.chat.id, prompt_for_step(draft.step))
                        .await;
                    return;
                };
                let (temporary_path, image_bytes) =
                    match download_telegram_photo(&bot, photos).await {
                        Ok(photo) => photo,
                        Err(_) => {
                            let _ = bot
                                .send_message(message.chat.id, "Foto non valida o non scaricabile.")
                                .await;
                            return;
                        }
                    };
                let work_order_id = draft.fields["work_order_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                let (resource_type, resource_ref, purpose) = match draft.step {
                    WorkflowStep::RequestRoePhoto => (
                        "ROE",
                        draft.fields["roe_id"].as_str().unwrap_or_default(),
                        "ROE_INSTALLATION",
                    ),
                    WorkflowStep::RequestModemPhoto => (
                        "MODEM",
                        draft.fields["serial"].as_str().unwrap_or_default(),
                        "MODEM_DELIVERY",
                    ),
                    _ => unreachable!(),
                };
                let stored_media = fiberspider_api::services::media_store::store_telegram_photo(
                    &pool,
                    fiberspider_api::services::media_store::PhotoMetadata {
                        workflow_id: &draft.workflow_id,
                        work_order_id: &work_order_id,
                        technician_id: &session.technician_id,
                        resource_type,
                        resource_ref,
                        purpose,
                        content_type: "image/jpeg",
                    },
                    image_bytes,
                )
                .await;
                remove_temporary_photo(&temporary_path);
                let stored_media = match stored_media {
                    Ok(media) => media,
                    Err(_) => {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                "Archiviazione foto su Ragno non riuscita.",
                            )
                            .await;
                        return;
                    }
                };
                let expected_version = draft.version;
                if draft.accept_media(&stored_media.media_id).is_err()
                    || workflow_store::save(&pool, &session.session_id, &draft, expected_version)
                        .await
                        .is_err()
                {
                    let _ = bot
                        .send_message(
                            message.chat.id,
                            "Foto archiviata, ma draft WR non salvabile.",
                        )
                        .await;
                    return;
                }
                let _ = bot
                    .send_message(
                        message.chat.id,
                        format!("Foto archiviata su Ragno per WR {work_order_id}. Rivedi i dati."),
                    )
                    .reply_markup(review_keyboard())
                    .await;
                return;
            }
            if draft.kind == WorkflowKind::RegisterOnt
                && draft.step == WorkflowStep::RequestOntPhoto
            {
                let Some(photo) = message
                    .photo()
                    .and_then(|photos| photos.iter().max_by_key(|photo| photo.file.size))
                else {
                    let _ = bot
                        .send_message(message.chat.id, prompt_for_step(draft.step))
                        .await;
                    return;
                };
                if photo.file.size as usize > 20 * 1024 * 1024 {
                    let _ = bot
                        .send_message(message.chat.id, "Foto troppo grande: limite 20 MB.")
                        .await;
                    return;
                }
                let remote_file = match bot.get_file(photo.file.id.clone()).await {
                    Ok(file) => file,
                    Err(_) => {
                        let _ = bot
                            .send_message(message.chat.id, "Foto Telegram non scaricabile.")
                            .await;
                        return;
                    }
                };
                let temporary_path =
                    std::env::temp_dir().join(format!("fiberspider-ont-{}.jpg", Uuid::new_v4()));
                let temporary_file = match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&temporary_path)
                {
                    Ok(file) => tokio::fs::File::from_std(file),
                    Err(_) => {
                        let _ = bot
                            .send_message(message.chat.id, "Storage temporaneo non disponibile.")
                            .await;
                        return;
                    }
                };
                let mut temporary_file = temporary_file;
                if bot
                    .download_file(&remote_file.path, &mut temporary_file)
                    .await
                    .is_err()
                    || temporary_file.flush().await.is_err()
                {
                    let _ = tokio::fs::remove_file(&temporary_path).await;
                    let _ = bot
                        .send_message(message.chat.id, "Download foto Telegram non riuscito.")
                        .await;
                    return;
                }
                drop(temporary_file);
                let image_size = match tokio::fs::metadata(&temporary_path).await {
                    Ok(metadata) if metadata.len() <= 20 * 1024 * 1024 => metadata.len(),
                    _ => {
                        let _ = tokio::fs::remove_file(&temporary_path).await;
                        let _ = bot
                            .send_message(message.chat.id, "Foto non valida o superiore a 20 MB.")
                            .await;
                        return;
                    }
                };
                let image_bytes = match tokio::fs::read(&temporary_path).await {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        let _ = tokio::fs::remove_file(&temporary_path).await;
                        let _ = bot
                            .send_message(message.chat.id, "Lettura foto non riuscita.")
                            .await;
                        return;
                    }
                };
                let roe_id = draft.fields["roe_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                let work_order_id = draft.fields["work_order_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                let label = fiberspider_api::llm::cabinet_agent::extract_ont_label(
                    &pool,
                    &roe_id,
                    &temporary_path,
                )
                .await;
                let _ = tokio::fs::remove_file(&temporary_path).await;
                let resource_ref = label
                    .as_ref()
                    .map(|label| label.serial.as_str())
                    .unwrap_or("ONT_LABEL_UNPARSED");
                let stored_media = fiberspider_api::services::media_store::store_telegram_photo(
                    &pool,
                    fiberspider_api::services::media_store::PhotoMetadata {
                        workflow_id: &draft.workflow_id,
                        work_order_id: &work_order_id,
                        technician_id: &session.technician_id,
                        resource_type: "ONT",
                        resource_ref,
                        purpose: "ONT_LABEL",
                        content_type: "image/jpeg",
                    },
                    image_bytes,
                )
                .await;
                let stored_media = match stored_media {
                    Ok(media) => media,
                    Err(_) => {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                "Archiviazione foto su Ragno non riuscita.",
                            )
                            .await;
                        return;
                    }
                };
                let label = match label {
                    Ok(label) => label,
                    Err(_) => {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                format!("Foto archiviata per WR {work_order_id}, ma seriale/GPON non leggibili. Invia una nuova foto."),
                            )
                            .await;
                        return;
                    }
                };
                if let Err(error) = field_operation_service::check_ont_gpon_port(
                    &pool,
                    &session.session_id,
                    &draft.workflow_id,
                    &roe_id,
                    &label.gpon_port_id,
                    &label.serial,
                )
                .await
                {
                    let message_text = if error == "GPON_PORT_OCCUPIED" {
                        format!(
                            "ALERT GPON: {} è occupata o assegnata a un altro utente. Nessun provisioning avviato; foto archiviata per WR {}.",
                            label.gpon_port_id, work_order_id
                        )
                    } else {
                        "Verifica GPON non disponibile; nessun provisioning avviato.".to_owned()
                    };
                    let _ = bot.send_message(message.chat.id, message_text).await;
                    if error == "GPON_PORT_OCCUPIED"
                        && let Ok(alert) = field_operation_service::analyze_ont_port_conflict(
                            &pool,
                            &session.session_id,
                            &draft.workflow_id,
                            &work_order_id,
                            &roe_id,
                            &label.gpon_port_id,
                            &label.serial,
                        )
                        .await
                    {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                format!(
                                    "Cabinet agent {}: DENY / {}. {}",
                                    alert.agent_id, alert.reason_code, alert.alert
                                ),
                            )
                            .await;
                    }
                    return;
                }
                let expected_version = draft.version;
                if draft
                    .accept_ont_label(&label.serial, &label.gpon_port_id, label.confidence_milli)
                    .is_err()
                {
                    let _ = bot
                        .send_message(message.chat.id, "Dati seriale/GPON non validi.")
                        .await;
                    return;
                }
                draft.fields.insert(
                    "ont_photo_media_id".into(),
                    serde_json::Value::String(stored_media.media_id),
                );
                if workflow_store::save(&pool, &session.session_id, &draft, expected_version)
                    .await
                    .is_err()
                {
                    let _ = bot
                        .send_message(message.chat.id, "Draft WR non salvabile.")
                        .await;
                    return;
                }
                let _ = bot
                    .send_message(
                        message.chat.id,
                        format!(
                            "Etichetta letta da Codex e foto archiviata su Ragno. Seriale: {}; GPON: {}; confidence: {}%. Controlla prima della conferma.",
                            label.serial,
                            label.gpon_port_id,
                            label.confidence_milli / 10,
                        ),
                    )
                    .reply_markup(keyboard_for_step(&pool, &draft).await.unwrap_or_else(|_| main_keyboard()))
                    .await;
                let _ = image_size;
                return;
            }
            if draft.status == WorkflowStatus::Review {
                let _ = bot
                    .send_message(
                        message.chat.id,
                        format!("Revisione:\n{}", review_fields(&draft)),
                    )
                    .reply_markup(review_keyboard())
                    .await;
                return;
            }
            if is_selection_step(draft.step) {
                match selection_options(&pool, &draft).await {
                    Ok(options) if options.iter().any(|option| option == text) => {}
                    Ok(options) => {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                "Seleziona un valore disponibile nell'elenco.",
                            )
                            .reply_markup(option_keyboard(options))
                            .await;
                        return;
                    }
                    Err(_) => {
                        let _ = bot
                            .send_message(message.chat.id, "Elenco delle risorse non disponibile.")
                            .await;
                        return;
                    }
                }
            }

            let expected_version = draft.version;
            let result = if matches!(
                draft.step,
                WorkflowStep::RequestLocation | WorkflowStep::RequestNearbyLocation
            ) {
                if let Some(location) = message.location() {
                    draft.accept_location(
                        location.latitude,
                        location.longitude,
                        location.horizontal_accuracy,
                    )
                } else {
                    let _ = bot
                        .send_message(message.chat.id, "Condividi la posizione dal pulsante.")
                        .reply_markup(location_keyboard())
                        .await;
                    return;
                }
            } else {
                draft.accept_text(text)
            };
            match result {
                Ok(step) => {
                    if draft.kind == WorkflowKind::NearbyRoe
                        && step == WorkflowStep::VerifyCabinetAgent
                    {
                        if workflow_store::save(
                            &pool,
                            &session.session_id,
                            &draft,
                            expected_version,
                        )
                        .await
                        .is_err()
                        {
                            let _ = bot
                                .send_message(
                                    message.chat.id,
                                    "Non è stato possibile salvare la selezione.",
                                )
                                .await;
                            return;
                        }
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                format!(
                                    "ROE selezionato. Il draft è salvato; avvia la verifica del cabinet agent.\n\n{}",
                                    selected_candidate_report(&draft)
                                ),
                            )
                            .reply_markup(verification_keyboard())
                            .await;
                        return;
                    }
                    if draft.kind == WorkflowKind::NearbyRoe
                        && step == WorkflowStep::SelectNearbyRoe
                    {
                        let latitude = draft.fields["technician_latitude"]
                            .as_f64()
                            .unwrap_or_default();
                        let longitude = draft.fields["technician_longitude"]
                            .as_f64()
                            .unwrap_or_default();
                        match field_query_service::nearby_roes(&pool, latitude, longitude, 10_000.0)
                            .await
                        {
                            Ok(candidates) => {
                                if draft.set_nearby_candidates(&candidates).is_err() {
                                    let _ = bot
                                        .send_message(
                                            message.chat.id,
                                            "Impossibile salvare i candidati.",
                                        )
                                        .await;
                                    return;
                                }
                            }
                            Err(_) => {
                                let _ = bot
                                    .send_message(
                                        message.chat.id,
                                        "Ricerca dei ROE vicini non disponibile.",
                                    )
                                    .await;
                                return;
                            }
                        }
                    }
                    if workflow_store::save(&pool, &session.session_id, &draft, expected_version)
                        .await
                        .is_err()
                    {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                "Bozza aggiornata altrove o sessione scaduta.",
                            )
                            .await;
                        return;
                    }
                    if step == WorkflowStep::Review {
                        let _ = bot
                            .send_message(
                                message.chat.id,
                                format!("Revisione:\n{}", review_fields(&draft)),
                            )
                            .reply_markup(review_keyboard())
                            .await;
                    } else {
                        match keyboard_for_step(&pool, &draft).await {
                            Ok(keyboard) => {
                                let prompt = if step == WorkflowStep::SelectNearbyRoe {
                                    nearby_candidate_report(&draft)
                                } else {
                                    prompt_for_step(step).to_owned()
                                };
                                let _ = bot
                                    .send_message(message.chat.id, prompt)
                                    .reply_markup(keyboard)
                                    .await;
                            }
                            Err(_) => {
                                let _ = bot
                                    .send_message(
                                        message.chat.id,
                                        "Elenco delle risorse non disponibile.",
                                    )
                                    .await;
                            }
                        }
                    }
                }
                Err(_) => {
                    let _ = bot
                        .send_message(message.chat.id, prompt_for_step(draft.step))
                        .await;
                }
            }
        }
        Ok(None) => {
            let _ = bot
                .send_message(message.chat.id, "Scegli un'operazione dal menu.")
                .reply_markup(main_keyboard())
                .await;
        }
        Err(_) => {
            let _ = bot
                .send_message(message.chat.id, "Archivio interventi non disponibile.")
                .await;
        }
    }
}

fn workflow_kind(text: &str) -> Option<WorkflowKind> {
    Some(match text {
        "🏗 Nuovo ROE" => WorkflowKind::CreateRoe,
        "🔌 Registra ONT" => WorkflowKind::RegisterOnt,
        "📡 Registra modem" => WorkflowKind::RegisterModem,
        "🔄 Sostituisci modem" => WorkflowKind::ReplaceModem,
        "🔎 Cerca apparato" => WorkflowKind::SearchAsset,
        "📍 Cerca ROE vicino" => WorkflowKind::NearbyRoe,
        _ => return None,
    })
}

fn workflow_label(kind: WorkflowKind) -> &'static str {
    match kind {
        WorkflowKind::CreateRoe => "Nuovo ROE",
        WorkflowKind::RegisterOnt => "Registra ONT",
        WorkflowKind::RegisterModem => "Registra modem",
        WorkflowKind::ReplaceModem => "Sostituisci modem",
        WorkflowKind::UnlinkModem => "Scollega modem",
        WorkflowKind::SearchAsset => "Cerca apparato",
        WorkflowKind::NearbyRoe => "Cerca ROE vicino",
    }
}

fn prompt_for_step(step: WorkflowStep) -> &'static str {
    match step {
        WorkflowStep::EnterWorkOrderId => "Inserisci l'ID WR della pratica.",
        WorkflowStep::SelectCabinet => "Seleziona l'armadio.",
        WorkflowStep::SelectSplitter => "Seleziona lo splitter.",
        WorkflowStep::EnterRoeId => "Inserisci l'ID del nuovo ROE.",
        WorkflowStep::EnterStreetName => "Inserisci la via.",
        WorkflowStep::EnterStreetNumber => "Inserisci il numero civico.",
        WorkflowStep::EnterCity => "Inserisci il comune.",
        WorkflowStep::EnterPostalCode => "Inserisci il CAP.",
        WorkflowStep::EnterAddressNote => {
            "Inserisci una nota indirizzo oppure '-' se non necessaria."
        }
        WorkflowStep::SelectCableType => "Seleziona la capacità del feeder.",
        WorkflowStep::EnterFiberCount => "Inserisci il numero di fibre (1-512).",
        WorkflowStep::EnterCableId => "Inserisci l'ID del cavo feeder.",
        WorkflowStep::RequestLocation => "Condividi la posizione GPS del ROE.",
        WorkflowStep::RequestRoePhoto => "Invia la foto del ROE installato.",
        WorkflowStep::RequestNearbyLocation => "Condividi la posizione per cercare ROE vicini.",
        WorkflowStep::RequestModemPhoto => "Invia la foto del modem consegnato.",
        WorkflowStep::SelectRoe => "Seleziona il ROE.",
        WorkflowStep::RequestOntPhoto => "Invia la foto dell'etichetta ONT.",
        WorkflowStep::SelectOnt => "Seleziona l'ONT.",
        WorkflowStep::SelectCurrentModem => "Seleziona il modem attuale.",
        WorkflowStep::SelectNearbyRoe => "Seleziona uno dei ROE vicini.",
        WorkflowStep::VerifyCabinetAgent => "Verifica del cabinet agent in corso.",
        WorkflowStep::EnterSerial => "Inserisci il seriale.",
        WorkflowStep::EnterVendor => "Inserisci il produttore oppure '-' se non disponibile.",
        WorkflowStep::EnterModel => "Inserisci il modello oppure '-' se non disponibile.",
        WorkflowStep::EnterUnlinkReason => "Inserisci il motivo dello scollegamento.",
        WorkflowStep::EnterSearchQuery => "Inserisci ROE ID, seriale ONT/modem o indirizzo.",
        WorkflowStep::Review => "Controlla i dati prima della conferma.",
        WorkflowStep::Done => "Intervento completato.",
        WorkflowStep::Cancelled => "Intervento annullato.",
    }
}

fn is_selection_step(step: WorkflowStep) -> bool {
    matches!(
        step,
        WorkflowStep::SelectCabinet
            | WorkflowStep::SelectSplitter
            | WorkflowStep::SelectRoe
            | WorkflowStep::SelectOnt
            | WorkflowStep::SelectCurrentModem
            | WorkflowStep::SelectNearbyRoe
    )
}

async fn selection_options(
    pool: &sqlx::PgPool,
    draft: &WorkflowDraft,
) -> Result<Vec<String>, sqlx::Error> {
    match draft.step {
        WorkflowStep::SelectCabinet => {
            sqlx::query_scalar(
                "SELECT cabinet_id FROM cabinets WHERE status='ACTIVE' ORDER BY cabinet_id",
            )
            .fetch_all(pool)
            .await
        }
        WorkflowStep::SelectSplitter => {
            let Some(cabinet_id) = draft
                .fields
                .get("cabinet_id")
                .and_then(serde_json::Value::as_str)
            else {
                return Ok(Vec::new());
            };
            sqlx::query_scalar(
                "SELECT splitter_id FROM splitters WHERE cabinet_id=$1 AND status='ACTIVE' ORDER BY splitter_id",
            )
            .bind(cabinet_id)
            .fetch_all(pool)
            .await
        }
        WorkflowStep::SelectRoe => {
            sqlx::query_scalar("SELECT roe_id FROM roes WHERE status='ACTIVE' ORDER BY roe_id")
                .fetch_all(pool)
                .await
        }
        WorkflowStep::SelectOnt => {
            sqlx::query_scalar("SELECT ont_id FROM onts WHERE status='ACTIVE' ORDER BY ont_id")
                .fetch_all(pool)
                .await
        }
        WorkflowStep::SelectCurrentModem => {
            let Some(ont_id) = draft
                .fields
                .get("ont_id")
                .and_then(serde_json::Value::as_str)
            else {
                return Ok(Vec::new());
            };
            sqlx::query_scalar(
                "SELECT m.modem_id FROM ont_modem_links l JOIN modems m ON m.modem_id=l.modem_id WHERE l.ont_id=$1 AND l.status='ACTIVE' AND m.status='ACTIVE' ORDER BY m.modem_id",
            )
            .bind(ont_id)
            .fetch_all(pool)
            .await
        }
        WorkflowStep::SelectNearbyRoe => Ok(draft
            .fields
            .get("nearby_candidates")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|candidate| candidate.get("roe_id")?.as_str().map(str::to_owned))
            .collect()),
        _ => Ok(Vec::new()),
    }
}

async fn keyboard_for_step(
    pool: &sqlx::PgPool,
    draft: &WorkflowDraft,
) -> Result<KeyboardMarkup, sqlx::Error> {
    if is_selection_step(draft.step) {
        return Ok(option_keyboard(selection_options(pool, draft).await?));
    }
    Ok(match draft.step {
        WorkflowStep::SelectCableType => KeyboardMarkup::new(vec![
            vec![KeyboardButton::new("12 FO"), KeyboardButton::new("24 FO")],
            vec![KeyboardButton::new("48 FO"), KeyboardButton::new("Altro")],
            vec![
                KeyboardButton::new("❌ Annulla"),
                KeyboardButton::new("🏠 Menu principale"),
            ],
        ])
        .resize_keyboard(),
        WorkflowStep::RequestLocation | WorkflowStep::RequestNearbyLocation => location_keyboard(),
        WorkflowStep::VerifyCabinetAgent => verification_keyboard(),
        _ => main_keyboard(),
    })
}

fn option_keyboard(options: Vec<String>) -> KeyboardMarkup {
    let mut rows: Vec<Vec<KeyboardButton>> = options
        .chunks(2)
        .map(|pair| pair.iter().map(KeyboardButton::new).collect())
        .collect();
    rows.push(vec![
        KeyboardButton::new("❌ Annulla"),
        KeyboardButton::new("🏠 Menu principale"),
    ]);
    KeyboardMarkup::new(rows).resize_keyboard()
}

fn location_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![
        KeyboardButton::new("📍 Condividi posizione").request(ButtonRequest::Location),
        KeyboardButton::new("❌ Annulla"),
    ]])
    .resize_keyboard()
}

fn review_fields(draft: &WorkflowDraft) -> String {
    draft
        .fields
        .iter()
        .map(|(key, value)| {
            let label = match key.as_str() {
                "cabinet_id" => "Armadio",
                "splitter_id" => "Splitter",
                "roe_id" => "ROE",
                "street_name" => "Via",
                "street_number" => "Numero civico",
                "city" => "Comune",
                "postal_code" => "CAP",
                "address_note" => "Nota indirizzo",
                "feeder_fiber_count" => "Capacità feeder",
                "feeder_cable_id" => "Cavo feeder",
                "fibers_allocated" => "Fibre assegnate",
                "latitude" => "Latitudine",
                "longitude" => "Longitudine",
                "location_accuracy_m" => "Precisione GPS (m)",
                "geolocation_source" => "Fonte GPS",
                "technician_latitude" => "Latitudine tecnico",
                "technician_longitude" => "Longitudine tecnico",
                "technician_location_accuracy_m" => "Precisione GPS tecnico (m)",
                "ont_id" => "ONT",
                "current_modem_id" => "Modem attuale",
                "serial" => "Seriale",
                "vendor" => "Produttore",
                "model" => "Modello",
                "reason" => "Motivo",
                "candidate_roe_id" => "ROE selezionato",
                "roe_proposal_id" => "Proposta ROE",
                "nearby_inventory_fingerprint" => "Fingerprint inventario",
                "codex_decision" => "Decisione Codex",
                _ => key.as_str(),
            };
            let display_value = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            format!("{label}: {display_value}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn nearby_candidate_report(draft: &WorkflowDraft) -> String {
    let candidates = draft
        .fields
        .get("nearby_candidates")
        .and_then(serde_json::Value::as_array);
    let Some(candidates) = candidates else {
        return "Snapshot dei candidati non disponibile.".into();
    };
    if candidates.is_empty() {
        return "Nessun ROE attivo geolocalizzato entro 10 km.".into();
    }
    let mut lines =
        vec!["ROE vicini. Seleziona un ID per chiedere la verifica cabinet agent:".to_owned()];
    lines.extend(candidates.iter().map(|candidate| {
        let text = |key| {
            candidate
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("-")
        };
        let distance = candidate
            .get("distance_m")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or_default();
        let capacity = candidate
            .get("available_fibers")
            .and_then(serde_json::Value::as_i64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/d".into());
        format!(
            "{} | {}, {} {} | {} → {} | fibre libere {} | {:.0} m",
            text("roe_id"),
            text("street_name"),
            text("street_number"),
            text("city"),
            text("cabinet_id"),
            text("splitter_id"),
            capacity,
            distance,
        )
    }));
    lines.join("\n")
}

fn selected_candidate_report(draft: &WorkflowDraft) -> String {
    let selected_id = draft
        .fields
        .get("candidate_roe_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-");
    let candidate = draft
        .fields
        .get("nearby_candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| {
            candidates.iter().find(|candidate| {
                candidate.get("roe_id").and_then(serde_json::Value::as_str) == Some(selected_id)
            })
        });
    let Some(candidate) = candidate else {
        return format!("ROE selezionato: {selected_id}; dati candidati non disponibili.");
    };
    let text = |key| {
        candidate
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-")
    };
    let distance = candidate
        .get("distance_m")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_default();
    let capacity = candidate
        .get("available_fibers")
        .and_then(serde_json::Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/d".into());
    format!(
        "ROE selezionato: {selected_id}\n{}, {} {}\nCabinet: {}\nSplitter: {}\nFibre libere: {}\nDistanza: {:.0} m",
        text("street_name"),
        text("street_number"),
        text("city"),
        text("cabinet_id"),
        text("splitter_id"),
        capacity,
        distance,
    )
}

fn review_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![
        KeyboardButton::new("✅ Conferma operazione"),
        KeyboardButton::new("❌ Annulla"),
    ]])
    .resize_keyboard()
}

fn verification_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![
        KeyboardButton::new("🧠 Verifica Codex"),
        KeyboardButton::new("❌ Annulla"),
        KeyboardButton::new("🏠 Menu principale"),
    ]])
    .resize_keyboard()
}

fn codex_decision_label(
    decision: &fiberspider_api::llm::cabinet_agent::CodexDecision,
) -> &'static str {
    use fiberspider_api::llm::cabinet_agent::CodexDecision;
    match decision {
        CodexDecision::Allow => "ALLOW",
        CodexDecision::AllowWithWarning => "ALLOW_WITH_WARNING",
        CodexDecision::Deny => "DENY",
        CodexDecision::NeedsSupervisor => "NEEDS_SUPERVISOR",
    }
}

async fn send_report(bot: &Bot, chat_id: ChatId, report: String) {
    let mut chunk = String::new();
    for line in report.lines() {
        if chunk.len() + line.len() + 1 > 3500 && !chunk.is_empty() {
            if bot.send_message(chat_id, &chunk).await.is_err() {
                return;
            }
            chunk.clear();
        }
        if !chunk.is_empty() {
            chunk.push('\n');
        }
        chunk.push_str(line);
    }
    if !chunk.is_empty() {
        let _ = bot.send_message(chat_id, chunk).await;
    }
}

async fn download_telegram_photo(
    bot: &Bot,
    photos: &[PhotoSize],
) -> Result<(PathBuf, Vec<u8>), &'static str> {
    let photo = photos
        .iter()
        .max_by_key(|photo| photo.file.size)
        .ok_or("Telegram photo is missing")?;
    if photo.file.size as usize > 20 * 1024 * 1024 {
        return Err("Telegram photo exceeds 20 MB");
    }
    let remote_file = bot
        .get_file(photo.file.id.clone())
        .await
        .map_err(|_| "Telegram photo metadata unavailable")?;
    let temporary_path =
        std::env::temp_dir().join(format!("fiberspider-photo-{}.jpg", Uuid::new_v4()));
    let temporary_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(|_| "temporary photo storage unavailable")?;
    let mut temporary_file = tokio::fs::File::from_std(temporary_file);
    if bot
        .download_file(&remote_file.path, &mut temporary_file)
        .await
        .is_err()
        || temporary_file.flush().await.is_err()
    {
        let _ = tokio::fs::remove_file(&temporary_path).await;
        return Err("Telegram photo download failed");
    }
    drop(temporary_file);
    let metadata = tokio::fs::metadata(&temporary_path)
        .await
        .map_err(|_| "downloaded photo is unavailable")?;
    if metadata.len() == 0 || metadata.len() > 20 * 1024 * 1024 {
        let _ = tokio::fs::remove_file(&temporary_path).await;
        return Err("downloaded photo has invalid size");
    }
    let bytes = tokio::fs::read(&temporary_path)
        .await
        .map_err(|_| "downloaded photo cannot be read")?;
    Ok((temporary_path, bytes))
}

fn remove_temporary_photo(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn main_keyboard() -> KeyboardMarkup {
    let rows: Vec<Vec<KeyboardButton>> = [
        ["🏠 Menu principale", "🏗 Nuovo ROE"],
        ["📍 Cerca ROE vicino", "🔌 Registra ONT"],
        ["📡 Registra modem", "🔄 Sostituisci modem"],
        ["🔗 Collegamenti", "🏢 Armadi / Stato armadio"],
        ["🧠 Verifica Codex", "🗺 Posizione / Mappa"],
        ["🔎 Cerca apparato", "❌ Annulla"],
        ["📋 Intervento corrente", "✅ Conferma operazione"],
        ["✅ Chiudi intervento", "🏠 Menu principale"],
    ]
    .into_iter()
    .map(|row| row.into_iter().map(KeyboardButton::new).collect())
    .collect();
    KeyboardMarkup::new(rows).resize_keyboard()
}

#[cfg(test)]
mod tests {
    use super::{main_keyboard, workflow_kind};
    use fiberspider_api::telegram::workflow::WorkflowKind;

    #[test]
    fn runtime_search_and_replacement_are_registered_but_unlink_is_not() {
        assert_eq!(
            workflow_kind("🔎 Cerca apparato"),
            Some(WorkflowKind::SearchAsset)
        );
        assert_eq!(
            workflow_kind("🔄 Sostituisci modem"),
            Some(WorkflowKind::ReplaceModem)
        );
        assert_eq!(workflow_kind("⛓ Scollega modem"), None);
        assert_eq!(workflow_kind("/unlink"), None);
    }

    #[test]
    fn runtime_keyboard_has_no_unlink_and_exposes_registered_workflows() {
        let keyboard = serde_json::to_value(main_keyboard()).unwrap();
        let labels: Vec<_> = keyboard["keyboard"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row.as_array().unwrap())
            .map(|button| button["text"].as_str().unwrap())
            .collect();
        assert!(!labels.contains(&"⛓ Scollega modem"));
        for label in [
            "🏗 Nuovo ROE",
            "🔌 Registra ONT",
            "📡 Registra modem",
            "🔄 Sostituisci modem",
            "📍 Cerca ROE vicino",
            "🔎 Cerca apparato",
        ] {
            assert!(labels.contains(&label));
            assert!(workflow_kind(label).is_some());
        }
    }
}
