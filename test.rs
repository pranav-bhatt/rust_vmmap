mod common;

// Include tests directly in this file if desired
#[cfg(test)]
mod tests {
    // use rust_vmmap::*;

    use crate::common;

    #[test]
    fn tests() {
        common::setup();
        //TODO: implement integration test

        let mut vmmap = Vmmap::new();
        
        // Test adding a valid entry (should pass)
        let entry = VmmapEntry::new(0, 100, PROT_READ | PROT_WRITE, 0);
        assert!(vmmap.add_entry(entry).is_ok());
        assert_eq!(vmmap.entries().len(), 1);
    
        // Test adding an overlapping entry (should fail)
        let overlapping_entry = VmmapEntry::new(50, 150, PROT_READ, 0);
        assert!(vmmap.add_entry(overlapping_entry).is_err());
        assert_eq!(vmmap.entries().len(), 1);
    
        // Test adding an adjacent entry (should pass)
        let adjacent_entry = VmmapEntry::new(100, 200, PROT_READ, 0);
        assert!(vmmap.add_entry(adjacent_entry).is_ok());
        assert_eq!(vmmap.entries().len(), 2);
    
        // Test adding an entry with invalid protection flags (should fail)
        let invalid_prot_entry = VmmapEntry::new(300, 400, 0xFF, 0);
        assert!(vmmap.add_entry(invalid_prot_entry).is_err());
        assert_eq!(vmmap.entries().len(), 2);

    }

    
    
}
