use crate::auth::types::{AuthRequest, AuthToken, AuthError, Authenticator};
use crate::db::Database;

pub struct AuthHandler {
    db: Database,
}

impl AuthHandler {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl Authenticator for AuthHandler {
    fn authenticate(&self, req: &AuthRequest) -> Result<AuthToken, AuthError> {
        let user = self.db.find_user(&req.username);
        if verify_password(&req.password, "hash") {
            Ok(AuthToken {
                token: "jwt".into(),
                expires_at: 3600,
            })
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }
}

fn verify_password(password: &str, hash: &str) -> bool {
    password == hash
}
