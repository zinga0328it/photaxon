use std::{
    env, io,
    path::{Path, PathBuf},
};

pub fn load_env_file() -> Result<(), io::Error> {
    let path = env::var_os("FIBERSPIDER_ENV_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("secrets/runtime.env"));
    dotenvy::from_path(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "FiberSpider runtime environment file is unavailable",
        )
    })?;
    let openstack_path = env::var_os("FIBERSPIDER_OPENSTACK_ENV_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("secrets/openstack-api.env"));
    if openstack_path.exists() {
        dotenvy::from_path(openstack_path).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "OpenStack runtime environment file is unavailable",
            )
        })?;
    }
    Ok(())
}

pub fn required(name: &'static str) -> Result<String, io::Error> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("required runtime setting {name} is missing"),
            )
        })
}

pub fn optional(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

pub fn required_from_file(path: &Path, name: &str) -> Result<String, io::Error> {
    let entries = dotenvy::from_path_iter(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "protected runtime file is unavailable",
        )
    })?;
    for entry in entries {
        let (key, value) = entry.map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "protected runtime file is invalid",
            )
        })?;
        if key == name && !value.trim().is_empty() {
            return Ok(value);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "required protected runtime setting is missing",
    ))
}
