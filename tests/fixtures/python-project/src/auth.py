from dataclasses import dataclass
from typing import Optional


@dataclass
class AuthRequest:
    username: str
    password: str


@dataclass
class AuthToken:
    token: str
    expires_at: int


class AuthError(Exception):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


class Authenticator:
    def authenticate(self, req: AuthRequest) -> AuthToken:
        raise NotImplementedError


class AuthHandler(Authenticator):
    def __init__(self, secret: str):
        self._secret = secret

    def authenticate(self, req: AuthRequest) -> AuthToken:
        if not req.username or not req.password:
            raise AuthError("INVALID_CREDENTIALS", "Missing credentials")
        token = self._generate_token(req.username)
        return AuthToken(token=token, expires_at=3600)

    def _generate_token(self, username: str) -> str:
        return f"{username}:{self._secret}"
