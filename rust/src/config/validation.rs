use super::AppConfig;
use std::{io, net::IpAddr};

pub fn is_yggdrasil(ip: std::net::Ipv6Addr) -> bool {
    ip.segments()[0] & 0xfe00 == 0x0200
}

pub fn validate(config: &AppConfig, api_port: u16) -> Result<(), io::Error> {
    let bind_ip = match config.server.bind_addr.ip() {
        IpAddr::V6(ip) => ip,
        IpAddr::V4(_) => return Err(invalid("API must bind to an IPv6 Yggdrasil address")),
    };
    if !is_yggdrasil(bind_ip) || config.server.bind_addr.port() != api_port {
        return Err(invalid(
            "API bind must use the configured Yggdrasil IPv6 and port",
        ));
    }
    if !is_yggdrasil(config.database.host) {
        return Err(invalid("database host must be an IPv6 Yggdrasil address"));
    }
    if config.database.password.trim().is_empty()
        || config.database.name.trim().is_empty()
        || config.database.user.trim().is_empty()
    {
        return Err(invalid("database settings are incomplete"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::is_yggdrasil;
    use std::net::Ipv6Addr;

    #[test]
    fn recognizes_yggdrasil_prefix_only() {
        assert!(is_yggdrasil("200::1".parse::<Ipv6Addr>().unwrap()));
        assert!(!is_yggdrasil("2001:db8::1".parse::<Ipv6Addr>().unwrap()));
    }
}
