pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

pub struct AuthToken {
    pub token: String,
    pub expires_at: u64,
}

pub trait Authenticator {
    fn authenticate(&self, req: &AuthRequest) -> Result<AuthToken, AuthError>;
}

pub enum AuthError {
    InvalidCredentials,
    Expired,
}
