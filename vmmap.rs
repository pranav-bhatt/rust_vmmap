use std::io;

use nodit::NoditMap;
use nodit::{interval::ie, Interval};

use crate::constants::{
    // MAP_PRIVATE, O_ACCMODE, O_RDONLY, O_RDWR, O_WRONLY, PAGESIZE,
    PROT_EXEC,
    PROT_NONE,
    PROT_READ,
    PROT_WRITE,
};
use crate::types::{MemoryBackingType, VmmapEntry, VmmapOps};

pub struct Vmmap {
    pub entries: NoditMap<u32, Interval<u32>, VmmapEntry>, // Keyed by `page_num`
    pub cached_entry: Option<VmmapEntry>,                  // TODO: is this still needed?
                                                           // Use Option for safety
}

#[allow(dead_code)]
impl Vmmap {
    pub fn new() -> Self {
        Vmmap {
            entries: NoditMap::new(),
            cached_entry: None,
        }
    }

    fn round_page_num_up_to_map_multiple(&self, npages: u32, pages_per_map: u32) -> u32 {
        (npages + pages_per_map - 1) & !(pages_per_map - 1)
    }

    fn trunc_page_num_down_to_map_multiple(&self, npages: u32, pages_per_map: u32) -> u32 {
        npages & !(pages_per_map - 1)
    }

    fn visit() {}

    fn debug() {}
}

impl VmmapOps for Vmmap {
    fn add_entry(&mut self, vmmap_entry_ref: VmmapEntry) {
        let _ = self.entries.insert_strict(
            // pages x to y, y included
            ie(
                vmmap_entry_ref.page_num,
                vmmap_entry_ref.page_num + vmmap_entry_ref.npages,
            ),
            vmmap_entry_ref,
        );
    }

    fn add_entry_with_override(
        &mut self,
        page_num: u32,
        npages: u32,
        prot: i32,
        maxprot: i32,
        flags: i32,
        backing: MemoryBackingType,
        file_offset: i64,
        file_size: i64,
        cage_id: u64,
    ) -> Result<(), io::Error> {
        self.update(
            page_num,
            npages,
            prot,
            maxprot,
            flags,
            backing,
            false,
            file_offset,
            file_size,
            cage_id,
        )
    }

    /// This function will not return any errors pertaining to the page number not mapping
    /// to any existing pages, as the remove operation is done on a best efforts basis:
    /// 1. First an insert overwrite operation with the below page range is performed, causing
    /// a new interval to be created over the provided page range, appropriately partitioning
    /// boundary pages.
    /// 2. This new interval is then deleted, leaving the underlying range unmapped
    fn remove_entry(&mut self, page_num: u32, npages: u32) -> Result<(), io::Error> {
        self.update(
            page_num,
            npages,
            0,
            0,
            0,
            MemoryBackingType::None,
            true,
            0,
            0,
            0,
        )
    }

