//! WebSearch API Format (Brave, Tavily, Serper)

use crate::types::*;

pub struct BraveSearchClient;
impl BraveSearchClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::BraveSearch.base_url() }
}

pub struct TavilySearchClient;
impl TavilySearchClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::TavilySearch.base_url() }
}

pub struct SerperSearchClient;
impl SerperSearchClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::SerperSearch.base_url() }
}
