# Hydrogen build — run inside WSL Ubuntu from the repo root (or `.\tools\wsl.ps1 make <target>`).
# Everything generated goes into build/ and target/. Nothing outside the repo is written.

SHELL := /bin/bash

BUILD        := build
KERNEL_ELF   := target/x86_64-unknown-none/release/hydrogen-kernel
ISO          := $(BUILD)/hydrogen.iso
# User programs (userspace/apps/src/bin/*.rs), shipped in the ISO as boot modules. Every program
# here must also be listed in boot/limine.conf.
APPS         := hello-a hello-b memory-check crash-test abuse-test
APPS_DIR     := target/userspace/x86_64-unknown-none/release
# Test variants: each is the normal kernel plus one Cargo feature, built in its own target dir.
VARIANTS     := panic-test fault-pagefault fault-stack fault-int3 fault-thread-stack fault-smap fault-smep

# --- Limine, pinned (official repo: codeberg.org/Limine/Limine) ---
LIMINE_TAG    := v11.2.1-binary
LIMINE_COMMIT := deba356b4a70ee183ec3214864f0ba2ffe510b8c
LIMINE_URL    := https://codeberg.org/Limine/Limine.git
LIMINE_DIR    := $(BUILD)/limine
LIMINE_SHA256 := 698013c27ce2102766d399451fc3520c12662e84ebecc504f09f77417a5103d1  $(LIMINE_DIR)/BOOTX64.EFI\n50cd55b04ce5373731624d61ffcd2ddce7b2fa7ca3921df984dc8910c01c2b2f  $(LIMINE_DIR)/limine-uefi-cd.bin

# --- QEMU (UEFI firmware from Ubuntu's ovmf package) ---
OVMF_CODE := /usr/share/OVMF/OVMF_CODE_4M.fd
OVMF_VARS := /usr/share/OVMF/OVMF_VARS_4M.fd
ACCEL     := $(shell [ -r /dev/kvm ] && [ -w /dev/kvm ] && echo "-accel kvm -cpu host" || echo "-accel tcg -cpu max")
QEMU_BASE  = qemu-system-x86_64 $(ACCEL) -M q35 -m 512M -no-reboot \
             -drive if=pflash,unit=0,format=raw,file=$(OVMF_CODE),readonly=on \
             -drive if=pflash,unit=1,format=raw,file=$(BUILD)/ovmf-vars.fd

.PHONY: all kernel apps iso run debug unit test test-threads test-preempt test-user test-elf test-isolation test-panic test-fault-pagefault test-fault-stack \
        test-fault-int3 test-fault-thread-stack test-fault-smap test-fault-smep test-all limine clean distclean FORCE

all: iso

# ---------- Limine ----------
limine: $(LIMINE_DIR)/.verified

$(LIMINE_DIR)/.verified:
	@mkdir -p $(BUILD)
	@[ -d $(LIMINE_DIR) ] || git clone -q -c advice.detachedHead=false --depth=1 --branch=$(LIMINE_TAG) $(LIMINE_URL) $(LIMINE_DIR)
	@[ "$$(git -C $(LIMINE_DIR) rev-parse HEAD)" = "$(LIMINE_COMMIT)" ] || { echo "Limine commit mismatch — refusing to use it"; exit 1; }
	@printf "$(LIMINE_SHA256)\n" | sha256sum --quiet -c - || { echo "Limine checksum mismatch — refusing to use it"; exit 1; }
	@touch $@
	@echo "[make] Limine $(LIMINE_TAG) verified"

# ---------- kernel ----------
kernel:
	cargo build --release -p hydrogen-kernel

# ---------- user programs (their own workspace, see userspace/Cargo.toml) ----------
apps:
	cd userspace && cargo build --release --target-dir ../target/userspace

