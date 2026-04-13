package auth

type AuthRequest struct {
	Username string
	Password string
}

type AuthToken struct {
	Token     string
	ExpiresAt int64
}

type Authenticator interface {
	Authenticate(req AuthRequest) (AuthToken, error)
}
