#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

cargo build

rm -rf esp
mkdir -p esp/EFI/BOOT
cp target/x86_64-unknown-uefi/debug/ruefi.efi esp/EFI/BOOT/BOOTX64.EFI

cp /usr/share/OVMF/OVMF_VARS_4M.fd OVMF_VARS.fd
chmod u+w OVMF_VARS.fd

qemu-system-x86_64 \
  -machine q35 -m 256M \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -drive if=pflash,format=raw,file=OVMF_VARS.fd \
  -drive format=raw,file=fat:rw:esp \
  -vga std
