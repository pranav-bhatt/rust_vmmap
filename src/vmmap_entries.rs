use crate::types::{VmmapEntryOps, MemoryBackingType};
use crate::constants::{
    O_RDONLY,
    O_WRONLY,
    O_ACCMODE,
    O_RDWR,
    PROT_READ,
    PROT_WRITE,
    PROT_NONE,
    MAP_PRIVATE,
    PAGESIZE,
};

/// in the old native client based vmmap, we relied on the fd, shmid
/// fields. Here we remove those fields and replace with a 'backing' field
/// which is an enum containing info based on the type
pub struct VmmapEntry {
    page_num: u32, /* base virtual addr >> NACL_PAGESHIFT */
    npages: u32,      /* number of pages */
    prot: i32,           /* mprotect attribute */
    maxprot: i32,
    flags: i32,         /* mapping flags */
    removed: bool,       /* flag set in fn Update(); */
    offset: i64,    /* offset into desc */
    file_size: i64, /* backing store size */
    cage_id: u64,
    backing: MemoryBackingType,
}

impl VmmapEntry {
    pub fn new(
        page_num: u32,
        npages: u32,
        prot: i32,
        maxprot: i32,
        flags: i32,
        removed: bool,
        offset: i64,
        file_size: i64,
        cage_id: u64,
        backing: MemoryBackingType
    ) -> Self {
        return VmmapEntry{
            page_num,
            npages,
            prot,
            maxprot,
            flags,
            removed,
            offset,
            file_size,
            cage_id,
            backing
        }
    }
}

impl VmmapEntryOps for VmmapEntry{
    fn get_key(&self) -> u32 {
        self.page_num // Key is the page number
    }

    fn get_size(&self) -> u32 {
        self.npages as u32 * PAGESIZE // Convert pages to bytes
    }

    fn get_protection(&self) -> i32 {
        self.prot
    }

    fn get_max_protection(&self) -> i32 {
        self.maxprot
    }

    fn get_flags(&self) -> i32 {
        self.flags
    }

    fn is_removed(&self) -> bool {
        self.removed
    }

    fn get_offset(&self) -> i64 {
        self.offset
    }

    fn get_file_size(&self) -> i64 {
        self.file_size
    }

    fn get_backing_info(&self) -> &MemoryBackingType {
        &self.backing
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

    fn print(&self){}

    fn check_fd_protection(&self, _cage_id: i32){}

}