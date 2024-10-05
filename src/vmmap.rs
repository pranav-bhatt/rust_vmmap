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
    ) {
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
        );
    }

    fn remove_entry(&mut self, page_num: u32, npages: u32) {
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
        );
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
    ) {
        assert!(npages > 0); //TODO: panics :(

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
mod tests {
    use super::Vmmap;

    #[test]
    fn test_vmmap_new() {
        let vmmap = Vmmap::new();
        assert!(vmmap.entries.is_empty());
    }
}