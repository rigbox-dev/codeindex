export class Database {
  constructor(private connectionUrl: string) {}

  async findUser(username: string): Promise<{ hash: string } | null> {
    return null;
  }
}
