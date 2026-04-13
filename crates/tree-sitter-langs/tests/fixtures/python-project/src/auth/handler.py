from .types import AuthRequest, AuthToken, AuthError

class AuthHandler:
    def __init__(self, db):
        self.db = db

    def authenticate(self, req):
        return None

def verify_password(password: str, hash: str) -> bool:
    return password == hash