    fn update(
        &mut self,
        page_num: u32,
        npages: u32,
        prot: i32,
        maxprot: i32,
        flags: i32,
        backing: MemoryBackingType,
        remove: bool,
        file_offset: i64,
        file_size: i64,
        cage_id: u64,
    ) -> Result<(), io::Error> {
        if npages == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Number of pages cannot be zero",
            ));
        }

        let new_region_end_page = page_num + npages;
        let new_region_start_page = page_num; // just for ease of understanding

        // Insert the new entry if not marked for removal
        let new_entry = VmmapEntry {
            page_num,
            npages,
            prot,
            maxprot,
            flags,
            backing,
            file_offset,
            file_size,
            removed: false,
            cage_id,
        };
        let _ = self
            .entries
            .insert_overwrite(ie(new_region_start_page, new_region_end_page), new_entry);

        if remove {
            // strange way to do this, but this is the best using the library we have at hand
            // while also maintaining the shrunk down entries
            // using remove first, then insert will cause us to lose existing entries
            let _ = self
                .entries
                .remove_overlapping(ie(new_region_start_page, new_region_end_page));
        }

        Ok(())
    }

    fn change_prot(&mut self, page_num: u32, npages: u32, new_prot: i32) {
        let new_region_end_page = page_num + npages;
        let new_region_start_page = page_num;

        let mut to_insert = Vec::new();

        for (overlap_interval, entry) in self
            .entries
            .overlapping_mut(ie(new_region_start_page, new_region_end_page))
        {
            let mut ent_start = overlap_interval.start();
            let ent_end = overlap_interval.end();

            if ent_start < new_region_start_page && ent_end > new_region_start_page {
                to_insert.push(ie(new_region_start_page, ent_end));
                ent_start = new_region_start_page; // need to update incase next condition is true
            }
            if ent_start < new_region_end_page && ent_end > new_region_end_page {
                to_insert.push(ie(ent_start, new_region_end_page));
            } else {
                entry.prot = new_prot;
            }
        }

        for interval in to_insert {
            let mut interval_val = self.entries.get_at_point(interval.start()).unwrap().clone();
            interval_val.prot = new_prot;
            let _ = self.entries.insert_overwrite(interval, interval_val);
        }
    }

    fn check_existing_mapping(&self, page_num: u32, npages: u32, prot: i32) -> bool {
        let region_end_page = page_num + npages;
        let region_interval = ie(page_num, region_end_page);

        // If no overlap, return false
        if !self.entries.overlaps(region_interval) {
            return false;
        }

        let mut current_page = page_num;

        // Iterate over overlapping intervals
        for (_interval, entry) in self.entries.overlapping(region_interval) {
            let ent_end_page = entry.page_num + entry.npages;
            let flags = entry.maxprot;

            // Case 1: Fully inside the existing entry
            if entry.page_num <= current_page && region_end_page <= ent_end_page {
                return (prot & !flags) == 0;
            }

            // Case 2: Overlaps with the current entry
            if entry.page_num <= current_page && current_page < ent_end_page {
                if (prot & !flags) != 0 {
                    return false;
                }
                current_page = ent_end_page; // Move to the next region
            }

            // Case 3: If there's a gap (no backing store), return false
            if current_page < entry.page_num {
                return false;
            }
        }

        false
    }

    fn check_addr_mapping(&mut self, page_num: u32, npages: u32, prot: i32) -> Option<u32> {
        let region_end_page = page_num + npages;

        // First, check if the cached entry can be used
        if let Some(ref cached_entry) = self.cached_entry {
            let ent_end_page = cached_entry.page_num + cached_entry.npages;
            let mut flags = cached_entry.prot;

            // If the protection is not PROT_NONE, enforce PROT_READ
            if flags & (PROT_EXEC | PROT_READ | PROT_WRITE) != PROT_NONE {
                flags |= PROT_READ;
            }

            if cached_entry.page_num <= page_num && region_end_page <= ent_end_page {
                if prot & !flags == 0 {
                    return Some(ent_end_page); // Mapping found inside the cached entry
                }
            }
        }

        // If no cached entry, check the overlapping regions in memory map
        let mut current_page = page_num;
        for (_, entry) in self.entries.overlapping(ie(page_num, region_end_page)) {
            let ent_end_page = entry.page_num + entry.npages;
            let mut flags = entry.prot;

            // If the protection is not PROT_NONE, enforce PROT_READ
            if flags & (PROT_EXEC | PROT_READ | PROT_WRITE) != PROT_NONE {
                flags |= PROT_READ;
            }

            if entry.page_num <= current_page && region_end_page <= ent_end_page {
                // Mapping is fully inside the current entry
                self.cached_entry = Some(entry.clone()); // Cache the entry
                if prot & !flags == 0 {
                    return Some(ent_end_page);
                }
            } else if entry.page_num <= current_page && current_page < ent_end_page {
                // Mapping overlaps with this entry
                if prot & !flags != 0 {
                    return None;
                }
                current_page = ent_end_page; // Move to next region
            } else if current_page < entry.page_num {
                // There's a gap between entries, return failure
                return None;
            }
        }

        // If no valid mapping is found, return None
        None
    }

    fn find_page(&self, page_num: u32) -> Option<&VmmapEntry> {
        self.entries.get_at_point(page_num)
    }

    fn find_page_mut(&mut self, page_num: u32) -> Option<&mut VmmapEntry> {
        self.entries.get_at_point_mut(page_num)
    }

    fn last_entry(&self) -> Option<(&Interval<u32>, &VmmapEntry)> {
        self.entries.last_key_value()
    }

    fn first_entry(&self) -> Option<(&Interval<u32>, &VmmapEntry)> {
        self.entries.first_key_value()
    }

    fn double_ended_iter(&self) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &VmmapEntry)> {
        self.entries.iter()
    }

    fn double_ended_iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &mut VmmapEntry)> {
        self.entries.iter_mut()
    }

    fn find_page_iter(
        &self,
        page_num: u32,
    ) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &VmmapEntry)> {
        if let Some(last_entry) = self.last_entry() {
            self.entries.overlapping(ie(page_num, last_entry.0.end()))
        } else {
            // Return an empty iterator if no last_entry
            self.entries.overlapping(ie(page_num, page_num))
        }
    }

    fn find_page_iter_mut(
        &mut self,
        page_num: u32,
    ) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &mut VmmapEntry)> {
        if let Some(last_entry) = self.last_entry() {
            self.entries
                .overlapping_mut(ie(page_num, last_entry.0.end()))
        } else {
            // Return an empty iterator if no last_entry
            self.entries.overlapping_mut(ie(page_num, page_num))
        }
    }

    fn find_space(&self, npages: u32) -> Option<Interval<u32>> {
        let start = self.first_entry();
        let end = self.last_entry();

        if start == None || end == None {
            return None;
        } else {
            let start_unwrapped = start.unwrap().0.start();
            let end_unwrapped = end.unwrap().0.end();

            let desired_space = npages + 1; // TODO: check if this is correct

            for gap in self
                .entries
                .gaps_trimmed(ie(start_unwrapped, end_unwrapped))
            {
                if gap.end() - gap.start() >= desired_space {
                    return Some(gap);
                }
            }
        }

        None
    }

    fn find_space_above_hint(&self, npages: u32, hint: u32) -> Option<Interval<u32>> {
        let start = hint;
        let end = self.last_entry();

        if end == None {
            return None;
        } else {
            let end_unwrapped = end.unwrap().0.end();

            let desired_space = npages + 1; // TODO: check if this is correct

            for gap in self.entries.gaps_trimmed(ie(start, end_unwrapped)) {
                if gap.end() - gap.start() >= desired_space {
                    return Some(gap);
                }
            }
        }

        None
    }

    fn find_map_space(&self, num_pages: u32, pages_per_map: u32) -> Option<Interval<u32>> {
        let start = self.first_entry();
        let end = self.last_entry();

        if start == None || end == None {
            return None;
        } else {
            let start_unwrapped = start.unwrap().0.start();
            let end_unwrapped = end.unwrap().0.end();

            let rounded_num_pages =
                self.round_page_num_up_to_map_multiple(num_pages, pages_per_map);

            for gap in self
                .entries
                .gaps_trimmed(ie(start_unwrapped, end_unwrapped))
            {
                let aligned_start_page =
                    self.trunc_page_num_down_to_map_multiple(gap.start(), pages_per_map);
                let aligned_end_page =
                    self.round_page_num_up_to_map_multiple(gap.end(), pages_per_map);

                let gap_size = aligned_end_page - aligned_start_page;
                if gap_size >= rounded_num_pages {
                    return Some(ie(aligned_end_page - rounded_num_pages, aligned_end_page));
                }
            }
        }

        None
    }

    fn find_map_space_with_hint(
        &self,
        num_pages: u32,
        pages_per_map: u32,
        hint: u32,
    ) -> Option<Interval<u32>> {
        let start = hint;
        let end = self.last_entry();

        if end == None {
            return None;
        } else {
            let end_unwrapped = end.unwrap().0.end();

            let rounded_num_pages =
                self.round_page_num_up_to_map_multiple(num_pages, pages_per_map);

            for gap in self.entries.gaps_trimmed(ie(start, end_unwrapped)) {
                let aligned_start_page =
                    self.trunc_page_num_down_to_map_multiple(gap.start(), pages_per_map);
                let aligned_end_page =
                    self.round_page_num_up_to_map_multiple(gap.end(), pages_per_map);

                let gap_size = aligned_end_page - aligned_start_page;
                if gap_size >= rounded_num_pages {
                    return Some(ie(aligned_end_page - rounded_num_pages, aligned_end_page));
                }
            }
        }

        None
    }
}

