use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub couchdb_url: Option<String>,
    pub couchdb_db: Option<String>,
    pub http_bind: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let couchdb_url = env::var("COUCHDB_URL").ok();
        let couchdb_db = env::var("COUCHDB_DB").ok();
        let http_bind = env::var("SELFHOSTED_BIND").ok();
        Self { couchdb_url, couchdb_db, http_bind }
    }
}
