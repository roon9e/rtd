//! Password providers for two-factor authorization.

/// Supplies the account password when the server asks for it.
///
/// The client calls this only after `account.getPassword` reports an SRP
/// challenge, so a caller that has no password configured can return `None`.
pub trait PasswordProvider: Send {
    /// Returns the account password, or `None` if none is configured.
    fn password(&self) -> Option<Vec<u8>>;
}

/// A [`PasswordProvider`] that always returns the same password.
#[derive(Debug, Clone)]
pub struct ConstantPassword(Vec<u8>);

impl ConstantPassword {
    /// Creates a provider for `password`.
    #[must_use]
    pub fn new(password: impl Into<Vec<u8>>) -> Self {
        Self(password.into())
    }
}

impl PasswordProvider for ConstantPassword {
    fn password(&self) -> Option<Vec<u8>> {
        Some(self.0.clone())
    }
}

/// A bot authorization token.
///
/// The token is held privately and its [`Debug`](core::fmt::Debug)
/// implementation is redacted, so it can never leak into logs.
#[derive(Clone, PartialEq, Eq)]
pub struct BotToken(String);

impl BotToken {
    /// Wraps a token literal.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Reads a token from the environment, checking `BOT_TOKEN` then
    /// `TELEGRAM_BOT_TOKEN`.
    ///
    /// Returns `None` when neither variable is set or a variable is empty.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        for key in ["BOT_TOKEN", "TELEGRAM_BOT_TOKEN"] {
            if let Ok(value) = std::env::var(key)
                && !value.is_empty()
            {
                return Some(Self(value));
            }
        }
        None
    }

    /// Returns the token string.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Debug for BotToken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("BotToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::BotToken;

    #[test]
    fn token_debug_is_redacted() {
        let token = BotToken::new("123456:secret");
        assert_eq!(token.expose(), "123456:secret");
        assert_eq!(format!("{token:?}"), "BotToken(<redacted>)");
    }
}
