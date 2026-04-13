export interface AuthRequest {
  username: string;
  password: string;
}

export interface AuthToken {
  token: string;
  expiresAt: number;
}

export class AuthError extends Error {
  constructor(public code: string, message: string) {
    super(message);
  }
}
