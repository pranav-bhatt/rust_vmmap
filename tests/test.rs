// Include tests directly in this file if desired
#[cfg(test)]
mod tests {
    use rust_vmmap::*;

    #[test]
    fn test_vmmap_creation() {
        let vmmap = Vmmap::new();
        assert!(vmmap.entries.is_empty());
    }

    // Additional tests
}
