use std::io;

use nodit::Interval;

/// Used to identify whether the vmmap entry is backed anonymously,
/// by an fd, or by a shared memory segment

#[allow(dead_code)]
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

#[allow(dead_code)]
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
    ) -> Result<(), io::Error>;

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
    ) -> Result<(), io::Error>;

    fn change_prot(&mut self, page_num: u32, npages: u32, new_prot: i32);

    fn remove_entry(&mut self, page_num: u32, npages: u32) -> Result<(), io::Error>;

    fn check_existing_mapping(&self, page_num: u32, npages: u32, prot: i32) -> bool;

    fn check_addr_mapping(&mut self, page_num: u32, npages: u32, prot: i32) -> Option<u32>;

    fn find_page(&self, page_num: u32) -> Option<&VmmapEntry>;

    fn find_page_mut(&mut self, page_num: u32) -> Option<&mut VmmapEntry>;

    fn find_page_iter(
        &self,
        page_num: u32,
    ) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &VmmapEntry)>;

    fn find_page_iter_mut(
        &mut self,
        page_num: u32,
    ) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &mut VmmapEntry)>;

    fn first_entry(&self) -> Option<(&Interval<u32>, &VmmapEntry)>;

    fn last_entry(&self) -> Option<(&Interval<u32>, &VmmapEntry)>;

    fn double_ended_iter(&self) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &VmmapEntry)>;

    fn double_ended_iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (&Interval<u32>, &mut VmmapEntry)>;

    fn find_space(&self, npages: u32) -> Option<Interval<u32>>;

    fn find_space_above_hint(&self, npages: u32, hint: u32) -> Option<Interval<u32>>;

    fn find_map_space(&self, num_pages: u32, pages_per_map: u32) -> Option<Interval<u32>>;

    fn find_map_space_with_hint(
        &self,
        num_pages: u32,
        pages_per_map: u32,
        hint: u32,
    ) -> Option<Interval<u32>>;
}
