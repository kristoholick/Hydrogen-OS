/* Layout of a Hydrogen program: each kind of memory on its own pages, never writable + executable. */
OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text   PT_LOAD FLAGS(5);   /* read + exec  */
    rodata PT_LOAD FLAGS(4);   /* read         */
    data   PT_LOAD FLAGS(6);   /* read + write */
}

SECTIONS
{
    . = 0x400000;
    .text   : { *(.text._start) *(.text .text.*) } :text
    . = ALIGN(4096);
    .rodata : { *(.rodata .rodata.*) } :rodata
    . = ALIGN(4096);
    .data   : { *(.data .data.*) } :data
    .bss    : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) *(.comment) }
}
