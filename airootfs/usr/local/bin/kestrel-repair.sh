#!/usr/bin/env bash
# ==============================================================================
# Kestrel Arch Intelligent System & Bootloader Repair Engine
# Non-interactive CLI driven by the Slint GUI (main.rs)
# ==============================================================================
set -euo pipefail

TARGET_ROOT="${1:-}"
TARGET_OS_ID="${2:-}"
CHOSEN_KERNEL="${3:-}"
CHOSEN_BL="${4:-}"
EFI_PART="${5:-}"

if [[ -z "$TARGET_ROOT" || -z "$CHOSEN_KERNEL" || -z "$CHOSEN_BL" ]]; then
    echo "[-] Error: Missing arguments." >&2
    echo "Usage: $0 <root_part> <os_id> <kernel> <bootloader> [efi_part]" >&2
    exit 1
fi

TARGET_MNT="/mnt/kestrel_repair"
EFI_MNT="${TARGET_MNT}/boot"

cleanup() {
    umount -R "${TARGET_MNT}" 2>/dev/null || true
    rm -rf "${TARGET_MNT}" 2>/dev/null || true
}
trap cleanup EXIT

echo "==> [1/5] Checking hardware firmware and bootloader compatibility..."
IS_UEFI=false
if [[ -d "/sys/firmware/efi" ]]; then
    IS_UEFI=true
    echo "[+] System Firmware: UEFI"
else
    echo "[!] System Firmware: Legacy BIOS (CSM)"
    if [[ "$CHOSEN_BL" == "systemd-boot" || "$CHOSEN_BL" == "rEFInd" ]]; then
        echo "[-] Fatal: Bootloader '$CHOSEN_BL' requires UEFI firmware. Cannot install on BIOS." >&2
        exit 1
    fi
fi

echo "==> [2/5] Mounting target root partition ($TARGET_ROOT)..."
mkdir -p "${TARGET_MNT}"
mount "$TARGET_ROOT" "${TARGET_MNT}"

if [[ "$IS_UEFI" == true && -n "$EFI_PART" && "$EFI_PART" != "Select EFI Partition..." && "$EFI_PART" != "None / Legacy MBR" ]]; then
    if [[ -d "${TARGET_MNT}/boot/efi" ]]; then
        EFI_MNT="${TARGET_MNT}/boot/efi"
    fi
    mkdir -p "$EFI_MNT"
    mount "$EFI_PART" "$EFI_MNT"
    echo "[+] Mounted ESP ($EFI_PART) -> $EFI_MNT"
fi

echo "==> [3/5] Resolving kernel package for target system..."
if [[ "$TARGET_OS_ID" == "cachyos" ]]; then
    CHOSEN_KERNEL="linux-cachyos"
    echo "[+] Target is CachyOS. Enforcing $CHOSEN_KERNEL."
fi

echo "==> Deploying ${CHOSEN_KERNEL} and headers via pacstrap..."
pacstrap -K "${TARGET_MNT}" "${CHOSEN_KERNEL}" "${CHOSEN_KERNEL}-headers" mkinitcpio

echo "==> [4/5] Regenerating initramfs images (mkinitcpio)..."
arch-chroot "${TARGET_MNT}" mkinitcpio -P

echo "==> [5/5] Deploying and configuring bootloader (${CHOSEN_BL})..."
case "$CHOSEN_BL" in
    "systemd-boot")
        arch-chroot "${TARGET_MNT}" bootctl install || arch-chroot "${TARGET_MNT}" bootctl update
        cat <<EOF > "${EFI_MNT}/loader/loader.conf"
default arch.conf
timeout 4
console-mode max
editor no
EOF
        ROOT_UUID=$(blkid -s UUID -o value "$TARGET_ROOT")
        cat <<EOF > "${EFI_MNT}/loader/entries/arch.conf"
title   Arch Linux (${CHOSEN_KERNEL})
linux   /vmlinuz-${CHOSEN_KERNEL}
initrd  /initramfs-${CHOSEN_KERNEL}.img
options root=UUID=${ROOT_UUID} rw quiet splash
EOF
        ;;

    "GRUB")
        pacstrap -K "${TARGET_MNT}" grub
        if [[ "$IS_UEFI" == true ]]; then
            pacstrap -K "${TARGET_MNT}" efibootmgr
            arch-chroot "${TARGET_MNT}" grub-install --target=x86_64-efi --efi-directory="${EFI_MNT#${TARGET_MNT}}" --bootloader-id=GRUB --recheck
        else
            DISK=$(lsblk -no PKNAME "$TARGET_ROOT" | head -n1)
            arch-chroot "${TARGET_MNT}" grub-install --target=i386-pc "/dev/${DISK}" --recheck
        fi
        arch-chroot "${TARGET_MNT}" grub-mkconfig -o /boot/grub/grub.cfg
        ;;

    "Limine")
        pacstrap -K "${TARGET_MNT}" limine
        if [[ "$IS_UEFI" == true ]]; then
            mkdir -p "${EFI_MNT}/EFI/BOOT"
            cp "${TARGET_MNT}/usr/share/limine/BOOTX64.EFI" "${EFI_MNT}/EFI/BOOT/"
        else
            DISK=$(lsblk -no PKNAME "$TARGET_ROOT" | head -n1)
            limine bios-install "/dev/${DISK}"
        fi
        ROOT_UUID=$(blkid -s UUID -o value "$TARGET_ROOT")
        cat <<EOF > "${TARGET_MNT}/boot/limine.cfg"
timeout: 5

/Arch Linux (${CHOSEN_KERNEL})
    protocol: linux
    kernel_path: boot():/vmlinuz-${CHOSEN_KERNEL}
    kernel_cmdline: root=UUID=${ROOT_UUID} rw quiet
    module_path: boot():/initramfs-${CHOSEN_KERNEL}.img
EOF
        ;;

    "rEFInd")
        pacstrap -K "${TARGET_MNT}" refind efibootmgr
        arch-chroot "${TARGET_MNT}" refind-install
        ;;
esac

echo "==> [SUCCESS] System repair completed successfully."