//! Supported audio HATs and their boot overlay / CamillaDSP output settings.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CamillaOutput {
    pub device: &'static str,
    pub format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HatProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub overlay: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eeprom_match: Option<&'static str>,
    pub camilla: CamillaOutput,
}

/// Looks up a HAT by its ID.
pub fn find(id: &str) -> Option<&'static HatProfile> {
    SUPPORTED_HATS.iter().find(|hat| hat.id == id)
}

/// All supported HATs. The order matters: when guessing the active HAT from
/// `config.txt`, the first profile with a matching overlay wins.
pub static SUPPORTED_HATS: &[HatProfile] = &[
    HatProfile {
        id: "hifiberry-dac",
        name: "HiFiBerry DAC / DAC Zero / MiniAmp / Beocreate / DAC+ Light",
        overlay: "dtoverlay=hifiberry-dac",
        eeprom_match: Some("HiFiBerry DAC"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dac8x",
        name: "HiFiBerry DAC8x",
        overlay: "dtoverlay=hifiberry-dac8x",
        eeprom_match: Some("HiFiBerry DAC8x"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dacplus-std",
        name: "HiFiBerry DAC+ Standard",
        overlay: "dtoverlay=hifiberry-dacplus-std",
        eeprom_match: Some("HiFiBerry DAC+"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dacplus-pro",
        name: "HiFiBerry DAC+ Pro / DAC2 Pro",
        overlay: "dtoverlay=hifiberry-dacplus-pro",
        eeprom_match: Some("HiFiBerry DAC+ Pro"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dacplus",
        name: "HiFiBerry DAC+ Pro (Legacy)",
        overlay: "dtoverlay=hifiberry-dacplus-pro",
        eeprom_match: Some("HiFiBerry DAC+"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dacplus-xr",
        name: "HiFiBerry DAC+ XR",
        overlay: "dtoverlay=hifiberry-dacplus-xr",
        eeprom_match: Some("HiFiBerry DAC+ XR"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dacplushd",
        name: "HiFiBerry DAC2 HD",
        overlay: "dtoverlay=hifiberry-dacplushd",
        eeprom_match: Some("HiFiBerry DAC2 HD"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dacplusadc",
        name: "HiFiBerry DAC+ ADC",
        overlay: "dtoverlay=hifiberry-dacplusadc",
        eeprom_match: Some("HiFiBerry DAC+ ADC"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-dacplusadcpro",
        name: "HiFiBerry DAC+ ADC Pro / DAC2 ADC Pro",
        overlay: "dtoverlay=hifiberry-dacplusadcpro",
        eeprom_match: Some("HiFiBerry DAC+ ADC Pro"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-amp",
        name: "HiFiBerry Amp+",
        overlay: "dtoverlay=hifiberry-amp",
        eeprom_match: Some("HiFiBerry Amp"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-amp2",
        name: "HiFiBerry Amp2 / Amp4",
        overlay: "dtoverlay=hifiberry-dacplus-std",
        eeprom_match: Some("HiFiBerry Amp2"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-amp3",
        name: "HiFiBerry Amp3",
        overlay: "dtoverlay=hifiberry-amp3",
        eeprom_match: Some("HiFiBerry Amp3"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-amp4pro",
        name: "HiFiBerry Amp4 Pro",
        overlay: "dtoverlay=hifiberry-amp4pro",
        eeprom_match: Some("HiFiBerry Amp4 Pro"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S32LE",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-digi",
        name: "HiFiBerry Digi / Digi+ / Digi 2 Standard",
        overlay: "dtoverlay=hifiberry-digi",
        eeprom_match: Some("HiFiBerry Digi"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S24LE3",
            channels: None,
        },
    },
    HatProfile {
        id: "hifiberry-digi-pro",
        name: "HiFiBerry Digi+ Pro / Digi 2 Pro",
        overlay: "dtoverlay=hifiberry-digi-pro",
        eeprom_match: Some("HiFiBerry Digi+ Pro"),
        camilla: CamillaOutput {
            device: "plughw:CARD=sndrpihifiberry,DEV=0",
            format: "S24LE3",
            channels: None,
        },
    },
    HatProfile {
        id: "rpi-hdmi0",
        name: "Raspberry Pi HDMI 0",
        overlay: "# Beatnik HDMI0",
        eeprom_match: None,
        camilla: CamillaOutput { device: "hdmi:1,0", format: "S16LE", channels: None },
    },
    HatProfile {
        id: "iqaudio-dacplus",
        name: "IQaudIO Pi-DAC PRO / DAC+",
        overlay: "dtoverlay=iqaudio-dacplus",
        eeprom_match: Some("Pi-DAC PRO"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "iqaudio-dac",
        name: "IQaudIO Pi-DAC Zero / Pi-DAC+ (non-Pro)",
        overlay: "dtoverlay=iqaudio-dac",
        eeprom_match: Some("IQaudIO Limited"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "iqaudio-digi",
        name: "IQaudIO Pi-Digi+",
        overlay: "dtoverlay=iqaudio-digi-wm8804",
        eeprom_match: Some("IQaudIO Pi-Digi+"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S24LE3", channels: None },
    },
    HatProfile {
        id: "iqaudio-codec",
        name: "IQaudIO Pi-Codec / Pi-Codec+",
        overlay: "dtoverlay=iqaudio-codec",
        eeprom_match: Some("IQaudIO Pi-Codec"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "iqaudio-digiamp-plus",
        name: "IQaudIO Pi-DigiAMP+",
        overlay: "dtoverlay=iqaudio-digiampplus,unmute_amp",
        eeprom_match: Some("IQaudIO Pi-DigiAMP+"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "rpi-dacpro",
        name: "Raspberry Pi DAC Pro",
        overlay: "dtoverlay=rpi-dacpro",
        eeprom_match: Some("Raspberry Pi DAC Pro"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "rpi-dacplus",
        name: "Raspberry Pi DAC+",
        overlay: "dtoverlay=rpi-dacplus",
        eeprom_match: Some("Raspberry Pi DAC+"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "rpi-codeczero",
        name: "Raspberry Pi Codec Zero",
        overlay: "dtoverlay=rpi-codeczero",
        eeprom_match: Some("Raspberry Pi Codec Zero"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "rpi-digiamp-plus",
        name: "Raspberry Pi DigiAMP+",
        // Official Raspberry Pi-branded overlay (green PCB), distinct from the IQaudIO (black PCB) variant
        overlay: "dtoverlay=rpi-digiampplus,unmute_amp",
        eeprom_match: Some("Raspberry Pi DigiAMP+"),
        camilla: CamillaOutput { device: "hw:1,0", format: "S32LE", channels: None },
    },
    HatProfile {
        id: "usb-dac",
        name: "Generic USB DAC",
        overlay: "# No overlay needed for USB",
        eeprom_match: None,
        camilla: CamillaOutput { device: "hw:1,0", format: "S16LE", channels: None },
    },
    HatProfile {
        id: "none",
        name: "No HAT (Headphone Jack)",
        overlay: "# No HAT configured",
        eeprom_match: None,
        camilla: CamillaOutput { device: "plughw:Headphones", format: "S16LE", channels: None },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<_> = SUPPORTED_HATS.iter().map(|hat| hat.id).collect();
        assert_eq!(ids.len(), SUPPORTED_HATS.len());
    }

    #[test]
    fn find_by_id() {
        assert_eq!(find("hifiberry-amp").unwrap().name, "HiFiBerry Amp+");
        assert!(find("does-not-exist").is_none());
    }

    #[test]
    fn serializes_like_the_typescript_version() {
        let json = serde_json::to_value(find("hifiberry-dac").unwrap()).unwrap();
        assert_eq!(json["eepromMatch"], "HiFiBerry DAC");
        assert!(json["camilla"].get("channels").is_none());
        let none = serde_json::to_value(find("none").unwrap()).unwrap();
        assert!(none.get("eepromMatch").is_none());
    }
}
