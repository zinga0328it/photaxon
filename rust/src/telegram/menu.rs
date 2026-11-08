#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    NewRoe,
    RegisterOnt,
    RegisterModem,
    Connections,
    Location,
    Search,
    CurrentIntervention,
    ConfirmOperation,
    CloseIntervention,
    Cancel,
    CabinetStatus,
    ExpectedRoe,
    FindNearbyRoe,
    VerifyWithCabinetAgent,
    ConfirmRoe,
    MainMenu,
}

impl MenuAction {
    pub fn parse(label: &str) -> Option<Self> {
        Some(match label.trim() {
            "🏗 Nuovo ROE" => Self::NewRoe,
            "🔌 Registra ONT" => Self::RegisterOnt,
            "📡 Registra modem" => Self::RegisterModem,
            "🔗 Collegamenti" => Self::Connections,
            "🗺 Posizione / Mappa" => Self::Location,
            "🔎 Cerca" => Self::Search,
            "📋 Intervento corrente" => Self::CurrentIntervention,
            "✅ Conferma operazione" => Self::ConfirmOperation,
            "✅ Chiudi intervento" => Self::CloseIntervention,
            "❌ Annulla" => Self::Cancel,
            "📊 Stato armadio" => Self::CabinetStatus,
            "📦 ROE previsto" => Self::ExpectedRoe,
            "📍 Cerca ROE vicino" => Self::FindNearbyRoe,
            "🧠 Verifica con AGENT-CAB-X" => Self::VerifyWithCabinetAgent,
            "✅ Conferma ROE" => Self::ConfirmRoe,
            "🏠 Menu principale" => Self::MainMenu,
            _ => return None,
        })
    }
}

pub fn main_menu_labels(cabinet_ids: &[String]) -> Vec<String> {
    let mut labels = vec![
        "🏗 Nuovo ROE".to_owned(),
        "🔌 Registra ONT".to_owned(),
        "📡 Registra modem".to_owned(),
        "🔗 Collegamenti".to_owned(),
        "🗺 Posizione / Mappa".to_owned(),
        "🔎 Cerca".to_owned(),
        "📋 Intervento corrente".to_owned(),
        "📊 Stato armadio".to_owned(),
        "🏠 Menu principale".to_owned(),
    ];
    for cabinet_id in cabinet_ids {
        if !labels.iter().any(|label| label == cabinet_id) {
            labels.push(cabinet_id.clone());
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::{MenuAction, main_menu_labels};

    #[test]
    fn mandatory_home_and_separate_confirm_close_are_exposed() {
        let labels = main_menu_labels(&["CAB-A".into(), "CAB-D".into()]);
        assert!(labels.contains(&"🏠 Menu principale".to_owned()));
        assert!(MenuAction::parse("✅ Conferma operazione").is_some());
        assert!(MenuAction::parse("✅ Chiudi intervento").is_some());
        assert!(labels.contains(&"CAB-D".to_owned()));
    }

    #[test]
    fn unregistered_commands_are_not_actions() {
        assert_eq!(MenuAction::parse("/sql"), None);
        assert_eq!(MenuAction::parse("/shell"), None);
    }
}
