package auth

import (
	"errors"
	"fmt"
)

// AuthRequest holds the credentials for an authentication attempt.
type AuthRequest struct {
	Username string
	Password string
}

// AuthToken represents a successful authentication result.
type AuthToken struct {
	Token     string
	ExpiresAt int64
}

// Authenticator is the interface for authentication handlers.
type Authenticator interface {
	Authenticate(req *AuthRequest) (*AuthToken, error)
}

// AuthError represents an authentication failure.
type AuthError struct {
	Code    string
	Message string
}

func (e *AuthError) Error() string {
	return fmt.Sprintf("[%s] %s", e.Code, e.Message)
}

// AuthHandler implements the Authenticator interface.
type AuthHandler struct {
	secret string
}

// NewAuthHandler creates a new AuthHandler.
func NewAuthHandler(secret string) *AuthHandler {
	return &AuthHandler{secret: secret}
}

// Authenticate verifies credentials and returns a token.
func (h *AuthHandler) Authenticate(req *AuthRequest) (*AuthToken, error) {
	if req.Username == "" || req.Password == "" {
		return nil, &AuthError{Code: "INVALID_CREDENTIALS", Message: "missing credentials"}
	}
	token := h.generateToken(req.Username)
	return &AuthToken{Token: token, ExpiresAt: 3600}, nil
}

func (h *AuthHandler) generateToken(username string) string {
	return username + ":" + h.secret
}

var ErrExpired = errors.New("token expired")
