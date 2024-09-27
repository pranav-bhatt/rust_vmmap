use nodit::NoditMap;
use nodit::{interval::ie, Interval};

use crate::types::{MemoryBackingType, VmmapEntry, VmmapOps};

pub struct Vmmap {
    pub entries: NoditMap<u32, Interval<u32>, VmmapEntry>, // Keyed by `page_num`
    pub cached_entry: Option<VmmapEntry>,                  // TODO: is this still needed?
                                                           // Use Option for safety
}

impl Vmmap {
    fn new() -> Self {
        Vmmap {
            entries: NoditMap::new(),
            cached_entry: None,
        }
    }

    fn spilt_and_insert() {}
}

impl VmmapOps for Vmmap {
    fn add_entry(&mut self, vmmap_entry_ref: VmmapEntry) {
        self.entries.insert_strict(
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
        self.entries
            .insert_overwrite(ie(new_region_start_page, new_region_end_page), new_entry);

        if remove {
            // strange way to do this, but this is the best using the library we have at hand
            // while also maintaining the shrunk down entries
            // using remove first, then insert will cause us to lose existing entries
            self.entries
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
            self.entries.insert_overwrite(interval, interval_val);
        }
    }
}
