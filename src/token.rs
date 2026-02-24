//! Token parsing and region detection.

/// Extracts the region string from a Logfire token.
///
/// Token format: `pylf_v{version}_{region}_{token}`
/// Returns `None` if the token format is invalid.
fn parse_region(token: &str) -> Option<&str> {
    let rest = token.strip_prefix("pylf_v")?;
    let (_, rest) = rest.split_once('_')?; // skip version
    let (region, _) = rest.split_once('_')?; // extract region
    Some(region)
}

/// Logfire data region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Region {
    /// US production region.
    Us,
    /// EU production region.
    Eu,
    /// Custom base URL for self-hosted deployments.
    Other(String),
}

impl Region {
    /// Base URL for the US production region.
    pub const US_URL: &str = "https://logfire-us.pydantic.dev";
    /// Base URL for the EU production region.
    pub const EU_URL: &str = "https://logfire-eu.pydantic.dev";

    /// Detects the region from a Logfire token.
    ///
    /// Falls back to US if the token format is unrecognized or the region is unknown.
    pub fn from_token(token: &str) -> Self {
        match parse_region(token) {
            Some("us") => Self::Us,
            Some("eu") => Self::Eu,
            _ => Self::Us,
        }
    }

    /// Returns the base URL for this region.
    pub fn base_url(&self) -> &str {
        match self {
            Self::Us => Self::US_URL,
            Self::Eu => Self::EU_URL,
            Self::Other(url) => url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_region_extracts_region() {
        assert_eq!(parse_region("pylf_v1_us_abc123"), Some("us"));
        assert_eq!(parse_region("pylf_v1_eu_abc123"), Some("eu"));
        assert_eq!(parse_region("pylf_v2_someregion_xyz"), Some("someregion"));
    }

    #[test]
    fn parse_region_returns_none_for_invalid() {
        assert_eq!(parse_region("not-a-valid-token"), None);
        assert_eq!(parse_region("pylf_v1_"), None);
        assert_eq!(parse_region(""), None);
    }

    #[test]
    fn from_token_parses_regions() {
        assert_eq!(Region::from_token("pylf_v1_us_abc123"), Region::Us);
        assert_eq!(Region::from_token("pylf_v1_eu_abc123"), Region::Eu);
    }

    #[test]
    fn from_token_falls_back_to_us() {
        assert_eq!(Region::from_token("pylf_v1_unknown_abc123"), Region::Us);
        assert_eq!(Region::from_token("not-a-valid-token"), Region::Us);
    }
}
