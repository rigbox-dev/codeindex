import { AuthHandler } from './auth';
import { UserRepository } from './user';

export function createApp(secret: string) {
    const auth = new AuthHandler(secret);
    const users = new UserRepository();
    return { auth, users };
}

export async function main(): Promise<void> {
    const app = createApp('supersecret');
    console.log('App created', app);
}
