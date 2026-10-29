//! Relay's brand: the cobalt accent.

use bolt::Brand;
use bolt::tokens::hex;

/// The brand Relay installs into Bolt.
pub(crate) fn relay() -> Brand {
    Brand {
        name: "Relay".into(),
        accent: hex(0x2B45B8),
        accent_hover: hex(0x3451C8),
        accent_pressed: hex(0x1F3389),
        accent_tint: hex(0x9DB1FF),
        on_accent: hex(0xFFFFFF),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::tests::rgb_of;

    #[test]
    fn relay_brand_is_cobalt() {
        let brand = relay();
        assert_eq!(brand.name.as_ref(), "Relay");
        assert_eq!(rgb_of(brand.accent), 0x2B45B8);
        assert_eq!(rgb_of(brand.accent_hover), 0x3451C8);
        assert_eq!(rgb_of(brand.accent_pressed), 0x1F3389);
        assert_eq!(rgb_of(brand.accent_tint), 0x9DB1FF);
        assert_eq!(rgb_of(brand.on_accent), 0xFFFFFF);
    }
}
