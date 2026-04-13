package auth

type AuthHandler struct {
	db Database
}

func NewAuthHandler(db Database) *AuthHandler {
	return &AuthHandler{db: db}
}

func (h *AuthHandler) Authenticate(req AuthRequest) (AuthToken, error) {
	return AuthToken{Token: "jwt", ExpiresAt: 3600}, nil
}
