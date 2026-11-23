/* Hydrogen kernel layout: higher-half, loaded by Limine at the top 2 GiB of virtual memory.
 * The __*_start/__*_end symbols let the kernel map each section with its own permissions. */
OUTPUT_FORMAT(elf64-x86-64)
ENTRY(kmain)

PHDRS
{
    limine_requests PT_LOAD FLAGS(4);   /* read        */
    text            PT_LOAD FLAGS(5);   /* read + exec */
    rodata          PT_LOAD FLAGS(4);   /* read        */
    data            PT_LOAD FLAGS(6);   /* read + write */
}

SECTIONS
{
    . = 0xffffffff80000000;
    __kernel_start = .;

    /* Limine scans this section for the kernel's boot requests. */
    .limine_requests : {
        KEEP(*(.limine_requests_start))
        KEEP(*(.limine_requests))
        KEEP(*(.limine_requests_end))
    } :limine_requests

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __text_start = .;
    .text : { *(.text .text.*) } :text
    __text_end = .;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __rodata_start = .;
    .rodata : { *(.rodata .rodata.*) } :rodata
    __rodata_end = .;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __data_start = .;
    .data : { *(.data .data.*) } :data
    .bss  : { *(.bss .bss.*) *(COMMON) } :data
    __kernel_end = .;

    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) }
}
