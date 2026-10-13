// SPDX-License-Identifier: AGPL-3.0-only
//! Strict, located access to the versioned JSON/TOML wire records.

use nrz_algebra::{Expr, Limits, Node, Rational, Symbol, parse};
use serde_json::Value;

use crate::error::Error;

/// A borrowed wire value with its complete source and field location.
#[derive(Clone, Debug)]
pub(super) struct Record<'a> {
    /// Original immutable value.
    pub(super) value: &'a Value,
    /// Source path and nested field path.
    pub(super) path: String,
}

impl<'a> Record<'a> {
    /// Locate a source value without changing its representation.
    pub(super) fn new(value: &'a Value, path: impl Into<String>) -> Self {
        Self {
            value,
            path: path.into(),
        }
    }

    /// Retain the field location in every decoding and checker refusal.
    pub(super) fn error(&self, detail: impl std::fmt::Display) -> Error {
        Error::Payload {
            path: self.path.clone(),
            detail: detail.to_string(),
        }
    }

    /// Admit exactly the required and optional keys, rejecting null alternatives.
    pub(super) fn keys(&self, required: &[&str], optional: &[&str]) -> Result<(), Error> {
        let object = self
            .value
            .as_object()
            .ok_or_else(|| self.error("expected an object"))?;
        for key in required {
            if !object.contains_key(*key) {
                return Err(self.error(format!("missing {key}")));
            }
        }
        for (key, value) in object {
            if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
                return Err(self.error(format!("unknown field {key}")));
            }
            if value.is_null() {
                return Err(self.error(format!("{key} must be absent rather than null")));
            }
        }
        Ok(())
    }

    /// Locate a required field.
    pub(super) fn at(&self, key: &str) -> Result<Self, Error> {
        self.optional(key)
            .ok_or_else(|| self.error(format!("missing {key}")))
    }

    /// Locate an optional field, preserving its exact value.
    pub(super) fn optional(&self, key: &str) -> Option<Self> {
        self.value
            .get(key)
            .map(|value| Self::new(value, format!("{}.{key}", self.path)))
    }

    /// Read a nonempty string.
    pub(super) fn text(&self) -> Result<&'a str, Error> {
        self.value
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| self.error("expected a nonempty string"))
    }

    /// Read a required nonempty string field.
    pub(super) fn string(&self, key: &str) -> Result<&'a str, Error> {
        self.at(key)?.text()
    }

    /// Read an array, retaining an index for every child.
    pub(super) fn list(&self) -> Result<Vec<Self>, Error> {
        let values = self
            .value
            .as_array()
            .ok_or_else(|| self.error("expected an array"))?;
        Ok(values
            .iter()
            .enumerate()
            .map(|(index, value)| Self::new(value, format!("{}[{index}]", self.path)))
            .collect())
    }

    /// Read a required list field.
    pub(super) fn array(&self, key: &str) -> Result<Vec<Self>, Error> {
        self.at(key)?.list()
    }

    /// Read an optional list with an explicit empty serialization default.
    pub(super) fn optional_array(&self, key: &str) -> Result<Vec<Self>, Error> {
        self.optional(key)
            .map_or_else(|| Ok(Vec::new()), |value| value.list())
    }

    /// Read a boolean, never accepting numeric truth values.
    pub(super) fn boolean(&self) -> Result<bool, Error> {
        self.value
            .as_bool()
            .ok_or_else(|| self.error("expected a boolean"))
    }

    /// Read a bounded signed integer for a domain endpoint.
    pub(super) fn integer(&self) -> Result<i64, Error> {
        self.value
            .as_i64()
            .ok_or_else(|| self.error("expected a signed 64-bit integer"))
    }

    /// Read an optional signed domain endpoint.
    pub(super) fn bound(&self, key: &str) -> Result<Option<i64>, Error> {
        self.optional(key).map(|value| value.integer()).transpose()
    }

    /// Read a parser-valid identifier.
    pub(super) fn symbol(&self) -> Result<Symbol, Error> {
        Symbol::new(self.text()?).map_err(|error| self.error(error))
    }

    /// Decode exact coordinates without accepting general algebra or rounding.
    pub(super) fn rational(&self) -> Result<Rational, Error> {
        let text = self.text()?;
        let parts: Vec<_> = text.split('/').collect();
        match parts.as_slice() {
            [value] => self.literal(value),
            [numerator, denominator] => self
                .literal(numerator)?
                .checked_div(self.literal(denominator)?)
                .map_err(|error| self.error(error)),
            _ => Err(self.error("a coordinate is a literal or a single literal fraction")),
        }
    }

    /// Decode one signed decimal/scientific literal with the algebra parser.
    fn literal(&self, text: &str) -> Result<Rational, Error> {
        if text.is_empty()
            || text.bytes().any(|byte| {
                !byte.is_ascii_digit() && !matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E')
            })
        {
            return Err(self.error("invalid exact literal"));
        }
        let expression = parse(text, Limits::default()).map_err(|error| self.error(error))?;
        match expression.node() {
            Node::Constant(value) => Ok(*value),
            Node::Neg(child) => match child.node() {
                Node::Constant(value) => value.checked_neg().map_err(|error| self.error(error)),
                _ => Err(self.error("a coordinate is not an arithmetic expression")),
            },
            _ => Err(self.error("a coordinate is not an arithmetic expression")),
        }
    }

    /// Parse an original algebra expression without normalization.
    pub(super) fn expression(&self) -> Result<Expr, Error> {
        parse(self.text()?, Limits::default()).map_err(|error| self.error(error))
    }
}
