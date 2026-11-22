# SIGIL build — run inside WSL Ubuntu from the repo root (or `.\tools\wsl.ps1 make <target>`).
# Everything generated goes into build/ and target/. Nothing outside the repo is written.

SHELL := /bin/bash

BUILD        := build
KERNEL_ELF   := target/x86_64-unknown-none/release/sigil-kernel
PANIC_ELF    := target/panic/x86_64-unknown-none/release/sigil-kernel
ISO          := $(BUILD)/sigil.iso
PANIC_ISO    := $(BUILD)/sigil-panic.iso

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

.PHONY: all kernel kernel-panic iso iso-panic run debug test test-panic test-all limine clean distclean

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
	cargo build --release -p sigil-kernel

kernel-panic:
	cargo build --release -p sigil-kernel --features panic-test --target-dir target/panic

# ---------- ISO (UEFI boot only) ----------
# $(1) kernel ELF, $(2) output ISO, $(3) staging dir
define make_iso
	@rm -rf $(3)
	@mkdir -p $(3)/boot/limine $(3)/EFI/BOOT
	@cp $(1) $(3)/boot/sigil-kernel
	@cp boot/limine.conf $(LIMINE_DIR)/limine-uefi-cd.bin $(3)/boot/limine/
	@cp $(LIMINE_DIR)/BOOTX64.EFI $(3)/EFI/BOOT/
	@xorriso -as mkisofs -R -r -J \
		--efi-boot boot/limine/limine-uefi-cd.bin -efi-boot-part --efi-boot-image \
		--protective-msdos-label $(3) -o $(2) 2>$(BUILD)/xorriso.log || { cat $(BUILD)/xorriso.log; exit 1; }
	@echo "[make] built $(2)"
endef

iso: limine kernel
	$(call make_iso,$(KERNEL_ELF),$(ISO),$(BUILD)/iso_root)

iso-panic: limine kernel-panic
	$(call make_iso,$(PANIC_ELF),$(PANIC_ISO),$(BUILD)/iso_root_panic)

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
test: iso
	@bash tools/test-boot.sh $(ISO) $(BUILD)/test "phase 1 boot complete"

test-panic: iso-panic
	@bash tools/test-boot.sh $(PANIC_ISO) $(BUILD)/test-panic "PANIC at kernel/src/main.rs"

test-all: test test-panic

clean:
	rm -rf $(BUILD)/iso_root* $(BUILD)/*.iso $(BUILD)/test* $(BUILD)/ovmf-vars.fd $(BUILD)/xorriso.log
	cargo clean

distclean: clean
	rm -rf $(BUILD)
