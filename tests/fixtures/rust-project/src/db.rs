pub struct Database {
    connection: String,
}

impl Database {
    pub fn new(url: &str) -> Self {
        Self { connection: url.to_string() }
    }

    pub fn find_user(&self, username: &str) -> Option<String> {
        None
    }
}
