export interface AuthRequest {
    username: string;
    password: string;
}

export interface AuthToken {
    token: string;
    expiresAt: number;
}

export interface Authenticator {
    authenticate(req: AuthRequest): Promise<AuthToken>;
}

export class AuthError extends Error {
    constructor(public code: string, message: string) {
        super(message);
        this.name = 'AuthError';
    }
}

export class AuthHandler implements Authenticator {
    constructor(private readonly secret: string) {}

    async authenticate(req: AuthRequest): Promise<AuthToken> {
        if (!req.username || !req.password) {
            throw new AuthError('INVALID_CREDENTIALS', 'Missing credentials');
        }
        const token = this.generateToken(req.username);
        return { token, expiresAt: Date.now() + 3600 * 1000 };
    }

    private generateToken(username: string): string {
        return `${username}:${this.secret}`;
    }
}
