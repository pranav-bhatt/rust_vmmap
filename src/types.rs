/// Used to identify whether the vmmap entry is backed anonymously, 
/// by an fd, or by a shared memory segment
pub enum MemoryBackingType{
    Anonymous,
    SharedMemory(u64), // stores shmid
    FileDescriptor(u64), // stores file descriptor addr
}

/// Kept generic incase for different types of addresss systems (page based, direct address, etc)
pub trait VmmapEntryOps {
    fn get_key(&self) -> u32;           // Key could be a page number or an address
    fn get_size(&self) -> u32;          // Size in bytes or pages
    fn get_protection(&self) -> i32;    // Return protection flags (e.g., read/write)
    fn get_max_protection(&self) -> i32; // Return max protection
    fn get_flags(&self) -> i32;         // implementation specific flags
    fn is_removed(&self) -> bool;       // Check if the entry is marked as removed
    fn get_offset(&self) -> i64;        // Offset into the backing store
    fn get_file_size(&self) -> i64;     // Size of the backing store (if applicable)
    fn get_backing_info(&self) -> &MemoryBackingType; // Get backing information (e.g., file descriptor or shared memory)

    fn max_prot(&self) -> i32; // determines the maximum protection for a memory map entry
    fn print(&self);

    // gonna have 3 types of memory:
    // memory that has no memory backing
    // things that are backed by fd -> represented by -1

    // Leave todo
    fn check_fd_protection(&self);  // will call the microvisor, need to pass fd 
                                    // number if only its files backed and returns flags of fd
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
        offset: i64,
        file_size: i64,
    );

    fn contain_cmp_entries();
    
    fn find_space();

    fn add_entry(
        &mut self,
        vmmap_entry_ref: Box<dyn VmmapEntryOps>,
    );

    fn add_entry_with_override(
        page_num: usize,
        npages: u32,
        prot: i32,
        maxprot: i32,
        flags: i32,
        shmid: i32,
        desc: u64, // check the file descriptors in rust posix if they are i32 or u64
        offset: u64,
        file_size: u64,
    );

    fn change_prot();

    fn add_entry_with_override_and_shmid();

    fn remove_entry();

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