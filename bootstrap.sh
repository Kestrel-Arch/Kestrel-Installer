#!/usr/bin/env bash
set -euo pipefail

# =====================================================================
# REPOSITORY CONFIGURATION
# =====================================================================
GITHUB_REPO="Kestrel-Arch/Kestrel-Installer"

clear
echo "=========================================================="
echo "          KESTREL ARCH UNIVERSAL BOOTSTRAPPER             "
echo "=========================================================="
echo "This script deploys Kestrel Arch on a vanilla Arch Linux ISO."
echo "An active internet connection is strictly required."
echo ""

# =====================================================================
# SMART RAM GATEKEEPER & ZRAM OOM PREVENTION (For 2GB VMs)
# =====================================================================
TOTAL_RAM=$(free -m | awk '/^Mem:/{print $2}')
if [ -n "$TOTAL_RAM" ] && [ "$TOTAL_RAM" -lt 4096 ]; then
    echo "[SYSTEM CHECK] ${TOTAL_RAM}MB RAM detected. Enabling ZRAM (Compressed Swap)..."
    modprobe zram 2>/dev/null || true
    ZRAM_DEV=$(zramctl --find --size 1536M 2>/dev/null) || true

    if [ -n "$ZRAM_DEV" ]; then
        mkswap "$ZRAM_DEV" >/dev/null 2>&1
        swapon "$ZRAM_DEV" -p 32767 || true
        echo "[INFO] ZRAM active on $ZRAM_DEV. Memory capacity expanded!"
    else
        echo "[WARNING] Failed to initialize ZRAM. Proceeding with caution..."
    fi
fi

echo "Select Deployment Interface:"
echo " [1] Graphical Installer & Rescue Engine (Slint GUI & GParted on Wayland)"
echo " [2] Headless CLI Installer (Pure Terminal)"
echo " [3] Headless CLI System Repair (In-Place Rescue)"
echo "----------------------------------------------------------"
while true; do
    read -r -p "Choice (1-3): " UI_CHOICE
    if [[ "$UI_CHOICE" =~ ^[1-3]$ ]]; then break; else echo "[WARNING] Invalid choice."; fi
done

echo "=> Fetching Kestrel Backend & Recovery Engines..."
mkdir -p /usr/local/bin

curl -fL -# --retry 5 --retry-all-errors \
    "https://raw.githubusercontent.com/${GITHUB_REPO}/refs/heads/main/airootfs/usr/local/bin/install.sh" \
    -o /usr/local/bin/install.sh
sed -i 's/\r$//' /usr/local/bin/install.sh
chmod +x /usr/local/bin/install.sh

curl -fL -# --retry 5 --retry-all-errors \
    "https://raw.githubusercontent.com/${GITHUB_REPO}/refs/heads/main/airootfs/usr/local/bin/kestrel-repair.sh" \
    -o /usr/local/bin/kestrel-repair.sh
sed -i 's/\r$//' /usr/local/bin/kestrel-repair.sh
chmod +x /usr/local/bin/kestrel-repair.sh
sync

if [ "$UI_CHOICE" = "1" ]; then
    echo "=> Initializing Graphical Environment in Live RAM..."
    mount -o remount,size=4G /run/archiso/cowspace 2>/dev/null || true

    pacman-key --init >/dev/null 2>&1 || true
    pacman-key --populate archlinux >/dev/null 2>&1 || true

    echo "=> Performing minimal core system upgrade..."
    pacman -Syu --overwrite "*" --ignore "linux,linux-firmware*,intel-ucode,amd-ucode,linux-api-headers,mkinitcpio,sof-firmware,open-vm-tools,virtualbox-guest-utils-nox,vim*,zsh,openvpn,openconnect,man-db,man-pages,nmap,tcpdump,python*,perl*,lvm2,mdadm,nftables,iptables,openssh,partclone,sqlite,cloud-init,hyperv,bolt,broadcom-wl,bcachefs-tools,libtorrent-rasterbar,screen,sg3_utils,tpm2-tools,archinstall,clonezilla" --noconfirm || true

    echo "=> Reclaiming RAM disk space..."
    rm -rf /var/cache/pacman/pkg/*

    echo "=> Installing Wayland, Cage, and UI Dependencies..."
    pacman -S --noconfirm --needed --overwrite "*" cage wayland gparted polkit ttf-liberation pciutils

    echo "=> Clearing final package cache & sync databases..."
    rm -rf /var/cache/pacman/pkg/*
    rm -rf /var/lib/pacman/sync/*

    echo "=> Fetching Kestrel GUI Binary..."
    rm -f /usr/local/bin/kestrel-gui
    curl -fL -# --retry 5 --retry-all-errors --retry-delay 3 -C - \
        "https://github.com/${GITHUB_REPO}/releases/download/latest-gui/kestrel-gui" \
        -o /usr/local/bin/kestrel-gui
    sync
    chmod +x /usr/local/bin/kestrel-gui

    echo "=> Initializing Wayland Compositor Environment..."
    export XDG_RUNTIME_DIR=/run/user/$(id -u)
    mkdir -p "$XDG_RUNTIME_DIR"
    chmod 0700 "$XDG_RUNTIME_DIR"

    export WLR_NO_HARDWARE_CURSORS=1
    export WLR_RENDERER_ALLOW_SOFTWARE=1
    export LIBGL_ALWAYS_SOFTWARE=1
    export SLINT_BACKEND=winit

    IS_VM=$(systemd-detect-virt -q && echo 1 || echo 0)
    HAS_NVIDIA=$(lspci | grep -iE "vga.*nvidia|3d.*nvidia" >/dev/null 2>&1 && echo 1 || echo 0)
    if [ "$IS_VM" = "1" ] || [ "$HAS_NVIDIA" = "1" ]; then
        echo "[INFO] VM or NVIDIA detected. Forcing pure software rendering (Pixman)..."
        export WLR_RENDERER=pixman
    fi

    echo "=> Launching Kestrel GUI..."
    exec cage -s -- /usr/local/bin/kestrel-gui
elif [ "$UI_CHOICE" = "2" ]; then
    echo "=> Launching Headless CLI Engine..."
    export NON_INTERACTIVE=0
    export INSTALL_MODE="1"
    exec /usr/local/bin/install.sh
else
    echo "=> Launching In-Place CLI Recovery..."
    lsblk -o NAME,SIZE,FSTYPE,LABEL,MOUNTPOINT
    read -r -p "Enter root partition (e.g. /dev/nvme0n1p2): " R_PART
    read -r -p "Enter OS ID (arch/cachyos): " R_OS
    read -r -p "Enter Kernel (linux/linux-cachyos): " R_KERN
    read -r -p "Enter Bootloader (GRUB/Limine/systemd-boot/rEFInd): " R_BL
    read -r -p "Enter EFI partition (or leave blank for BIOS): " R_EFI
    exec /usr/local/bin/kestrel-repair.sh "$R_PART" "$R_OS" "$R_KERN" "$R_BL" "$R_EFI"
fi