#[cfg(test)]
pub mod test_vmmap_util {
    pub fn create_default_vmmap() {}
}

#[cfg(test)]
mod tests {
    use nodit::interval::ie;

    use crate::types::VmmapOps;
    use crate::vmmap_entries::test_vmmap_entry_util::*;

    use super::Vmmap;

    #[test]
    fn test_add_valid_vmmap_entry() {
        let mut vmmap = Vmmap::new();
        assert!(vmmap.entries.is_empty());

        // trying to add invalid entry should fail
        let invalid_vmmap_entry = create_invalid_vmmap_entry();

        let add_invalid_vmmap_entry = vmmap.add_entry_with_override(
            invalid_vmmap_entry.page_num,
            invalid_vmmap_entry.npages,
            invalid_vmmap_entry.prot,
            invalid_vmmap_entry.maxprot,
            invalid_vmmap_entry.flags,
            invalid_vmmap_entry.backing,
            invalid_vmmap_entry.file_offset,
            invalid_vmmap_entry.file_size,
            invalid_vmmap_entry.cage_id,
        );

        assert!(add_invalid_vmmap_entry.is_err());

        // add proper entry
        let vmmap_entry_0_10 = create_default_vmmap_entry();

        let add_vmmap_entry = vmmap.add_entry_with_override(
            vmmap_entry_0_10.page_num,
            vmmap_entry_0_10.npages,
            vmmap_entry_0_10.prot,
            vmmap_entry_0_10.maxprot,
            vmmap_entry_0_10.flags,
            vmmap_entry_0_10.backing,
            vmmap_entry_0_10.file_offset,
            vmmap_entry_0_10.file_size,
            vmmap_entry_0_10.cage_id,
        );

        assert!(add_vmmap_entry.is_ok());
        assert_eq!(vmmap.entries.len(), 1);
        assert_eq!(vmmap.entries.get_at_point(0), Some(&vmmap_entry_0_10));
        assert_eq!(vmmap.entries.get_at_point(10), None);
        assert!(vmmap.entries.contains_interval(ie(0, 10)));

        let mut vmmap_entry_5_10 = create_default_vmmap_entry();
        vmmap_entry_5_10.page_num = 5;
        vmmap_entry_5_10.npages = 3;

        let add_overwritten_vmmap_entry = vmmap.add_entry_with_override(
            vmmap_entry_5_10.page_num,
            vmmap_entry_5_10.npages,
            vmmap_entry_5_10.prot,
            vmmap_entry_5_10.maxprot,
            vmmap_entry_5_10.flags,
            vmmap_entry_5_10.backing,
            vmmap_entry_5_10.file_offset,
            vmmap_entry_5_10.file_size,
            vmmap_entry_5_10.cage_id,
        );

        assert!(add_overwritten_vmmap_entry.is_ok());
        assert_eq!(vmmap.entries.len(), 3);
        assert_eq!(vmmap.entries.get_at_point(0), Some(&vmmap_entry_0_10));
        assert_eq!(vmmap.entries.get_at_point(5), Some(&vmmap_entry_5_10));
        assert_eq!(vmmap.entries.get_at_point(8), Some(&vmmap_entry_0_10));
        assert_eq!(vmmap.entries.get_at_point(10), None);
        // just checks to see if all values in range are allocated
        assert!(vmmap.entries.contains_interval(ie(0, 10)));

    }

