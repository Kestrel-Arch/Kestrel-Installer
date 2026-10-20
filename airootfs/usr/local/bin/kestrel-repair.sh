#!/usr/bin/env bash
# ==============================================================================
# Kestrel Arch Intelligent System & Bootloader Repair Engine
# Includes Resilient Pacstrap Failover, Subvolume Detection & ALPM Kernel Guard
# ==============================================================================
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    exec sudo bash "$0" "$@"
fi

TARGET_ROOT="${1:-}"
TARGET_OS_ID="${2:-}"
CHOSEN_KERNEL="${3:-}"
CHOSEN_BL="${4:-}"
EFI_PART="${5:-}"

echo "==> [0/5] Initializing Kestrel Recovery Engine..."
echo "[*] Target Root: $TARGET_ROOT ($TARGET_OS_ID)"
echo "[*] Kernel: $CHOSEN_KERNEL | Bootloader: $CHOSEN_BL | ESP: ${EFI_PART:-None}"

if [[ -z "$TARGET_ROOT" || -z "$CHOSEN_KERNEL" || -z "$CHOSEN_BL" ]]; then
    echo "[-] Error: Missing required arguments."
    echo "Usage: $0 <root_part> <os_id> <kernel> <bootloader> [efi_part]"
    exit 1
fi

TARGET_MNT="/mnt/kestrel_repair"
EFI_MNT="${TARGET_MNT}/boot"

cleanup() {
    umount -R "${TARGET_MNT}" 2>/dev/null || true
    rm -rf "${TARGET_MNT}" 2>/dev/null || true
}
trap cleanup EXIT

# Resilient pacstrap wrapper with mirror failover
kestrel_pacstrap() {
    local mnt="$1"
    shift
    local attempt=1
    while [ $attempt -le 3 ]; do
        echo "[*] Running pacstrap (Attempt $attempt/3): $*"
        if pacstrap -K "$mnt" "$@"; then
            return 0
        fi
        echo "[!] Mirror timeout or transient error detected. Refreshing mirrorlist..."
        reflector --latest 10 --protocol https --sort rate --save /etc/pacman.d/mirrorlist 2>/dev/null || true
        attempt=$((attempt + 1))
        sleep 2
    done
    echo "[-] Critical: pacstrap failed after 3 attempts."
    return 1
}

echo "==> [1/5] Checking hardware firmware and bootloader compatibility..."
IS_UEFI=false
if [[ -d "/sys/firmware/efi" ]]; then
    IS_UEFI=true
    echo "[+] System Firmware: UEFI"
else
    echo "[!] System Firmware: Legacy BIOS (CSM)"
    if [[ "$CHOSEN_BL" == "systemd-boot" || "$CHOSEN_BL" == "rEFInd" ]]; then
        echo "[-] Fatal: Bootloader '$CHOSEN_BL' requires UEFI firmware. Cannot install on BIOS."
        exit 1
    fi
fi

echo "==> [2/5] Mounting target root partition ($TARGET_ROOT)..."
mkdir -p "${TARGET_MNT}"
ROOT_FSTYPE=$(blkid -s TYPE -o value "$TARGET_ROOT" || echo "ext4")

if [[ "$ROOT_FSTYPE" == "btrfs" ]]; then
    mount -o subvol=@ "$TARGET_ROOT" "${TARGET_MNT}" 2>/dev/null || mount "$TARGET_ROOT" "${TARGET_MNT}"
else
    mount "$TARGET_ROOT" "${TARGET_MNT}"
fi

# Clear any stale pacman lockfile from a previously crashed update
rm -f "${TARGET_MNT}/var/lib/pacman/db.lck"

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

echo "==> Deploying ${CHOSEN_KERNEL} and headers with mirror failover..."
kestrel_pacstrap "${TARGET_MNT}" "${CHOSEN_KERNEL}" "${CHOSEN_KERNEL}-headers" mkinitcpio

echo "==> [4/5] Installing Kestrel Self-Healing ALPM Guard & Regenerating initramfs..."
mkdir -p "${TARGET_MNT}/usr/local/bin" "${TARGET_MNT}/etc/pacman.d/hooks"

cat << 'GUARD_EOF' > "${TARGET_MNT}/usr/local/bin/kestrel-doctor"
#!/usr/bin/env bash
# Kestrel Doctor & Post-Transaction Self-Healing Kernel Verifier
set -euo pipefail
if [[ $EUID -ne 0 ]]; then exec sudo bash "$0" "$@"; fi

if [[ -f /var/lib/pacman/db.lck ]] && ! pgrep -x pacman >/dev/null; then
    echo "[Kestrel Doctor] Removing stale pacman db.lck..."
    rm -f /var/lib/pacman/db.lck
fi

for kver_dir in /usr/lib/modules/*; do
    [[ -d "$kver_dir" ]] || continue
    pkgbase_file="${kver_dir}/pkgbase"
    if [[ -f "$pkgbase_file" ]]; then
        pkgbase=$(cat "$pkgbase_file")
        if [[ ! -f "/boot/vmlinuz-${pkgbase}" ]] || [[ ! -s "/boot/initramfs-${pkgbase}.img" ]]; then
            echo "[Kestrel Doctor] Missing or desynchronized boot image for ${pkgbase}. Rebuilding..."
            cp -f "${kver_dir}/vmlinuz" "/boot/vmlinuz-${pkgbase}" 2>/dev/null || true
            mkinitcpio -p "${pkgbase}"
        fi
    fi
done
echo "[Kestrel Doctor] Boot & kernel integrity verified."
GUARD_EOF
chmod +x "${TARGET_MNT}/usr/local/bin/kestrel-doctor"

cat << 'HOOK_EOF' > "${TARGET_MNT}/etc/pacman.d/hooks/99-kestrel-guard.hook"
[Trigger]
Operation = Install
Operation = Upgrade
Type = Path
Target = usr/lib/modules/*/vmlinuz

[Action]
Description = [Kestrel Guard] Verifying kernel & initramfs boot integrity...
When = PostTransaction
Exec = /usr/local/bin/kestrel-doctor
HOOK_EOF

arch-chroot "${TARGET_MNT}" mkinitcpio -P

echo "==> [5/5] Deploying and configuring bootloader (${CHOSEN_BL})..."
case "$CHOSEN_BL" in
    "systemd-boot")
        arch-chroot "${TARGET_MNT}" bootctl install || arch-chroot "${TARGET_MNT}" bootctl update
        mkdir -p "${EFI_MNT}/loader/entries"
        cat <<EOF > "${EFI_MNT}/loader/loader.conf"
default arch.conf
timeout 4
console-mode max
editor no
