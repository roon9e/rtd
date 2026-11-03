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
