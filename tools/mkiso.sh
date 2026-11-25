#!/bin/sh
set -e
cd "$(dirname "$0")/.."

EFI=target/x86_64-unknown-uefi/release/hywos.efi
if [ ! -f "$EFI" ]; then
  echo "missing $EFI - run: cargo build --release --target x86_64-unknown-uefi" >&2
  exit 1
fi

rm -rf build

mkdir -p build/esp/EFI/BOOT
cp "$EFI" build/esp/EFI/BOOT/BOOTX64.EFI
mkdir -p build/iso/HYWOS/SYSTEM
mkdir -p build/iso/EFI/BOOT
cp "$EFI" build/iso/EFI/BOOT/BOOTX64.EFI
cp tools/cdmark.txt build/iso/HYWOS/CD
cp tools/cdmark.txt build/iso/CDMARK.TXT
cp tools/welcome.txt build/iso/HYWOS/SYSTEM/WELCOME.TXT
cp tools/welcome.txt build/iso/WELCOME.TXT

echo "creating efiboot.img"
dd if=/dev/zero of=build/efiboot.img bs=1M count=24 status=none
mkfs.fat -F 16 build/efiboot.img >/dev/null
mmd -i build/efiboot.img ::/EFI
mmd -i build/efiboot.img ::/EFI/BOOT
mcopy -i build/efiboot.img build/esp/EFI/BOOT/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
mmd -i build/efiboot.img ::/HYWOS
mmd -i build/efiboot.img ::/HYWOS/SYSTEM
mcopy -i build/efiboot.img tools/cdmark.txt ::/HYWOS/CD
mcopy -i build/efiboot.img tools/welcome.txt ::/HYWOS/SYSTEM/WELCOME.TXT

echo "creating usb.img (user volume, MBR + FAT32)"
dd if=/dev/zero of=build/usb.img bs=1M count=64 status=none
printf 'label: dos\nstart=2048, size=129024, type=0c, bootable\n' | sfdisk -q build/usb.img
mkfs.fat -F 32 --offset 2048 build/usb.img 64512 >/dev/null
python3 tools/make_updpkg.py build/hywos-1.1.0.updpkg 1.1.0 "$EFI"
IMG="build/usb.img@@1M"
mmd -i "$IMG" ::/Documents
mmd -i "$IMG" ::/Pictures
mmd -i "$IMG" ::/updates
mcopy -i "$IMG" build/hywos-1.1.0.updpkg ::/hywos-1.1.0.updpkg
mcopy -i "$IMG" tools/welcome.txt ::/Documents/welcome.txt
mcopy -i "$IMG" tools/welcome.txt ::/updates/readme.txt
mcopy -i "$IMG" tools/latest.json ::/updates/latest.json

echo "creating kernel test disk"
dd if=/dev/zero of=build/kdisk.img bs=1M count=64 status=none
dd if=/dev/zero of=build/install.img bs=1M count=256 status=none

echo "creating ISO"
cp build/efiboot.img build/iso/efiboot.img
xorriso -as mkisofs -R -J -V HYWOS \
  -o build/hywos.iso \
  -e efiboot.img -no-emul-boot -isohybrid-gpt-basdat \
  build/iso >/dev/null

ls -lh build/hywos.iso build/efiboot.img build/usb.img build/kdisk.img build/hywos-1.1.0.updpkg