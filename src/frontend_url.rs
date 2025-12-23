use anyhow::{Context, Result};
use std::env;

pub fn get() -> String {
    env::var("FRONTEND_URL").unwrap_or_else(|_| "http://0.0.0.0:8080".to_string())
}

pub fn validate_or_warn() -> Result<()> {
    let Ok(raw_url) = std::env::var("FRONTEND_URL") else {
        return Ok(());
    };

    let url =
        url::Url::parse(&raw_url).with_context(|| format!("Invalid FRONTEND_URL {raw_url:?}"))?;

    let Some(host) = url.host() else {
        log::error!("FRONTEND_URL {raw_url:?} has no host");
        return Ok(());
    };

    let host = host.to_string();

    if !(host.contains(".")) {
        log::warn!(
            "FRONTEND_URL {raw_url:?} has a host with no dots ({host:?}), links might fail to render. See e.g. https://github.com/telegramdesktop/tdesktop/issues/7827

Hint: Try localhost.localdomain, 127.0.0.1, 0.0.0.0, the target's IP address");
    }

    Ok(())
}
