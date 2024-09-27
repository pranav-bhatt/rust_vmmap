use crate::vmmap_entries;

/// Used to identify whether the vmmap entry is backed anonymously,
/// by an fd, or by a shared memory segment

#[derive(Clone, Copy, PartialEq, Eq)]

pub enum MemoryBackingType {
    None, // just a dummy value for places where it needs to be passed, but you dont have the value
    Anonymous,
    SharedMemory(u64),   // stores shmid
    FileDescriptor(u64), // stores file descriptor addr
}

/// in the old native client based vmmap, we relied on the fd, shmid
/// fields. Here we remove those fields and replace with a 'backing' field
/// which is an enum containing info based on the type
#[derive(Clone, PartialEq, Eq)]
pub struct VmmapEntry {
    pub page_num: u32, /* base virtual addr >> NACL_PAGESHIFT */
    pub npages: u32,   /* number of pages */
    pub prot: i32,     /* mprotect attribute */
    pub maxprot: i32,
    pub flags: i32,       /* mapping flags */
    pub removed: bool,    /* flag set in fn Update(); */
    pub file_offset: i64, /* offset into desc */
    pub file_size: i64,   /* backing store size */
    pub cage_id: u64,
    pub backing: MemoryBackingType,
}

pub trait VmmapOps {
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
    );

    fn contain_cmp_entries();

    fn find_space();

    fn add_entry(&mut self, vmmap_entry_ref: VmmapEntry);

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
    );

    fn change_prot(&mut self, page_num: u32, npages: u32, new_prot: i32);

    // fn add_entry_with_override_and_shmid();

    fn remove_entry(&mut self, page_num: u32, npages: u32);

    fn check_existing_mapping();

    fn check_addr_mapping();

    fn find_page();

    fn find_page_iter();

    fn iter_at_end();

    fn iter_start();

    fn iter_incr();

    fn visit();

    fn find_map_space();

    fn find_map_space_above_hint();

    fn debug();
}
