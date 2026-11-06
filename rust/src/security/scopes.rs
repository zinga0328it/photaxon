use axum::http::Method;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Scope {
    Dashboard,
    Broker,
    McpRead,
    McpWrite,
    Admin,
    Secrets,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Broker => "broker",
            Self::McpRead => "mcp:read",
            Self::McpWrite => "mcp:write",
            Self::Admin => "admin",
            Self::Secrets => "secrets",
        }
    }
}

pub fn required_scope(method: &Method, path: &str) -> Option<Scope> {
    if method != Method::GET {
        return None;
    }
    match path {
        "/api/health" | "/api/lab/openstack" | "/api/lab/cabinets" | "/api/lab/roes"
        | "/api/lab/topology" => Some(Scope::Dashboard),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Scope, required_scope};
    use axum::http::Method;

    #[test]
    fn dashboard_scope_maps_only_the_five_read_endpoints() {
        for path in [
            "/api/health",
            "/api/lab/openstack",
            "/api/lab/cabinets",
            "/api/lab/roes",
            "/api/lab/topology",
        ] {
            assert_eq!(required_scope(&Method::GET, path), Some(Scope::Dashboard));
        }
    }

    #[test]
    fn dashboard_scope_never_maps_writes_or_privileged_routes() {
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert_eq!(
                required_scope(&method, "/api/lab/cabinets"),
                None,
                "{method} must not receive dashboard scope"
            );
        }
        for path in [
            "/api/v1/admin/database",
            "/api/v1/internal/cabinets",
            "/api/mcp/tools/write",
            "/api/secrets",
        ] {
            assert_eq!(required_scope(&Method::GET, path), None);
        }
    }
}
