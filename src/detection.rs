//! Detects the connected HAT from its EEPROM (exposed by the Pi firmware in the device tree).

use std::io::ErrorKind;
use std::path::Path;

use crate::hats::{self, HatProfile};

const HAT_PRODUCT_PATH: &str = "/proc/device-tree/hat/product";

pub async fn detect_connected_hat() -> Option<&'static HatProfile> {
    let raw = match tokio::fs::read(HAT_PRODUCT_PATH).await {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            tracing::info!("[Detection] No HAT EEPROM detected at {HAT_PRODUCT_PATH}.");
            if Path::new("/proc/device-tree/hat").exists() {
                tracing::info!("[Detection] /proc/device-tree/hat exists, but product file is missing.");
            } else {
                tracing::info!("[Detection] /proc/device-tree/hat directory does not exist.");
            }
            return None;
        }
        Err(error) => {
            tracing::error!("[Detection] Error reading HAT info: {error}");
            return None;
        }
    };

    // e.g. "HiFiBerry DAC+ Pro\0"
    let product = String::from_utf8_lossy(&raw).replace('\0', "");
    let product = product.trim();
    tracing::info!("[Detection] Found EEPROM Product: \"{product}\"");

    match match_product(product) {
        Some(hat) => {
            tracing::info!("[Detection] Match found: {} ({})", hat.id, hat.name);
            Some(hat)
        }
        None => {
            tracing::warn!("[Detection] Unknown HAT: \"{product}\" - No matching profile found.");
            None
        }
    }
}

/// The profile with the longest `eeprom_match` contained in the product name.
/// On equal length the first profile wins.
pub fn match_product(product: &str) -> Option<&'static HatProfile> {
    let mut best: Option<(&'static HatProfile, usize)> = None;
    for hat in hats::SUPPORTED_HATS {
        if let Some(needle) = hat.eeprom_match {
            if product.contains(needle) && best.is_none_or(|(_, len)| needle.len() > len) {
                best = Some((hat, needle.len()));
            }
        }
    }
    best.map(|(hat, _)| hat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_match_wins() {
        assert_eq!(match_product("HiFiBerry DAC+ ADC Pro").unwrap().id, "hifiberry-dacplusadcpro");
        assert_eq!(match_product("HiFiBerry DAC+ Pro").unwrap().id, "hifiberry-dacplus-pro");
        // "HiFiBerry DAC+" matches dacplus-std and the legacy alias; the first one wins
        assert_eq!(match_product("HiFiBerry DAC+").unwrap().id, "hifiberry-dacplus-std");
        assert!(match_product("Something else").is_none());
    }
}
