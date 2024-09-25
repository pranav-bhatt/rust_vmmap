use crate::constants::{
    MAP_PRIVATE, O_ACCMODE, O_RDONLY, O_RDWR, O_WRONLY, PAGESIZE, PROT_NONE, PROT_READ, PROT_WRITE,
};
use crate::types::{MemoryBackingType, VmmapEntryOps};

/// in the old native client based vmmap, we relied on the fd, shmid
/// fields. Here we remove those fields and replace with a 'backing' field
/// which is an enum containing info based on the type
pub struct VmmapEntry {
    pub page_num: u32, /* base virtual addr >> NACL_PAGESHIFT */
    pub npages: u32,   /* number of pages */
    pub prot: i32,     /* mprotect attribute */
    pub maxprot: i32,
    pub flags: i32,     /* mapping flags */
    pub removed: bool,  /* flag set in fn Update(); */
    pub offset: i64,    /* offset into desc */
    pub file_size: i64, /* backing store size */
    pub cage_id: u64,
    pub backing: MemoryBackingType,
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
        backing: MemoryBackingType,
    ) -> Self {
        return VmmapEntry {
            page_num,
            npages,
            prot,
            maxprot,
            flags,
            removed,
            offset,
            file_size,
            cage_id,
            backing,
        };
    }
}

impl VmmapEntryOps for VmmapEntry {
    fn get_key(&self) -> u32 {
        self.page_num // Key is the page number
    }
    fn set_key(&mut self, key: u32) {
        self.page_num = key;
    }

    fn get_size(&self) -> u32 {
        self.npages as u32 * PAGESIZE // Convert pages to bytes
    }
    fn set_size(&mut self, size: u32) {
        self.npages = size;
    }

    fn get_protection(&self) -> i32 {
        self.prot
    }
    fn set_protection(&mut self, prot: i32) {
        self.prot = prot;
    }

    fn get_max_protection(&self) -> i32 {
        self.maxprot
    }
    fn set_max_protection(&mut self, maxprot: i32) {
        self.maxprot = maxprot;
    }

    fn get_flags(&self) -> i32 {
        self.flags
    }
    fn set_flags(&mut self, flags: i32) {
        self.flags = flags;
    }

    fn is_removed(&self) -> bool {
        self.removed
    }
    fn set_removed(&mut self, removed: bool) {
        self.removed = removed;
    }

    fn get_offset(&self) -> i64 {
        self.offset
    }
    fn set_offset(&mut self, offset: i64) {
        self.offset = offset;
    }

    fn get_file_size(&self) -> i64 {
        self.file_size
    }
    fn set_file_size(&mut self, offset: i64) {
        self.file_size = offset;
    }

    fn get_backing_info(&self) -> &MemoryBackingType {
        &self.backing
    }
    fn set_backing_info(&mut self, backing: MemoryBackingType) {
        self.backing = backing;
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

    fn check_fd_protection(&self, _cage_id: i32) {}
}
