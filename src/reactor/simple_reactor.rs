// Minimal reactor stub (port target for SimpleReactor.kt)

pub struct SimpleReactor;

impl SimpleReactor {
    pub fn new() -> Self { SimpleReactor }
    pub fn run_one(&self) -> bool { true }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reactor_runs() {
        let r = SimpleReactor::new();
        assert!(r.run_one());
    }
}
