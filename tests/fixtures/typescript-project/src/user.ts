export interface User {
    id: number;
    username: string;
    email: string;
}

export class UserRepository {
    private users: Map<number, User> = new Map();

    findById(id: number): User | undefined {
        return this.users.get(id);
    }

    findByUsername(username: string): User | undefined {
        for (const user of this.users.values()) {
            if (user.username === username) {
                return user;
            }
        }
        return undefined;
    }

    save(user: User): void {
        this.users.set(user.id, user);
    }

    delete(id: number): boolean {
        return this.users.delete(id);
    }
}
