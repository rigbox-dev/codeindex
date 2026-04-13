from dataclasses import dataclass
from typing import Dict, Optional


@dataclass
class User:
    id: int
    username: str
    email: str


class UserRepository:
    def __init__(self):
        self._users: Dict[int, User] = {}

    def find_by_id(self, user_id: int) -> Optional[User]:
        return self._users.get(user_id)

    def find_by_username(self, username: str) -> Optional[User]:
        for user in self._users.values():
            if user.username == username:
                return user
        return None

    def save(self, user: User) -> None:
        self._users[user.id] = user

    def delete(self, user_id: int) -> bool:
        if user_id in self._users:
            del self._users[user_id]
            return True
        return False