# ---------- ISO (UEFI boot only) ----------
# $(1) kernel ELF, $(2) output ISO, $(3) staging dir
define make_iso
	@rm -rf $(3)
	@mkdir -p $(3)/boot/limine $(3)/boot/apps $(3)/EFI/BOOT
	@cp $(addprefix $(APPS_DIR)/,$(APPS)) $(3)/boot/apps/
	@cp $(1) $(3)/boot/hydrogen-kernel
	@cp boot/limine.conf $(LIMINE_DIR)/limine-uefi-cd.bin $(3)/boot/limine/
	@cp $(LIMINE_DIR)/BOOTX64.EFI $(3)/EFI/BOOT/
	@xorriso -as mkisofs -R -r -J \
		--efi-boot boot/limine/limine-uefi-cd.bin -efi-boot-part --efi-boot-image \
		--protective-msdos-label $(3) -o $(2) 2>$(BUILD)/xorriso.log || { cat $(BUILD)/xorriso.log; exit 1; }
	@echo "[make] built $(2)"
endef

iso: limine kernel apps
	$(call make_iso,$(KERNEL_ELF),$(ISO),$(BUILD)/iso_root)

# build/hydrogen-<variant>.iso, e.g. build/hydrogen-fault-stack.iso
$(BUILD)/hydrogen-%.iso: limine apps FORCE
	cargo build --release -p hydrogen-kernel --features $* --target-dir target/variants/$*
	$(call make_iso,target/variants/$*/x86_64-unknown-none/release/hydrogen-kernel,$@,$(BUILD)/iso_root_$*)

FORCE:
.PRECIOUS: $(BUILD)/hydrogen-%.iso

$(BUILD)/ovmf-vars.fd:
	@cp $(OVMF_VARS) $@

# ---------- run / debug (opens a QEMU window on the Windows desktop via WSLg) ----------
run: iso $(BUILD)/ovmf-vars.fd
	$(QEMU_BASE) -cdrom $(ISO) -serial stdio

debug: iso $(BUILD)/ovmf-vars.fd
	@echo "[make] QEMU paused, GDB server on :1234. In another Ubuntu shell:"
	@echo "       gdb $(KERNEL_ELF) -ex 'target remote :1234'"
	$(QEMU_BASE) -cdrom $(ISO) -serial stdio -s -S

# ---------- automated tests (headless, time-limited) ----------
unit:
	cargo test -p hydrogen-mm -p hydrogen-elf --target x86_64-unknown-linux-gnu

test: iso
	@bash tools/test-boot.sh $(ISO) $(BUILD)/test \
		"Hydrogen kernel online" "IDT loaded" "paging: own page tables active" "now on kernel stack" \
		"selftest pmm: ok" "selftest paging: ok" "selftest heap: ok" "timer ok" "selftest threads: ok" \
		"phase 3 boot complete"

# Phase 3.1: two kernel threads alternate A1 B1 A2 B2 A3 B3 (the kernel checks the exact order).
test-threads: iso
	@bash tools/test-boot.sh $(ISO) $(BUILD)/test-threads \
		"sched: boot path is now thread 0" "thread A (id 2): step 1" "thread B (id 3): step 1" \
		"thread A (id 2): step 3" "thread B (id 3): step 3" "selftest threads: ok (A1 B1 A2 B2 A3 B3;"

# Phase 3.2: the timer takes the CPU from endless loops, sleeps end on time, forced switches
# corrupt nothing, finished threads leak nothing, the idle thread runs when all others sleep.
test-preempt: iso
	@bash tools/test-boot.sh $(ISO) $(BUILD)/test-preempt \
		"sched: preemption on (10 ms time slices)" "locks mask interrupts" \
		"clock kept time while printing" "3 threads in endless loops got" \
		"results under forced switches, 0 wrong" "50 threads started and finished, no memory leaked" \
		"idle thread took" "phase 3 boot complete"

# Phase 3.3: programs in ring 3 with private memory, talking to the kernel through system calls.
test-user: iso
	@bash tools/test-boot.sh $(ISO) $(BUILD)/test-user 		"user mode: syscall door open; SMEP on, SMAP on" "] hello from ring 3" "exited with code 0" 		"hello ran in ring 3, exit code 0" "registers intact across system calls" 		"500 ms of nothing but system calls" "each saw only its own data" "50 programs started and finished, no memory leaked" "phase 3 boot complete"

