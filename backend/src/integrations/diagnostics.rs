use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderIssue {
    pub provider: &'static str,
    pub mint: String,
    pub code: &'static str,
    pub http_status: Option<u16>,
    pub message: &'static str,
}

impl ProviderIssue {
    pub fn new(
        provider: &'static str,
        mint: &str,
        code: &'static str,
        message: &'static str,
    ) -> Self {
        Self {
            provider,
            mint: mint.into(),
            code,
            message,
            http_status: None,
        }
    }

    pub fn http(provider: &'static str, mint: &str, status: u16) -> Self {
        let (code, message) = match status {
            401 => (
                "authentication_failed",
                "The provider rejected the configured credential.",
            ),
            402 => ("quota_or_plan_required", "The provider requires an available quota or eligible plan; no upgrade was attempted."),
            403 => (
                "access_denied",
                "The provider denied access; check the account's API permissions.",
            ),
            404 => (
                "report_unavailable",
                "The provider has no report for this token or the endpoint is unavailable.",
            ),
            429 => (
                "rate_limited",
                "The provider's rate or usage limit was reached.",
            ),
            500..=599 => (
                "provider_error",
                "The provider could not process the request.",
            ),
            _ => ("request_rejected", "The provider rejected this request."),
        };
        Self {
            http_status: Some(status),
            ..Self::new(provider, mint, code, message)
        }
    }

    pub fn transport(provider: &'static str, mint: &str, error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::timeout(provider, mint)
        } else {
            Self::new(
                provider,
                mint,
                "connection_failed",
                "Could not connect to the provider or validate its connection.",
            )
        }
    }

    pub fn timeout(provider: &'static str, mint: &str) -> Self {
        Self::new(
            provider,
            mint,
            "timed_out",
            "The token-scan time limit was reached; completed results were retained.",
        )
    }

    pub fn invalid_response(provider: &'static str, mint: &str) -> Self {
        Self::new(
            provider,
            mint,
            "invalid_response",
            "The provider returned no usable score or risk data.",
        )
    }
}
