// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed-width exact identities; never JSON floating-point numbers.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

/// Exact binary64 input or computational source bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binary64(pub u64);

impl Serialize for Binary64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:016x}", self.0))
    }
}

impl<'de> Deserialize<'de> for Binary64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if !canonical_hex(&text, 16) {
            return Err(D::Error::custom("expected 16 lowercase hexadecimal digits"));
        }
        u64::from_str_radix(&text, 16)
            .map(Self)
            .map_err(D::Error::custom)
    }
}

/// Full 128-bit recipe or bootstrap seed, distinct from its effective offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Seed(pub u128);

impl Seed {
    /// Effective offset of the declared deterministic generator.
    #[must_use]
    pub const fn offset(self) -> u128 {
        (self.0 ^ (self.0 >> 64)) & 1_048_575
    }
}

impl Serialize for Seed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:032x}", self.0))
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if !canonical_hex(&text, 32) {
            return Err(D::Error::custom("expected 32 lowercase hexadecimal digits"));
        }
        u128::from_str_radix(&text, 16)
            .map(Self)
            .map_err(D::Error::custom)
    }
}

/// Fixed lowercase hexadecimal grammar, excluding alternate spellings.
fn canonical_hex(text: &str, length: usize) -> bool {
    let width = text.len() == length;
    let digits = text
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    width && digits
}
