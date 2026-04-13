from dataclasses import dataclass

@dataclass
class AuthRequest:
    username: str
    password: str

@dataclass
class AuthToken:
    token: str
    expires_at: int

class AuthError(Exception):
    pass
