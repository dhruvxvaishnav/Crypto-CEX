use crate::auth::AuthService;
use crate::readiness::SharedReadiness;

/// Application state shared by handlers.
#[derive(Clone)]
pub struct AppState {
    /// Auth service.
    pub auth: AuthService,
    /// Readiness dependency checker.
    pub readiness: SharedReadiness,
}

impl AppState {
    /// Creates application state.
    #[must_use]
    pub const fn new(auth: AuthService, readiness: SharedReadiness) -> Self {
        Self { auth, readiness }
    }
}
