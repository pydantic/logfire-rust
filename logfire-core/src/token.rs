//! Token parsing and region detection.

/// Extracts the region string from a Logfire token.
///
/// Token format: `pylf_v{version}_{region}_{token}`
/// Returns `None` if the token format is invalid.
#[allow(clippy::must_use_candidate)]
pub fn parse_region(token: &str) -> Option<&str> {
    let rest = token.strip_prefix("pylf_v")?;
    let (_, rest) = rest.split_once('_')?;
    let (region, _) = rest.split_once('_')?;
    Some(region)
}

/// Region segments that map to a known Logfire region.
const KNOWN_REGIONS: &[&str] = &["us", "eu", "stagingus", "stagingeu"];

/// Returns the region segment of a Logfire token when this SDK does not know it.
///
/// [`Region::from_token`] falls back to the US region for such a token, so callers can
/// use this to warn that data may go to the wrong region. Returns `None` for a known
/// region and for a token without a region segment (for example a legacy token).
#[allow(clippy::must_use_candidate)]
pub fn unknown_region(token: &str) -> Option<&str> {
    parse_region(token).filter(|region| !KNOWN_REGIONS.contains(region))
}

/// Returns a warning message when the region of a Logfire token is not known.
///
/// The message names the unknown region segment and the US base URL that is used instead.
#[must_use]
pub fn unknown_region_warning(token: &str) -> Option<String> {
    unknown_region(token).map(|region| {
        format!(
            "logfire: unrecognised region `{region}` in the Logfire token, sending data to the US region ({}). Set the base URL explicitly if this is wrong.",
            Region::US_URL
        )
    })
}

/// Logfire data region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Region {
    /// US production region.
    Us,
    /// EU production region.
    Eu,
    /// US staging region.
    StagingUs,
    /// EU staging region.
    StagingEu,
    /// Custom base URL for self-hosted deployments.
    Other(String),
}

impl Region {
    /// Base URL for the US production region.
    pub const US_URL: &str = "https://logfire-us.pydantic.dev";
    /// Base URL for the EU production region.
    pub const EU_URL: &str = "https://logfire-eu.pydantic.dev";
    /// Base URL for the US staging region.
    pub const STAGING_US_URL: &str = "https://logfire-us.pydantic.info";
    /// Base URL for the EU staging region.
    pub const STAGING_EU_URL: &str = "https://logfire-eu.pydantic.info";

    /// Detects the region from a Logfire token.
    ///
    /// Falls back to US if the token format is unrecognized or the region is unknown.
    #[must_use]
    pub fn from_token(token: &str) -> Self {
        match parse_region(token) {
            Some("eu") => Self::Eu,
            Some("stagingus") => Self::StagingUs,
            Some("stagingeu") => Self::StagingEu,
            Some("us" | _) | None => Self::Us,
        }
    }

    /// Returns the base URL for this region.
    #[must_use]
    pub fn base_url(&self) -> &str {
        match self {
            Self::Us => Self::US_URL,
            Self::Eu => Self::EU_URL,
            Self::StagingUs => Self::STAGING_US_URL,
            Self::StagingEu => Self::STAGING_EU_URL,
            Self::Other(url) => url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Region, parse_region, unknown_region, unknown_region_warning};

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
        assert_eq!(
            Region::from_token("pylf_v1_stagingus_abc123"),
            Region::StagingUs
        );
        assert_eq!(
            Region::from_token("pylf_v1_stagingeu_abc123"),
            Region::StagingEu
        );
    }

    #[test]
    fn from_token_falls_back_to_us() {
        assert_eq!(Region::from_token("pylf_v1_unknown_abc123"), Region::Us);
        assert_eq!(Region::from_token("not-a-valid-token"), Region::Us);
    }

    #[test]
    fn unknown_region_reports_unrecognised_segment() {
        assert_eq!(unknown_region("pylf_v1_unknown_abc123"), Some("unknown"));
        assert_eq!(unknown_region("pylf_v1_ap_abc123"), Some("ap"));
    }

    #[test]
    fn unknown_region_ignores_known_and_legacy_tokens() {
        for token in [
            "pylf_v1_us_abc123",
            "pylf_v1_eu_abc123",
            "pylf_v1_stagingus_abc123",
            "pylf_v1_stagingeu_abc123",
            "not-a-valid-token",
            "",
        ] {
            assert_eq!(unknown_region(token), None, "{token}");
            assert_eq!(unknown_region_warning(token), None, "{token}");
        }
    }

    #[test]
    fn unknown_region_warning_names_region_and_fallback() {
        let warning = unknown_region_warning("pylf_v1_ap_abc123").unwrap();
        assert!(warning.contains("`ap`"), "{warning}");
        assert!(warning.contains(Region::US_URL), "{warning}");
    }
}
