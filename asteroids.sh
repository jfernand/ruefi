#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release -p asteroids-uefi

rm -rf esp-asteroids
mkdir -p esp-asteroids/EFI/BOOT
cp target/x86_64-unknown-uefi/release/asteroids.efi esp-asteroids/EFI/BOOT/BOOTX64.EFI

cp /usr/share/OVMF/OVMF_VARS_4M.fd OVMF_VARS_asteroids.fd
chmod u+w OVMF_VARS_asteroids.fd

# Graphical window (not -nographic): the game draws to the GOP framebuffer,
# which needs an actual display device to show up. Arrow keys to
# turn/thrust, space to fire, Enter to restart after game over, Esc to quit.
qemu-system-x86_64 \
  -machine q35 -m 256M \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -drive if=pflash,format=raw,file=OVMF_VARS_asteroids.fd \
  -drive format=raw,file=fat:rw:esp-asteroids
