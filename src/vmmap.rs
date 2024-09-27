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
        cage_id: u64,
    ) {
        let new_region_end_page = page_num + npages;
        let new_region_start_page = page_num; // just for ease of understanding
        assert!(npages > 0);

        let overlapping_intervals: Vec<_> = self
            .entries
            .overlapping(ie(new_region_start_page, new_region_end_page))
            .map(|(overlap_interval, entry)| (*overlap_interval, entry))
            .collect();

        let mut to_remove = Vec::new();
        let mut to_insert = Vec::new();

        for (overlap_interval, entry) in overlapping_intervals {
            let ent_end_page = overlap_interval.end();
            let ent_start_page = overlap_interval.start();

            //TODO: double check this
            let additional_offset = ((new_region_end_page - ent_start_page) << 12) as i64;

            // If the overlapping entry start lies before the new region start,
            // shrink the overlapping entry from its start upto new region end
            if ent_start_page < new_region_start_page && ent_end_page > new_region_start_page {
                let left_entry = VmmapEntry {
                    page_num: ent_start_page,
                    npages: new_region_start_page - ent_start_page,
                    prot: entry.prot,
                    maxprot: entry.maxprot,
                    flags: entry.flags,
                    backing: entry.backing,
                    offset: entry.offset - (new_region_start_page as i64), //TODO: check if this is right
                    file_size: entry.file_size,
                    removed: false,
                    cage_id: entry.cage_id,
                };
                to_insert.push((ie(ent_start_page, new_region_start_page), left_entry));
                to_remove.push(overlap_interval);
            }

            // If the new region end lies before the overlapping entry end,
            // shrink the overlapping entry from new region end upto overlapping entry end
            if ent_start_page < new_region_end_page && ent_end_page > new_region_end_page {
                let right_entry = VmmapEntry {
                    page_num: new_region_end_page,
                    npages: ent_end_page - new_region_end_page,
                    prot: entry.prot,
                    maxprot: entry.maxprot,
                    flags: entry.flags,
                    backing: entry.backing,
                    offset: entry.offset + additional_offset,
                    file_size: entry.file_size,
                    removed: false,
                    cage_id: entry.cage_id,
                };

                to_insert.push((ie(new_region_end_page, ent_end_page), right_entry));
                // need to check if previous condition didn't already mark the interval to be removed
                if !(ent_start_page < new_region_start_page && ent_end_page > new_region_start_page)
                {
                    to_remove.push(overlap_interval);
                }
            }

            if new_region_start_page <= ent_start_page && ent_end_page <= new_region_end_page {
                to_remove.push(overlap_interval);
            }
        }
        

        // Remove overlapping intervals
        for interval in to_remove {
            self.entries.remove_overlapping(interval);
        }

        // Insert split entries
        for (interval, value) in to_insert {
            self.entries.insert_strict(interval, value);
        }

        // Insert the new entry if not marked for removal
        if !remove {
            let new_entry = VmmapEntry {
                page_num,
                npages,
                prot,
                maxprot,
                flags,
                backing,
                offset,
                file_size,
                removed: false,
                cage_id,
            };
            self.entries
                .insert_strict(ie(new_region_start_page, new_region_end_page), new_entry);
        }
    }
}