    #[test]
    fn test_remove_vmmap_entry() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        let add_vmmap_entry = vmmap.add_entry_with_override(
            vmmap_entry.page_num,
            vmmap_entry.npages,
            vmmap_entry.prot,
            vmmap_entry.maxprot,
            vmmap_entry.flags,
            vmmap_entry.backing,
            vmmap_entry.file_offset,
            vmmap_entry.file_size,
            vmmap_entry.cage_id,
        );
        assert!(add_vmmap_entry.is_ok());

        // try to remove from some invalid range, shouldn't return any errors (see fn docs)
        let remove_non_existant = vmmap.remove_entry(11, 1);
        assert!(remove_non_existant.is_ok());
    }

    // Test changing protection within the maximum allowed protection
    #[test]
    fn test_change_prot_within_maxprot() {
        // Initialize Vmmap and create a default entry
        let mut vmmap = Vmmap::new();
        let mut vmmap_entry = create_default_vmmap_entry();
        vmmap_entry.page_num = 0;
        vmmap_entry.npages = 10;
        vmmap_entry.prot = ProtFlags::READ;
        vmmap_entry.maxprot = ProtFlags::READ | ProtFlags::WRITE;

        // Add the initial entry to the Vmmap
        let add_result = vmmap.add_entry_with_override(
            vmmap_entry.page_num,
            vmmap_entry.npages,
            vmmap_entry.prot,
            vmmap_entry.maxprot,
            vmmap_entry.flags,
            vmmap_entry.backing,
            vmmap_entry.file_offset,
            vmmap_entry.file_size,
            vmmap_entry.cage_id,
        );
        assert!(add_result.is_ok(), "Failed to add initial entry");

        // Attempt to change protection within maxprot
        let change_result = vmmap.change_prot(0, 5, ProtFlags::READ | ProtFlags::WRITE);
        assert!(change_result.is_ok(), "Failed to change protection within maxprot");

        // Verify the changes
        let changed_entry = vmmap.entries.get_at_point(0).unwrap();
        assert_eq!(changed_entry.prot, ProtFlags::READ | ProtFlags::WRITE, "Protection not changed as expected");
        assert_eq!(changed_entry.npages, 5, "Number of pages changed unexpectedly");
    }

    // Test changing protection beyond the maximum allowed protection
    #[test]
    fn test_change_prot_beyond_maxprot() {
        // Initialize Vmmap and create a default entry
        let mut vmmap = Vmmap::new();
        let mut vmmap_entry = create_default_vmmap_entry();
        vmmap_entry.page_num = 0;
        vmmap_entry.npages = 10;
        vmmap_entry.prot = ProtFlags::READ;
        vmmap_entry.maxprot = ProtFlags::READ | ProtFlags::WRITE;

        // Add the initial entry to the Vmmap
        let add_result = vmmap.add_entry_with_override(
            vmmap_entry.page_num,
            vmmap_entry.npages,
            vmmap_entry.prot,
            vmmap_entry.maxprot,
            vmmap_entry.flags,
            vmmap_entry.backing,
            vmmap_entry.file_offset,
            vmmap_entry.file_size,
            vmmap_entry.cage_id,
        );
        assert!(add_result.is_ok(), "Failed to add initial entry");

        // Attempt to change protection beyond maxprot
        let invalid_change = vmmap.change_prot(5, 5, ProtFlags::READ | ProtFlags::WRITE | ProtFlags::EXEC);
        assert!(invalid_change.is_err(), "Changing protection beyond maxprot should fail");
    }

    // Test changing protection for a non-existent range
    #[test]
    fn test_change_prot_non_existent_range() {
        // Initialize Vmmap and create a default entry
        let mut vmmap = Vmmap::new();
        let mut vmmap_entry = create_default_vmmap_entry();
        vmmap_entry.page_num = 0;
        vmmap_entry.npages = 10;
        vmmap_entry.prot = ProtFlags::READ;
        vmmap_entry.maxprot = ProtFlags::READ | ProtFlags::WRITE;

        // Add the initial entry to the Vmmap
        let add_result = vmmap.add_entry_with_override(
            vmmap_entry.page_num,
            vmmap_entry.npages,
            vmmap_entry.prot,
            vmmap_entry.maxprot,
            vmmap_entry.flags,
            vmmap_entry.backing,
            vmmap_entry.file_offset,
            vmmap_entry.file_size,
            vmmap_entry.cage_id,
        );
        assert!(add_result.is_ok(), "Failed to add initial entry");

        // Attempt to change protection for a non-existent range
        let non_existent_change = vmmap.change_prot(20, 5, ProtFlags::READ);
        assert!(non_existent_change.is_err(), "Changing protection for non-existent range should fail");
    }

    // Test changing protection across multiple entries
    #[test]
    fn test_change_prot_across_multiple_entries() {
        // Initialize Vmmap and create a default entry
        let mut vmmap = Vmmap::new();
        let mut vmmap_entry = create_default_vmmap_entry();
        vmmap_entry.page_num = 0;
        vmmap_entry.npages = 10;
        vmmap_entry.prot = ProtFlags::READ;
        vmmap_entry.maxprot = ProtFlags::READ | ProtFlags::WRITE;

        // Add the initial entry to the Vmmap
        let add_result = vmmap.add_entry_with_override(
            vmmap_entry.page_num,
            vmmap_entry.npages,
            vmmap_entry.prot,
            vmmap_entry.maxprot,
            vmmap_entry.flags,
            vmmap_entry.backing,
            vmmap_entry.file_offset,
            vmmap_entry.file_size,
            vmmap_entry.cage_id,
        );
        assert!(add_result.is_ok(), "Failed to add initial entry");

        // Add a second entry
        let _ = vmmap.add_entry_with_override(10, 5, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        
        // Attempt to change protection across the boundary of two entries
        let multi_entry_change = vmmap.change_prot(8, 4, ProtFlags::READ | ProtFlags::WRITE);
        assert!(multi_entry_change.is_ok(), "Failed to change protection across multiple entries");

        // Verify the changes
        assert_eq!(vmmap.entries.len(), 3, "Unexpected number of entries after multi-entry change");
        assert_eq!(vmmap.entries.get_at_point(8).unwrap().prot, ProtFlags::READ | ProtFlags::WRITE, "Protection not changed as expected for first affected entry");
        assert_eq!(vmmap.entries.get_at_point(10).unwrap().prot, ProtFlags::READ | ProtFlags::WRITE, "Protection not changed as expected for second affected entry");
    }

    #[test]
    fn test_remove_entire_entry() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry to remove later
        let add_result = vmmap.add_entry_with_override(
            vmmap_entry.page_num,
            vmmap_entry.npages,
            vmmap_entry.prot,
            vmmap_entry.maxprot,
            vmmap_entry.flags,
            vmmap_entry.backing,
            vmmap_entry.file_offset,
            vmmap_entry.file_size,
            vmmap_entry.cage_id,
        );
        assert!(add_result.is_ok(), "Failed to add initial entry");

        // Test removing the entire entry
        let remove_result = vmmap.remove_entry(0, 10);
        assert!(remove_result.is_ok(), "Failed to remove entire entry");
        assert!(vmmap.entries.is_empty(), "Vmmap should be empty after removing the only entry");
    }

    #[test]
    fn test_remove_portion_of_entry() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add multiple entries for testing
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(10, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test removing a portion of entries
        let partial_remove_result = vmmap.remove_entry(5, 10);
        assert!(partial_remove_result.is_ok(), "Failed to remove portion of entries");
        assert_eq!(vmmap.entries.len(), 3, "Expected 3 entries after partial removal");
        assert_eq!(vmmap.entries.get_at_point(0).unwrap().npages, 5, "First entry should be truncated");
        assert_eq!(vmmap.entries.get_at_point(15).unwrap().npages, 5, "Last entry should be truncated");
    }

    #[test]
    fn test_remove_across_multiple_entries() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add multiple entries for testing
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(10, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test removing across multiple entries
        let multi_remove_result = vmmap.remove_entry(2, 18);
        assert!(multi_remove_result.is_ok(), "Failed to remove across multiple entries");
        assert_eq!(vmmap.entries.len(), 2, "Expected 2 entries after removal across multiple entries");
        assert_eq!(vmmap.entries.get_at_point(0).unwrap().npages, 2, "First entry should be truncated");
        assert_eq!(vmmap.entries.get_at_point(20).unwrap().npages, 5, "Last entry should remain unchanged");
    }

    #[test]
    fn test_remove_non_existent_range() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test removing non-existent range
        let non_existent_remove = vmmap.remove_entry(50, 10);
        assert!(non_existent_remove.is_err(), "Removing non-existent range should fail");
    }

    #[test]
    fn test_remove_with_invalid_input() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test removing with invalid input
        let invalid_remove = vmmap.remove_entry(10, 0);
        assert!(invalid_remove.is_err(), "Removing with invalid input should fail");
    }

    #[test]
    fn test_check_existing_mapping() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test fully contained mapping
        let result = vmmap.check_existing_mapping(2, 5);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return correct existing mapping range");

        // Test partially overlapping mapping at start
        let result = vmmap.check_existing_mapping(0, 15);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return correct existing mapping range");

        // Test partially overlapping mapping at end
        let result = vmmap.check_existing_mapping(5, 10);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return correct existing mapping range");

        // Test exact match
        let result = vmmap.check_existing_mapping(0, 10);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return correct existing mapping range");

        // Test non-overlapping range before
        let result = vmmap.check_existing_mapping(15, 5);
        assert!(result.is_none(), "Should not detect existing mapping");

        // Test non-overlapping range after
        let result = vmmap.check_existing_mapping(11, 5);
        assert!(result.is_none(), "Should not detect existing mapping");

        // Add another entry
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test overlapping multiple entries
        let result = vmmap.check_existing_mapping(5, 20);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return first overlapping mapping range");
    }

    #[test]
    fn test_check_existing_mapping_edge_cases() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add two entries with a gap between them
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test mapping that starts at the end of one entry and ends at the start of another
        let result = vmmap.check_existing_mapping(10, 10);
        assert!(result.is_none(), "Should not detect existing mapping in gap between entries");

        // Test mapping that starts just before an entry
        let result = vmmap.check_existing_mapping(19, 2);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (20, 10), "Should return correct existing mapping range");

        // Test mapping that ends just after an entry
        let result = vmmap.check_existing_mapping(9, 2);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return correct existing mapping range");

        // Test mapping with zero length
        let result = vmmap.check_existing_mapping(5, 0);
        assert!(result.is_none(), "Should not detect existing mapping for zero length");

        // Test mapping at the very start of the first entry
        let result = vmmap.check_existing_mapping(0, 1);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return correct existing mapping range");

        // Test mapping at the very end of the last entry
        let result = vmmap.check_existing_mapping(29, 1);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (20, 10), "Should return correct existing mapping range");

        // Test mapping that covers both entries and the gap
        let result = vmmap.check_existing_mapping(0, 30);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (0, 10), "Should return first overlapping mapping range");
    }

    #[test]
    fn test_check_existing_mapping_empty_vmmap() {
        let vmmap = Vmmap::new();

        // Test on empty Vmmap
        let result = vmmap.check_existing_mapping(0, 10);
        assert!(result.is_none(), "Should not detect existing mapping in empty Vmmap");
    }

    #[test]
    fn test_check_existing_mapping_large_values() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry at a large page number
        let _ = vmmap.add_entry_with_override(1_000_000, 1000, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test with large values
        let result = vmmap.check_existing_mapping(999_999, 1002);
        assert!(result.is_some(), "Should detect existing mapping");
        assert_eq!(result.unwrap(), (1_000_000, 1000), "Should return correct existing mapping range");

        // Test with very large values that don't overlap
        let result = vmmap.check_existing_mapping(2_000_000, 1_000_000);
        assert!(result.is_none(), "Should not detect existing mapping for non-overlapping large range");
    }


    #[test]
    fn test_check_addr_mapping() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add two entries to the Vmmap
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::READ | ProtFlags::WRITE, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test address within first entry
        let result = vmmap.check_addr_mapping(5);
        assert!(result.is_some(), "Should detect mapping for address within first entry");
        assert_eq!(result.unwrap().0, 0, "Should return correct start page for first entry");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages for first entry");

        // Test address within second entry
        let result = vmmap.check_addr_mapping(25);
        assert!(result.is_some(), "Should detect mapping for address within second entry");
        assert_eq!(result.unwrap().0, 20, "Should return correct start page for second entry");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages for second entry");

        // Test address at the start of an entry
        let result = vmmap.check_addr_mapping(20);
        assert!(result.is_some(), "Should detect mapping for address at start of entry");
        assert_eq!(result.unwrap().0, 20, "Should return correct start page");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages");

        // Test address at the end of an entry
        let result = vmmap.check_addr_mapping(9);
        assert!(result.is_some(), "Should detect mapping for address at end of entry");
        assert_eq!(result.unwrap().0, 0, "Should return correct start page");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages");

        // Test address in gap between entries
        let result = vmmap.check_addr_mapping(15);
        assert!(result.is_none(), "Should not detect mapping for address in gap");

        // Test address before first entry
        let result = vmmap.check_addr_mapping(u32::MAX);
        assert!(result.is_none(), "Should not detect mapping for address before first entry");

        // Test address after last entry
        let result = vmmap.check_addr_mapping(30);
        assert!(result.is_none(), "Should not detect mapping for address after last entry");
    }

    #[test]
    fn test_check_addr_mapping_empty_vmmap() {
        let vmmap = Vmmap::new();

        // Test on empty Vmmap
        let result = vmmap.check_addr_mapping(0);
        assert!(result.is_none(), "Should not detect mapping in empty Vmmap");
    }

    #[test]
    fn test_check_addr_mapping_large_values() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry at a large page number
        let _ = vmmap.add_entry_with_override(1_000_000, 1000, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test with address within large entry
        let result = vmmap.check_addr_mapping(1_000_500);
        assert!(result.is_some(), "Should detect mapping for address within large entry");
        assert_eq!(result.unwrap().0, 1_000_000, "Should return correct start page for large entry");
        assert_eq!(result.unwrap().1, 1000, "Should return correct number of pages for large entry");

        // Test with address just before large entry
        let result = vmmap.check_addr_mapping(999_999);
        assert!(result.is_none(), "Should not detect mapping for address just before large entry");

        // Test with address just after large entry
        let result = vmmap.check_addr_mapping(1_001_000);
        assert!(result.is_none(), "Should not detect mapping for address just after large entry");
    }

    #[test]
    fn test_find_page_basic() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry from page 0 to 10
        let _ = vmmap.add_entry_with_override(
            0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE,
            vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset,
            vmmap_entry.file_size, vmmap_entry.cage_id
        );

        // Test finding a page within the entry
        let result = vmmap.find_page(5);
        assert!(result.is_some(), "Should find page within entry");
        assert_eq!(result.unwrap().0, 0, "Should return correct start page");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages");

        // Test finding the first page of the entry
        let result = vmmap.find_page(0);
        assert!(result.is_some(), "Should find first page of entry");
        assert_eq!(result.unwrap().0, 0, "Should return correct start page");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages");

        // Test finding the last page of the entry
        let result = vmmap.find_page(9);
        assert!(result.is_some(), "Should find last page of entry");
        assert_eq!(result.unwrap().0, 0, "Should return correct start page");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages");

        // Test finding a page outside the entry
        let result = vmmap.find_page(10);
        assert!(result.is_none(), "Should not find page outside entry");
    }

    #[test]
    fn test_find_page_multiple_entries() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add multiple entries
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test finding a page in the first entry
        let result = vmmap.find_page(5);
        assert!(result.is_some(), "Should find page in first entry");
        assert_eq!(result.unwrap().0, 0, "Should return correct start page for first entry");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages for first entry");

        // Test finding a page in the second entry
        let result = vmmap.find_page(25);
        assert!(result.is_some(), "Should find page in second entry");
        assert_eq!(result.unwrap().0, 20, "Should return correct start page for second entry");
        assert_eq!(result.unwrap().1, 10, "Should return correct number of pages for second entry");

        // Test finding a page in the gap between entries
        let result = vmmap.find_page(15);
        assert!(result.is_none(), "Should not find page in gap between entries");
    }

    #[test]
    fn test_find_page_empty_vmmap() {
        let vmmap = Vmmap::new();

        // Test finding a page in an empty Vmmap
        let result = vmmap.find_page(0);
        assert!(result.is_none(), "Should not find page in empty Vmmap");
    }

    #[test]
    fn test_find_page_large_values() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add an entry at a large page number
        let _ = vmmap.add_entry_with_override(1_000_000, 1000, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test finding a page within the large entry
        let result = vmmap.find_page(1_000_500);
        assert!(result.is_some(), "Should find page within large entry");
        assert_eq!(result.unwrap().0, 1_000_000, "Should return correct start page for large entry");
        assert_eq!(result.unwrap().1, 1000, "Should return correct number of pages for large entry");

        // Test finding a page just before the large entry
        let result = vmmap.find_page(999_999);
        assert!(result.is_none(), "Should not find page just before large entry");

        // Test finding a page just after the large entry
        let result = vmmap.find_page(1_001_000);
        assert!(result.is_none(), "Should not find page just after large entry");
    }
    
    #[test]
    fn test_find_page_mut() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add entries for testing
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test finding and modifying a page in the first entry
        let result = vmmap.find_page_mut(5);
        assert!(result.is_some(), "Should find page in first entry");
        if let Some(entry) = result {
            entry.prot = ProtFlags::WRITE;
        }
        assert_eq!(vmmap.entries.get_at_point(5).unwrap().prot, ProtFlags::WRITE, "Protection should be changed");

        // Test finding a page in the gap between entries
        let result = vmmap.find_page_mut(15);
        assert!(result.is_none(), "Should not find page in gap between entries");

        // Test finding a page outside of any entry
        let result = vmmap.find_page_mut(30);
        assert!(result.is_none(), "Should not find page outside of any entry");
    }

    #[test]
    fn test_find_page_iter() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add entries for testing
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::WRITE, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test iterating over all pages
        let mut iter = vmmap.find_page_iter(0);
        assert_eq!(iter.next().unwrap().prot, ProtFlags::READ, "First entry should have READ protection");
        assert_eq!(iter.next().unwrap().prot, ProtFlags::WRITE, "Second entry should have WRITE protection");
        assert!(iter.next().is_none(), "Iterator should be exhausted");

        // Test iterating from middle of an entry
        let mut iter = vmmap.find_page_iter(5);
        assert_eq!(iter.next().unwrap().prot, ProtFlags::READ, "Should start from first entry");
        assert_eq!(iter.next().unwrap().prot, ProtFlags::WRITE, "Should continue to second entry");
        assert!(iter.next().is_none(), "Iterator should be exhausted");

        // Test iterating from gap between entries
        let mut iter = vmmap.find_page_iter(15);
        assert_eq!(iter.next().unwrap().prot, ProtFlags::WRITE, "Should start from second entry");
        assert!(iter.next().is_none(), "Iterator should be exhausted");
    }

    #[test]
    fn test_find_page_iter_mut() {
        let mut vmmap = Vmmap::new();
        let vmmap_entry = create_default_vmmap_entry();

        // Add entries for testing
        let _ = vmmap.add_entry_with_override(0, 10, ProtFlags::READ, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);
        let _ = vmmap.add_entry_with_override(20, 10, ProtFlags::WRITE, ProtFlags::READ | ProtFlags::WRITE, vmmap_entry.flags, vmmap_entry.backing, vmmap_entry.file_offset, vmmap_entry.file_size, vmmap_entry.cage_id);

        // Test modifying entries through the iterator
        let mut iter = vmmap.find_page_iter_mut(0);
        if let Some(entry) = iter.next() {
            entry.prot = ProtFlags::READ | ProtFlags::WRITE;
        }
        if let Some(entry) = iter.next() {
            entry.prot = ProtFlags::READ;
        }

        // Verify modifications
        assert_eq!(vmmap.entries.get_at_point(0).unwrap().prot, ProtFlags::READ | ProtFlags::WRITE, "First entry should be modified");
        assert_eq!(vmmap.entries.get_at_point(20).unwrap().prot, ProtFlags::READ, "Second entry should be modified");

        // Test iterating from middle of an entry
        let mut iter = vmmap.find_page_iter_mut(5);
        assert!(iter.next().is_some(), "Should return first entry");
        assert!(iter.next().is_some(), "Should return second entry");
        assert!(iter.next().is_none(), "Iterator should be exhausted");

        // Test iterating from gap between entries
        let mut iter = vmmap.find_page_iter_mut(15);
        assert!(iter.next().is_some(), "Should return second entry");
        assert!(iter.next().is_none(), "Iterator should be exhausted");
    }

}
        
    