# Phase 3.4: two separate program files are loaded and take turns; the loader gets memory right.
test-elf: iso
	@bash tools/test-boot.sh $(ISO) $(BUILD)/test-elf \
		"boot modules: 5 programs" "] A5" "] B5" "constants, variables and stack are fine" \
		"selftest elf: ok (hello-a and hello-b" "selftest elf: ok (memory-check" \
		"selftest elf: ok (damaged program files refused" "selftest elf: ok (no memory leaked" "phase 3 boot complete"
	@bash tools/check-interleave.sh $(BUILD)/test-elf/serial.log

# Phase 3.5: programs that misbehave are ended; the kernel and the other programs carry on.
test-isolation: iso
	@bash tools/test-boot.sh $(ISO) $(BUILD)/test-isolation \
		"KILLED: PAGE FAULT at 0xffffffff80000000" "KILLED: DIVIDE ERROR" "KILLED: INVALID OPCODE" \
		"KILLED: GENERAL PROTECTION FAULT" "43 kinds of crash, each ended only its own program" \
		"] [hydrogen] forged kernel line??[31mred" "bad system call arguments come back as errors" \
		"crashed programs leaked no memory" "phase 3 boot complete"

test-panic: $(BUILD)/hydrogen-panic-test.iso
	@bash tools/test-boot.sh $< $(BUILD)/test-panic "PANIC at kernel/src/main.rs"

test-fault-pagefault: $(BUILD)/hydrogen-fault-pagefault.iso
	@bash tools/test-boot.sh $< $(BUILD)/test-fault-pagefault "EXCEPTION: PAGE FAULT at 0xdeadb000"

test-fault-stack: $(BUILD)/hydrogen-fault-stack.iso
	@bash tools/test-boot.sh $< $(BUILD)/test-fault-stack "EXCEPTION: DOUBLE FAULT" "kernel stack overflow"

test-fault-int3: $(BUILD)/hydrogen-fault-int3.iso
	@bash tools/test-boot.sh $< $(BUILD)/test-fault-int3 "BREAKPOINT at rip=" "resumed after breakpoint" "phase 3 boot complete"

# Phase 3.1: a kernel thread overflowing its 16 KiB stack hits its own guard page.
test-fault-thread-stack: $(BUILD)/hydrogen-fault-thread-stack.iso
	@bash tools/test-boot.sh $< $(BUILD)/test-fault-thread-stack "EXCEPTION: DOUBLE FAULT" "thread stack overflow"

# Phase 3.5: the CPU stops the kernel itself from reading (SMAP) or running (SMEP) program memory.
test-fault-smap: $(BUILD)/hydrogen-fault-smap.iso
	@bash tools/test-boot.sh $< $(BUILD)/test-fault-smap "EXCEPTION: PAGE FAULT at 0x400000 (PageFaultErrorCode(PROTECTION_VIOLATION))"

test-fault-smep: $(BUILD)/hydrogen-fault-smep.iso
	@bash tools/test-boot.sh $< $(BUILD)/test-fault-smep "EXCEPTION: PAGE FAULT at 0x400000 (PageFaultErrorCode(PROTECTION_VIOLATION | INSTRUCTION_FETCH))"

test-all: unit test test-threads test-preempt test-user test-elf test-isolation test-panic test-fault-pagefault test-fault-stack \
          test-fault-int3 test-fault-thread-stack test-fault-smap test-fault-smep
	@echo "[make] ALL TESTS PASSED"

clean:
	rm -rf $(BUILD)/iso_root* $(BUILD)/*.iso $(BUILD)/test* $(BUILD)/ovmf-vars.fd $(BUILD)/xorriso.log
	cargo clean

distclean: clean
	rm -rf $(BUILD)
