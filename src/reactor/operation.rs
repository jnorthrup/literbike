// I/O operation interests and readiness flags (port of Trikeshed IOOperation.kt)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Read,
    Write,
    Connect,
    Accept,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interest {
    pub readable: bool,
    pub writable: bool,
}

impl Interest {
    pub fn read() -> Self {
        Self { readable: true, writable: false }
    }
    pub fn write() -> Self {
        Self { readable: false, writable: true }
    }
    pub fn read_write() -> Self {
        Self { readable: true, writable: true }
    }
    pub fn contains(&self, op: Operation) -> bool {
        match op {
            Operation::Read | Operation::Connect | Operation::Accept => self.readable,
            Operation::Write => self.writable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interest_read_write() {
        let i = Interest::read_write();
        assert!(i.contains(Operation::Read));
        assert!(i.contains(Operation::Write));
    }
}
