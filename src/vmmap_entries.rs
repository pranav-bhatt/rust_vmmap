#[allow(dead_code)]
use crate::constants::{
    // MAP_PRIVATE, O_ACCMODE, O_RDONLY, O_RDWR, O_WRONLY, PAGESIZE,
    PROT_NONE,
    // PROT_READ, PROT_WRITE,
};
use crate::types::{MemoryBackingType, VmmapEntry};

#[allow(dead_code)]
impl VmmapEntry {
    pub fn new(
        page_num: u32,
        npages: u32,
        prot: i32,
        maxprot: i32,
        flags: i32,
        removed: bool,
        file_offset: i64,
        file_size: i64,
        cage_id: u64,
        backing: MemoryBackingType,
    ) -> Self {
        return VmmapEntry {
            page_num,
            npages,
            prot,
            maxprot,
            flags,
            removed,
            file_offset,
            file_size,
            cage_id,
            backing,
        };
    }

    fn max_prot(&self) -> i32 {
        let flags = PROT_NONE;

        // if entry->desc != NULL && 0 == (entry->flags & MAP_PRIVATE) {
        //     int o_flags = (*NACL_VTBL(NaClDesc, entry->desc)->GetFlags)(entry->desc);
        //     switch (o_flags & O_ACCMODE) {
        //     case O_RDONLY:
        //         flags = PROT_READ;
        //         break;
        //     case O_WRONLY:
        //         flags = PROT_WRITE;
        //         break;
        //     case O_RDWR:
        //         flags = PROT_READ | PROT_WRITE;
        //         break;
        //     default:
        //         break;
        //     }
        // } else {
        //     flags = PROT_READ | PROT_WRITE;
        // }

        flags
    }

    fn print(&self) {}

    // gonna have 3 types of memory:
    // memory that has no memory backing
    // things that are backed by fd -> represented by -1

    // Leave todo
    fn check_fd_protection(&self, cage_id: i32) {
        let _ = cage_id;
    } // will call the microvisor, need to pass fd
      // number if only its files backed and returns flags of fd
}

#[cfg(test)]
pub mod test_vmmap_entry_util {
    use crate::types::VmmapEntry;

    pub fn create_invalid_vmmap_entry() -> VmmapEntry {
        VmmapEntry::new(
            0,
            0, // setting npages = 0 makes it an invalid entry
            0,
            0,
            0,
            false,
            0,
            0,
            1,
            crate::types::MemoryBackingType::Anonymous,
        )
    }

    pub fn create_default_vmmap_entry() -> VmmapEntry {
        VmmapEntry::new(
            0,
            10, // setting npages = 0 makes it an invalid entry
            0,
            0,
            0,
            false,
            0,
            0,
            1,
            crate::types::MemoryBackingType::Anonymous,
        )
    }
}
