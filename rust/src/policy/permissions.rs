#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolClass {
    Read,
    WriteSafe,
    WriteTopology,
    Admin,
}

pub fn class_for(tool: &str) -> Option<ToolClass> {
    match tool {
        "list_cabinets"
        | "get_cabinet"
        | "get_topology"
        | "list_roes"
        | "get_roe"
        | "list_ptes"
        | "get_pte"
        | "list_splitters"
        | "get_splitter_capacity"
        | "get_pending_topology_changes" => Some(ToolClass::Read),
        "report_ont_event" | "report_anomaly" => Some(ToolClass::WriteSafe),
        "create_cabinet"
        | "create_roe"
        | "create_pte"
        | "create_splitter"
        | "attach_roe_to_cabinet"
        | "attach_pte_to_line"
        | "attach_resource"
        | "update_physical_path" => Some(ToolClass::WriteTopology),
        _ => None,
    }
}

pub fn codex_may_call(tool: &str) -> bool {
    matches!(
        class_for(tool),
        Some(ToolClass::Read | ToolClass::WriteSafe | ToolClass::WriteTopology)
    )
}

#[cfg(test)]
mod tests {
    use super::{ToolClass, class_for, codex_may_call};

    #[test]
    fn admin_and_unregistered_tools_are_never_available_to_codex() {
        assert_eq!(class_for("create_roe"), Some(ToolClass::WriteTopology));
        assert!(!codex_may_call("systemctl"));
        assert!(!codex_may_call("exec_shell"));
        assert!(!codex_may_call("run_sql"));
        assert!(!codex_may_call("admin_drop_database"));
    }
}
