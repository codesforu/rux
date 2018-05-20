/// [Multiboot](https://www.gnu.org/software/grub/manual/multiboot/multiboot.html)
/// information parser.
mod multiboot;

/// Paging initialization code.
mod paging;

/// Interrupt initialization code.
mod interrupt;

/// Segmentation initialization code.
mod segmentation;

pub use self::paging::{KERNEL_PML4, KERNEL_PDPT, KERNEL_PD,
                       OBJECT_POOL_PT, OBJECT_POOL_START_VADDR,
                       LOCAL_APIC_PAGE_VADDR, IO_APIC_PAGE_VADDR};
pub use self::segmentation::set_kernel_stack;

use ::kmain;
use super::{kernel_end_paddr, kernel_start_paddr, kernel_start_vaddr};

use core::mem;
use core::slice::{self, Iter};

use common::{PAddr, MemoryRegion};

extern {
    /// Multiboot signature exposed by linker.
    #[allow(dead_code)]
    static multiboot_sig: u32;
    /// Multiboot pointer exposed by linker.
    static multiboot_ptr: u64;
}

// Helper functions
pub fn multiboot_paddr() -> PAddr {
    unsafe { PAddr::from(multiboot_ptr) }
}

/// Iterator for `Option<MemoryRegion>`. It returns `None` if the
/// inner `Option` is none. Otherwise return the value unwrapped.
pub struct FreeRegionsIterator<'a>(Iter<'a, Option<MemoryRegion>>);

impl<'a> Iterator for FreeRegionsIterator<'a> {
    type Item = MemoryRegion;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.0.next();

        if item.is_none() {
            None
        } else {
            if item.unwrap().is_none() {
                None
            } else {
                Some(item.unwrap().unwrap())
            }
        }
    }
}

/// Initialization information to be passed to `kmain`. It contains
/// free regions and rinit and kernel memory region information. At
/// most 16 free regions are supported.
#[derive(Debug)]
pub struct InitInfo {
    free_regions_size: usize,
    free_regions: [Option<MemoryRegion>; 16],
    rinit_region: MemoryRegion,
    kernel_region: MemoryRegion,
}

impl InitInfo {
    /// Return a `FreeRegionsIterator` that allows iterating over all
    /// free regions.
    pub fn free_regions(&self) -> FreeRegionsIterator {
        FreeRegionsIterator(self.free_regions.iter())
    }

    /// The kernel memory region.
    pub fn kernel_region(&self) -> MemoryRegion {
        self.kernel_region
    }

    /// The user-space rinit program memory region.
    pub fn rinit_region(&self) -> MemoryRegion {
        self.rinit_region
    }

    /// Create a new `InitInfo` using a kernel region and a rinit region.
    pub fn new(kernel_region: MemoryRegion, rinit_region: MemoryRegion) -> InitInfo {
        InitInfo { free_regions_size: 0,
                   free_regions: [None; 16],
                   kernel_region: kernel_region,
                   rinit_region: rinit_region }
    }

    /// Append a new free region to the `InitInfo`.
    pub fn push_free_region(&mut self, region: MemoryRegion) {
        self.free_regions[self.free_regions_size] = Some(region);
        self.free_regions_size += 1;
    }
}

/// Read the multiboot structure. Construct an `InitInfo` with all
/// free regions. A memory region that will be used for initial memory
/// allocation is returned seperately. That region is always the same
/// as the region of the kernel region.
fn bootstrap_archinfo() -> (InitInfo, MemoryRegion) {
    let bootinfo = unsafe {
        multiboot::Multiboot::new(multiboot_paddr(), |addr, size| {
            let ptr = mem::transmute(super::kernel_paddr_to_vaddr(addr).into(): usize);
            Some(slice::from_raw_parts(ptr, size))
        })
    }.unwrap();

    let rinit_module = bootinfo.modules().unwrap().next().unwrap();
    log!("rinit module: {:?}", rinit_module);
    
    let mut archinfo = InitInfo::new(
        MemoryRegion::new(kernel_start_paddr(),
                          kernel_end_paddr().into(): usize + 1 -
                          kernel_start_paddr().into(): usize),
        MemoryRegion::new(rinit_module.start,
                          rinit_module.end.into(): usize + 1 -
                          rinit_module.start.into(): usize));
    let mut alloc_region: Option<MemoryRegion> = None;
    
    for area in bootinfo.memory_regions().unwrap() {
        use self::multiboot::{MemoryType};
        
        if !(area.memory_type() == MemoryType::RAM) {
            continue;
        }

        let mut cur_region = MemoryRegion::new(area.base_address(), area.length() as usize);

        if cur_region.skip_up(&archinfo.kernel_region()) {
            assert!(cur_region.skip_up(&archinfo.rinit_region()));
            alloc_region = Some(cur_region);
        } else {
            archinfo.push_free_region(cur_region);
        }
    }

    (archinfo, alloc_region.unwrap())
}

/// Kernel entrypoint. This function calls `bootstrap_archinfo`, and
/// then use the information to initialize paging, segmentation,
/// interrupt, and APIC. It then jumps to `kmain`.
#[lang="start"]
#[no_mangle]
#[allow(private_no_mangle_fns)]
pub fn kinit() {
    let (mut archinfo, mut alloc_region) = bootstrap_archinfo();

    log!("kernel_start_vaddr: 0x{:x}", kernel_start_vaddr());
    log!("archinfo: {:?}", archinfo);
    log!("alloc_region: {:?}", alloc_region);

    paging::init(&mut alloc_region);
    segmentation::init();
    interrupt::init();

    archinfo.push_free_region(alloc_region);

    {
        let local_apic = ::arch::interrupt::LOCAL_APIC.lock();
        let io_apic = ::arch::interrupt::IO_APIC.lock();
        log!("Local APIC id: 0x{:x}", local_apic.id());
        log!("Local APIC version: 0x{:x}", local_apic.version());
        log!("I/O APIC id: 0x{:x}", io_apic.id());
        log!("I/O APIC version: 0x{:x}", io_apic.version());
    }

    kmain(archinfo);
}
