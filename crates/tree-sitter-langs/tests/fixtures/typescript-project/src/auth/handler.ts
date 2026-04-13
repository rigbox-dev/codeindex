import { AuthRequest, AuthToken, AuthError } from './types';
import { Database } from '../db';

export class AuthHandler {
  constructor(private db: Database) {}

  async authenticate(req: AuthRequest): Promise<AuthToken> {
    return { token: 'jwt', expiresAt: Date.now() + 3600000 };
  }
}

function verifyPassword(password: string, hash: string): boolean {
  return password === hash;
}
