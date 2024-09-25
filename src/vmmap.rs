use std::collections::BTreeMap;

use crate::types::{MemoryBackingType, VmmapEntryOps, VmmapOps};
use crate::vmmap_entries::VmmapEntry;

pub struct Vmmap {
    entries: BTreeMap<u32, Box<dyn VmmapEntryOps>>, // Keyed by `page_num`
    cached_entry: Option<Box<dyn VmmapEntryOps>>,   // Use Option for safety
}

impl Vmmap {
    fn new() -> Self {
        Vmmap {
            entries: BTreeMap::new(),
            cached_entry: None,
        }
    }
}

impl VmmapOps for Vmmap {
    fn add_entry(&mut self, vmmap_entry_ref: Box<dyn VmmapEntryOps>) {
        let page_num = vmmap_entry_ref.get_key();
        self.entries.insert(page_num, vmmap_entry_ref);
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
        offset: i64,
        file_size: i64,
    ) {
        let new_region_end_page = page_num + npages;

        // Ensure we have valid input
        assert!(npages > 0);

        // Iterate over existing entries and update as needed
        let mut to_remove = Vec::new();
        let mut to_insert = Vec::new();
        for (&entry_page_num, entry) in self.entries.range_mut(..) {
            let ent_end_page = entry.get_key() + entry.get_size();
            let additional_offset = ((new_region_end_page - entry.get_key()) << 12) as i64;

            if entry.get_key() < page_num && new_region_end_page < ent_end_page {
                // Case 1: Split the entry into two, with new mapping in the middle
                let split_entry = Box::new(VmmapEntry::new(
                    new_region_end_page,
                    (ent_end_page - new_region_end_page),
                    entry.get_protection(),
                    entry.get_max_protection(),
                    entry.get_flags(),
                    false,
                    (entry.get_offset() + additional_offset) as i64,
                    entry.get_file_size(),
                    0,
                    backing,
                ));
                to_insert.push((new_region_end_page, split_entry));
                entry.set_size((page_num - entry.get_key()) as u32);
                break;
            } else if entry.get_key() < page_num && page_num < ent_end_page {
                // Case 2: New mapping overlaps end of the existing mapping
                entry.set_size((page_num - entry.get_key()) as u32);
            } else if entry.get_key() < new_region_end_page && new_region_end_page < ent_end_page {
                // Case 3: New mapping overlaps the start of the existing mapping
                entry.set_key(new_region_end_page);
                entry.set_size((ent_end_page - new_region_end_page) as u32);
                entry.set_offset(entry.get_offset() + additional_offset);
                break;
            } else if page_num <= entry.get_key() && ent_end_page <= new_region_end_page {
                // Case 4: New mapping completely covers the existing entry
                entry.set_removed(true);
                to_remove.push(entry_page_num);
                // if Some(entry) == self.cached_entry.as_ref() {
                //     self.cached_entry = None;
                // }
            }
        }

        // Remove marked entries
        for key in to_remove {
            self.entries.remove(&key);
        }

        // Insert the split entries
        for (key, value) in to_insert {
            self.entries.insert(key, value);
        }

        // Insert the new entry if not marked for removal
        if !remove {
            let new_entry = Box::new(VmmapEntry {
                page_num,
                npages: npages as u32,
                prot,
                maxprot,
                flags,
                removed: false,
                offset,
                file_size,
                cage_id: 0,
                backing,
            });
            self.entries.insert(page_num, new_entry);
        }
    }
}
