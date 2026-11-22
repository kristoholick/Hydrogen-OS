/* SIGIL kernel layout: higher-half, loaded by Limine at the top 2 GiB of virtual memory. */
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

    /* Limine scans this section for the kernel's boot requests. */
    .limine_requests : {
        KEEP(*(.limine_requests_start))
        KEEP(*(.limine_requests))
        KEEP(*(.limine_requests_end))
    } :limine_requests

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .text : { *(.text .text.*) } :text

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .rodata : { *(.rodata .rodata.*) } :rodata

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .data : { *(.data .data.*) } :data
    .bss  : { *(.bss .bss.*) *(COMMON) } :data

    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) }
}
