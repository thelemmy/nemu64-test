.section .boot, "ax"
.global _start
.set noreorder

// Floating Point GPRs
.set FPC_CSR,   $31

// Floating Point status register
.set FPCSR_FS,  0x01000000 // Flush denormalized to zero
.set FPCSR_EV,  0x00000800 // Enable invalid operation exceptions

// N64 PIF/OS pointers
.set OS_MEM_SIZE,           0x80000318
.set PIF_ENTRY_POINT,       0xBFC00000
.set PIF_CONTROL,           0x07FC

// Runtime environment pointers
.set FS_START,              0x8000031C

_start:
    // IPL3 copied over only the first MB which might not be enough. Copy the remainder, if there is any
    li $t0, 0x100000
    dla $t2, __binary_size
    subu $t2, $t2, $t0
    bltz $t2, loading_done
    nop
    dla $t3, __boot_start
    li $t1, 0x10001000

    // Remove the first MB (already removed from size above already)
    addu $t3, $t3, $t0
    addu $t1, $t1, $t0

    // DMA is initiated once length is written to (the number of bytes is length+1)
    addiu $t2, $t2, -1
    lui $t0, 0xA460
    sw $t1, 0x4($t0)  // cart
    sw $t3, 0x0($t0)  // dram
    sw $t2, 0xC($t0)  // length

wait_for_dma_finished:
    lw $t1, 0x10($t0)
    andi $t1, $t1, 1
    bnez $t1, wait_for_dma_finished
    nop

loading_done:
    // Initialize stack
    li $t0, OS_MEM_SIZE
    lw $t0, 0($t0)
    li $t1, 0x80000000
    or $sp, $t0, $t1

    // Clear .bss section
    dla $t0, __bss_start
    dla $t1, __bss_end
bss_clear_loop:
    bge $t0, $t1, bss_clear_done
    nop
    sw $zero, 0($t0)
    addiu $t0, $t0, 4
    b bss_clear_loop
    nop
bss_clear_done:

    // Configure Floating Point Unit
    li $t0, (FPCSR_FS | FPCSR_EV)
    ctc1 $t0, FPC_CSR

    // Enable PIF NMI
    li $t0, PIF_ENTRY_POINT
    ori $t1, $zero, 8
    sw $t1, PIF_CONTROL($t0)

    // Store the FS location for the OS
    dla $t0, __rom_end
    li $t1, FS_START
    sw $t0, 0($t1)

    // Clear $k0 and $k1. These have to be 0 as these can be used for regular code to configure the exception handler
    lui $k0, 0
    lui $k1, 0

    // Jump to Rust
    jal rust_entrypoint
    nop
