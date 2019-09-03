use core::sync::atomic::*;
use log::*;
use rboot::BootInfo;

pub mod acpi;
pub mod board;
pub mod consts;
pub mod cpu;
pub mod driver;
pub mod gdt;
pub mod idt;
pub mod interrupt;
pub mod io;
pub mod ipi;
pub mod memory;
pub mod paging;
pub mod rand;
pub mod syscall;
pub mod timer;

static AP_CAN_INIT: AtomicBool = AtomicBool::new(false);

/// The entry point of kernel
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    let cpu_id = cpu::id();
    println!("Hello world! from CPU {}!", cpu_id);

    if cpu_id != 0 {
        while !AP_CAN_INIT.load(Ordering::Relaxed) {
            spin_loop_hint();
        }
        other_start();
    }

    // First init log mod, so that we can print log info.
    crate::logging::init();

    // check BootInfo from bootloader
    let boot_info = unsafe { &*boot_info };
    info!("{:#x?}", boot_info);
    assert_eq!(
        boot_info.physical_memory_offset as usize,
        consts::PHYSICAL_MEMORY_OFFSET
    );

    // setup fast syscall in x86_64
    interrupt::fast_syscall::init();

    // Init physical memory management and heap.
    memory::init(boot_info);

    // Now heap is available

    // Init GDT
    gdt::init();
    // Init trap handling
    // WARN: IDT must be initialized after GDT.
    //       Because x86_64::IDT will use current CS segment in IDT entry.
    idt::init();
    // Init virtual space
    memory::init_kernel_kseg2_map();
    // get local apic id of cpu
    cpu::init();
    // now we can start LKM.
    crate::lkm::manager::ModuleManager::init();
    // Use IOAPIC instead of PIC, use APIC Timer instead of PIT, init serial&keyboard in x86_64
    driver::init(boot_info);
    // init pci/bus-based devices ,e.g. Intel 10Gb NIC, ...
    crate::drivers::init();
    // init cpu scheduler and process manager, and add user shell app in process manager
    crate::process::init();
    // load acpi
    acpi::init();

    // wake up other CPUs
    AP_CAN_INIT.store(true, Ordering::Relaxed);

    // call the first main function in kernel.
    crate::kmain();
}

/// The entry point for other processors
fn other_start() -> ! {
    // init gdt
    gdt::init();
    // init trap handling
    idt::init();
    // init local apic
    cpu::init();
    // setup fast syscall in x86_64
    interrupt::fast_syscall::init();
    // call the first main function in kernel.
    crate::kmain();
}
