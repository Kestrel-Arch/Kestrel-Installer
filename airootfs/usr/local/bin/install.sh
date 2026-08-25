#!/bin/bash
set -eE -o pipefail

# CRITICAL GUI FIX: Merge standard error into standard output. 
# This guarantees that the Slint GUI captures the EXACT raw errors from failing commands.
exec 2>&1

# =====================================================================
#              FAIL-SAFE TELEMETRY AND ERROR TRAPPING ENGINE
# =====================================================================
STATUS_FILE="/tmp/kestrel_status"

update_status() {
    local clean_msg=$(echo -e "$1" | sed -r 's/\x1B\[([0-9]{1,2}(;[0-9]{1,2})?)?[mGK]//g')
    echo "$clean_msg" > "$STATUS_FILE"
    
    if [ "$NON_INTERACTIVE" = "1" ]; then
        echo "$clean_msg"
    fi
}

error_handler() {
    local exit_code=$1
    local line_number=$2
    
    update_status "ERROR: Deployment failed at line $line_number. Check log above for raw error."
    
    echo -e "\n=========================================================="
    echo "         CRITICAL FAULT DETECTED BY KESTREL         "
    echo "=========================================================="
    echo "[FAULT] Command failed with exit code: $exit_code"
    echo "[LOCATION] Failed execution occurred on line: $line_number"
    
    if [ "$NON_INTERACTIVE" = "1" ]; then
        echo "[INFO] GUI Mode active. Aborting deployment."
        umount -R /mnt &>/dev/null || true
        swapoff -a &>/dev/null || true
        exit "$exit_code"
    fi

    echo "----------------------------------------------------------"
    echo "Options:"
    echo " [1] Force safe unmount and restart system execution"
    echo " [2] Drop into live emergency recovery shell (Zsh)"
    echo "----------------------------------------------------------"
    read -r -p "Select recovery path (1-2): " FAULT_CHOICE
    
    if [ "$FAULT_CHOICE" = "2" ]; then
        echo "[INFO] Handing over root bash console. Type 'exit' to return."
        /bin/zsh --no-rcs || true
    fi
    
    umount -R /mnt &>/dev/null || true
    swapoff -a &>/dev/null || true
    sleep 2
    reboot || true
    exit "$exit_code"
}

trap 'error_handler $? $LINENO' ERR

# =====================================================================
#              GUI / HEADLESS OVERRIDE MODULE
# =====================================================================
if [ "$NON_INTERACTIVE" = "1" ]; then
    update_status "PROGRESS: Initializing non-interactive GUI deployment..."
    echo "[INFO] Non-Interactive GUI Mode Engaged."
    
    clear() { :; }
    
    TARGET_DRIVE="${TARGET_DISK}"
    INSTALL_MODE="${INSTALL_MODE:-2}"
    
    case "$PARTITION_STRATEGY" in
        "1") INSTALL_STRATEGY="WIPE" ;;
        "2") INSTALL_STRATEGY="REPLACE" ;;
        "3") INSTALL_STRATEGY="MANUAL" ;;
        *) INSTALL_STRATEGY="WIPE" ;;
    esac

    FILESYSTEM="${GUI_FILESYSTEM:-ext4}"
    GUI_REPLACE_PART="${GUI_REPLACE_PART:-}"
    GUI_ROOT_PART="${GUI_ROOT_PART:-}"
    GUI_EFI_PART="${GUI_EFI_PART:-}"
    system_hostname="${GUI_HOSTNAME:-kestrel}"
    username="${GUI_USERNAME:-kestrel}"
    user_password="${GUI_PASSWORD:-password}"
    root_password="${GUI_ROOT_PASSWORD:-$user_password}"
    BROWSER_CHOICE="${BROWSER_CHOICE:-1}"
    PERF_CHOICE="${PERF_CHOICE:-Y}"
    DE_CHOICE="${DE_CHOICE:-1}" 
    BOOT_CHOICE="${BOOT_CHOICE:-1}" 
fi

clear
echo "=========================================================="
echo "                 KESTREL ARCH DEPLOYMENT ENGINE                 "
echo "=========================================================="
echo ""

TARGET="/mnt"
ISO_CACHE="/opt/offline_cache"
GRUB_OS_PROBER="true" 
EFI_DIR="/boot"
DISPLAY_MANAGER="sddm"

# =====================================================================
#              CPU ARCHITECTURE DETECTION (v1 vs v3)
# =====================================================================
if /usr/lib/ld-linux-x86-64.so.2 --help | grep -q "x86-64-v3 (supported, searched)"; then
    echo "[INFO] Modern CPU architecture (x86-64-v3) detected. Using CachyOS Kernel."
    KERNEL_PKG="linux-cachyos linux-cachyos-headers"
    VMLINUZ="vmlinuz-linux-cachyos"
    INITRAMFS="initramfs-linux-cachyos.img"
else
    echo "[WARNING] Legacy CPU architecture detected. Falling back to Standard Arch Kernel."
    update_status "WARNING: Legacy CPU architecture detected. Using standard Arch kernel."
    KERNEL_PKG="linux linux-headers"
    VMLINUZ="vmlinuz-linux"
    INITRAMFS="initramfs-linux.img"
fi

CORE_PKGS="base $KERNEL_PKG linux-firmware scx-scheds efibootmgr os-prober ntfs-3g networkmanager iwd bluez bluez-utils blueman pipewire pipewire-pulse wireplumber brightnessctl flatpak xorg-server sudo zram-generator earlyoom reflector ttf-dejavu ttf-liberation noto-fonts noto-fonts-emoji curl chaotic-keyring chaotic-mirrorlist parted foot git stow qt5-wayland qt6-wayland tesseract-data-eng"

# =====================================================================
# DYNAMIC VIRTUAL MACHINE DETECTOR
# =====================================================================
if systemd-detect-virt -q; then
    echo "[INFO] Virtual Machine detected. Injecting VM guest utilities for Wayland support..."
    update_status "PROGRESS: Virtual Machine detected. Adding guest tools..."
    CORE_PKGS="$CORE_PKGS virtualbox-guest-utils xf86-video-vmware xf86-video-qxl qemu-guest-agent"
fi

if [ -z "$INSTALL_MODE" ]; then
    update_status "PROGRESS: Determining installation mode..."
    if [ -d "$ISO_CACHE" ]; then
        echo "Choose your connection architecture:"
        echo " [1] ONLINE INSTALL - Download the absolute latest packages."
        echo " [2] OFFLINE INSTALL - 100% Air-gapped deployment."
        echo ""
        while true; do
            read -r -p "Select mode (1-2): " INSTALL_MODE
            if [[ "$INSTALL_MODE" =~ ^[1-2]$ ]]; then break; else echo "[WARNING] Invalid option."; fi
        done
    else
        echo "[INFO] Standard Arch Linux ISO Detected."
        echo "[INFO] Locking deployment to ONLINE mode (No offline cache present)."
        INSTALL_MODE="1"
        sleep 3
    fi
fi

# =====================================================================
#              DYNAMIC HARDWARE DRIVE DETECTOR
# =====================================================================
if [ -z "$TARGET_DRIVE" ]; then
    clear
    echo "=========================================================="
    echo "                 TARGET DISK SELECTION MODULE                "
    echo "=========================================================="
    lsblk -d -o NAME,SIZE,MODEL,TYPE | grep -E "disk|nvme|loop|mmc" || true
    echo "----------------------------------------------------------"

    while true; do
        read -r -p "Type your destination installation disk (e.g., /dev/sda): " TARGET_DRIVE
        if [ -b "$TARGET_DRIVE" ]; then break; else echo "[ERROR] Device path does not exist. Try again."; fi
    done
fi

umount -R /mnt &>/dev/null || true
if [[ "$TARGET_DRIVE" =~ [0-9]$ ]]; then PART_PREFIX="p"; else PART_PREFIX=""; fi

# =====================================================================
#              NETWORK ENGAGEMENT ENGINE
# =====================================================================
if [ "$INSTALL_MODE" = "1" ] && [ "$NON_INTERACTIVE" != "1" ]; then
    update_status "PROGRESS: Establishing Network Connection..."
    while true; do
        clear
        if ping -c 1 -W 2 archlinux.org &> /dev/null; then echo "[SUCCESS] Active network connection detected!"; sleep 2; break; fi
        
        WIFI_IFACE=$(iw dev 2>/dev/null | awk '$1=="Interface"{print $2}' | head -n 1) || true
        if [ -z "$WIFI_IFACE" ]; then
            read -r -p "No Wi-Fi adapter detected! Switch to OFFLINE mode? (y/N): " ESCAPE_CHOICE
            if [[ "$ESCAPE_CHOICE" =~ ^[Yy]$ ]]; then 
                if [ -d "$ISO_CACHE" ]; then INSTALL_MODE="2"; break; else exit 1; fi
            else exit 1; fi
        fi

        iwctl station "$WIFI_IFACE" scan || true; sleep 2
        iwctl station "$WIFI_IFACE" get-networks || true
        read -r -p "SSID Selection (or type CANCEL): " WIFI_SSID
        
        if [ "$WIFI_SSID" = "CANCEL" ] || [ -z "$WIFI_SSID" ]; then 
            if [ -d "$ISO_CACHE" ]; then INSTALL_MODE="2"; break; else exit 1; fi
        fi
        read -r -s -p "Enter Wi-Fi Password: " WIFI_PASS; echo ""
        
        if [ -z "$WIFI_PASS" ]; then iwctl station "$WIFI_IFACE" connect "$WIFI_SSID" || true
        else iwctl --passphrase "$WIFI_PASS" station "$WIFI_IFACE" connect "$WIFI_SSID" || true; fi
        
        sleep 5
        if ping -c 1 -W 2 archlinux.org &> /dev/null; then break; fi
    done
fi

# =====================================================================
#              STORAGE PROVISIONING PATHWAY
# =====================================================================
clear
update_status "PROGRESS: Preparing Storage and Partitioning..."

if [ ! -d "$ISO_CACHE" ]; then pacman -Sy --noconfirm ntfs-3g parted gparted >/dev/null 2>&1 || true; fi

# BUG FIX: Force FILESYSTEM to strictly lowercase and remove any phantom spaces
FILESYSTEM="${FILESYSTEM:-ext4}"
FILESYSTEM=$(echo "$FILESYSTEM" | tr '[:upper:]' '[:lower:]' | tr -d ' ')

PROVISIONING_COMPLETE=0
RESET_STRATEGY=0

while [ "$PROVISIONING_COMPLETE" -eq 0 ]; do

    if [ -z "$INSTALL_STRATEGY" ] || [ "$RESET_STRATEGY" = "1" ]; then
        echo "=========================================================="
        echo "          STEP 2: STORAGE PROVISIONING PATHWAY            "
        echo "=========================================================="
        echo " [1] WIPE    - Erase entire disk and install fresh."
        echo " [2] REPLACE - Format a specific target partition."
        echo " [3] MANUAL  - Launch Partition Manager (GParted/cfdisk)."
        while true; do 
            read -r -p "Enter your choice (1-3): " MENU_CHOICE
            case $MENU_CHOICE in
                1) INSTALL_STRATEGY="WIPE"; RESET_STRATEGY=0; break ;;
                2) INSTALL_STRATEGY="REPLACE"; RESET_STRATEGY=0; break ;;
                3) INSTALL_STRATEGY="MANUAL"; RESET_STRATEGY=0; break ;;
                *) echo "[WARNING] Invalid option." ;;
            esac
        done
    fi

    echo "STARTING: Executing partitioning strategy: $INSTALL_STRATEGY on $TARGET_DRIVE..."

    case $INSTALL_STRATEGY in
        "WIPE")
            echo "====== HARD NUKE: WIPE ENTIRE DRIVE ======"
            if [ "$NON_INTERACTIVE" != "1" ]; then
                read -r -p "DANGER: Type 'YES' to confirm erasing $TARGET_DRIVE: " CONFIRM_NUKE
                [[ "${CONFIRM_NUKE^^}" != "YES" ]] && exit 1
            fi
            sleep 2
            
            wipefs -a "$TARGET_DRIVE" &>/dev/null || true
            
            if [ -d "/sys/firmware/efi" ]; then
                parted -s "$TARGET_DRIVE" mklabel gpt
                parted -s -a optimal "$TARGET_DRIVE" mkpart primary fat32 1MiB 513MiB
                parted -s "$TARGET_DRIVE" set 1 esp on
                parted -s -a optimal "$TARGET_DRIVE" mkpart primary "$FILESYSTEM" 513MiB 100%
                
                partprobe "$TARGET_DRIVE"; udevadm settle; sleep 2
                ARCH_EFI="${TARGET_DRIVE}${PART_PREFIX}1"
                ARCH_ROOT="${TARGET_DRIVE}${PART_PREFIX}2"
                
                wipefs -a "$ARCH_EFI" &>/dev/null || true
                wipefs -a "$ARCH_ROOT" &>/dev/null || true
                
                echo "[INFO] Formatting $ARCH_EFI to FAT32..."
                mkfs.vfat -F 32 "$ARCH_EFI"
                echo "[INFO] Formatting $ARCH_ROOT to $FILESYSTEM..."
                if [ "$FILESYSTEM" = "btrfs" ]; then mkfs.btrfs -f "$ARCH_ROOT"; else mkfs.ext4 -O ^orphan_file,^metadata_csum_seed -F "$ARCH_ROOT"; fi
                
                mount "$ARCH_ROOT" "$TARGET"
                mkdir -p "$TARGET$EFI_DIR"
                mount -t vfat "$ARCH_EFI" "$TARGET$EFI_DIR"
            else
                echo "[INFO] Legacy BIOS Detected. Building dedicated Boot & Root partitions..."
                parted -s "$TARGET_DRIVE" mklabel msdos
                parted -s -a optimal "$TARGET_DRIVE" mkpart primary fat32 2MiB 514MiB
                parted -s "$TARGET_DRIVE" set 1 boot on
                parted -s -a optimal "$TARGET_DRIVE" mkpart primary "$FILESYSTEM" 514MiB 100%
                
                partprobe "$TARGET_DRIVE"; udevadm settle; sleep 2
                ARCH_BOOT="${TARGET_DRIVE}${PART_PREFIX}1"
                ARCH_ROOT="${TARGET_DRIVE}${PART_PREFIX}2"
                
                wipefs -a "$ARCH_BOOT" &>/dev/null || true
                wipefs -a "$ARCH_ROOT" &>/dev/null || true
                
                echo "[INFO] Formatting $ARCH_BOOT to FAT32 (Limine Compatible)..."
                mkfs.vfat -F 32 "$ARCH_BOOT"
                
                echo "[INFO] Formatting $ARCH_ROOT to $FILESYSTEM..."
                if [ "$FILESYSTEM" = "btrfs" ]; then mkfs.btrfs -f "$ARCH_ROOT"; else mkfs.ext4 -O ^orphan_file,^metadata_csum_seed -F "$ARCH_ROOT"; fi
                
                mount "$ARCH_ROOT" "$TARGET"
                mkdir -p "$TARGET/boot"
                mount -t vfat "$ARCH_BOOT" "$TARGET/boot"
                EFI_DIR="/boot"
            fi
            GRUB_OS_PROBER="true"
            PROVISIONING_COMPLETE=1
            ;;
            
        "REPLACE")
            echo "====== REPLACE TARGET PARTITION ======"
            if [ "$NON_INTERACTIVE" != "1" ]; then
                if [ -z "$ARCH_ROOT" ]; then
                    lsblk "$TARGET_DRIVE" -o NAME,SIZE,TYPE,FSTYPE
                    while true; do
                        read -r -p "Enter partition to FORMAT and REPLACE (e.g., /dev/sda2): " ARCH_ROOT
                        if [ -b "$ARCH_ROOT" ] && [ "$ARCH_ROOT" != "$TARGET_DRIVE" ]; then break; fi
                    done
                fi
                read -r -p "Type 'NUKE' to erase $ARCH_ROOT: " CONFIRM_NUKE
                [[ "${CONFIRM_NUKE^^}" != "NUKE" ]] && exit 1
            else
                ARCH_ROOT="$GUI_REPLACE_PART"
                echo "[INFO] GUI Mode active. Targeting $ARCH_ROOT for replacement."
            fi
            
            wipefs -a "$ARCH_ROOT" &>/dev/null || true
            
            echo "[INFO] Formatting $ARCH_ROOT to $FILESYSTEM..."
            if [ "$FILESYSTEM" = "btrfs" ]; then mkfs.btrfs -f "$ARCH_ROOT"; else mkfs.ext4 -O ^orphan_file,^metadata_csum_seed -F "$ARCH_ROOT"; fi
            mount "$ARCH_ROOT" "$TARGET"
            
            # BUG FIX: Allow Legacy BIOS GUI to properly mount a replacement boot partition
            if [ -n "$GUI_EFI_PART" ] && [ -b "$GUI_EFI_PART" ]; then
                if [ -d "/sys/firmware/efi" ]; then
                    echo "[INFO] Safely mounting $GUI_EFI_PART to $EFI_DIR (Preserving existing bootloaders)..."
                    mkdir -p "$TARGET$EFI_DIR"
                    mount -t vfat "$GUI_EFI_PART" "$TARGET$EFI_DIR"
                else
                    echo "[INFO] Legacy BIOS: Mounting Boot Partition..."
                    mkdir -p "$TARGET/boot"
                    mount -t vfat "$GUI_EFI_PART" "$TARGET/boot" || mount "$GUI_EFI_PART" "$TARGET/boot"
                    PART_NUM=$(echo "$GUI_EFI_PART" | grep -o '[0-9]*$')
                    parted -s "$TARGET_DRIVE" set "$PART_NUM" boot on || true
                fi
            else
                mkdir -p "$TARGET/boot"
            fi
            GRUB_OS_PROBER="false"
            PROVISIONING_COMPLETE=1
            ;;

        "MANUAL")
            echo "====== ADVANCED PROVISIONING ======"
            if [ "$NON_INTERACTIVE" != "1" ]; then
                echo "[INFO] Launching Partition Manager..."
                echo "----------------------------------------------------------"
                echo "👉 INSTRUCTIONS: Build your partitions, Apply/Write changes,"
                echo "   and then exit the manager. The installer will automatically resume!"
                echo "----------------------------------------------------------"
                sleep 4
                
                while true; do
                    sfdisk -d "$TARGET_DRIVE" > /tmp/kestrel_part_before 2>/dev/null || true
                    
                    if { [ -n "$WAYLAND_DISPLAY" ] || [ -n "$DISPLAY" ]; } && command -v gparted &> /dev/null; then
                        gparted "$TARGET_DRIVE" || true
                    else
                        cfdisk "$TARGET_DRIVE" || true
                    fi
                    
                    sfdisk -d "$TARGET_DRIVE" > /tmp/kestrel_part_after 2>/dev/null || true
                    
                    if cmp -s /tmp/kestrel_part_before /tmp/kestrel_part_after; then
                        clear
                        echo "=========================================================="
                        echo " [WARNING] No changes were written to the drive!"
                        echo "=========================================================="
                        echo " Options:"
                        echo "  [1] I forgot to apply/save (Go back to manager)"
                        echo "  [2] I want to proceed anyway (Assign Mount Points)"
                        echo "  [3] Abort and go back to Main Menu (Select WIPE/REPLACE)"
                        echo "----------------------------------------------------------"
                        read -r -p "Choice (1-3): " CFDISK_CHOICE
                        
                        if [ "$CFDISK_CHOICE" = "1" ]; then continue
                        elif [ "$CFDISK_CHOICE" = "3" ]; then RESET_STRATEGY=1; break
                        else break; fi
                    else break; fi
                done
                
                if [ "$RESET_STRATEGY" = "1" ]; then continue; fi
                
                clear
                echo "====== ASSIGN MOUNT POINTS & FORMATTING ======"
                lsblk "$TARGET_DRIVE" -o NAME,SIZE,TYPE,FSTYPE
                echo "----------------------------------------------------------"
                
                while true; do
                    read -r -p "Enter partition for Arch ROOT (e.g., /dev/sda2): " ARCH_ROOT
                    if [ -z "$ARCH_ROOT" ]; then continue; fi
                    if [ -b "$ARCH_ROOT" ]; then
                        read -r -p "Format $ARCH_ROOT? (y/N): " FORMAT_ROOT
                        if [[ "$FORMAT_ROOT" =~ ^[Yy]$ ]]; then
                            read -r -p "Select Filesystem (ext4/btrfs) [default: ext4]: " ROOT_FS
                            ROOT_FS=${ROOT_FS:-ext4}
                            ROOT_FS=$(echo "$ROOT_FS" | tr '[:upper:]' '[:lower:]' | tr -d ' ')
                            wipefs -a "$ARCH_ROOT" &>/dev/null || true
                            if [ "$ROOT_FS" = "btrfs" ]; then mkfs.btrfs -f "$ARCH_ROOT"; else mkfs.ext4 -O ^orphan_file,^metadata_csum_seed -F "$ARCH_ROOT"; fi
                        fi
                        if mount "$ARCH_ROOT" "$TARGET"; then break; else echo "[ERROR] Mount failed."; fi
                    else echo "[ERROR] Invalid partition path."; fi
                done
                
                if [ -d "/sys/firmware/efi" ]; then
                    while true; do
                        read -r -p "Enter EFI partition path (e.g., /dev/sda1): " ARCH_EFI
                        if [ -z "$ARCH_EFI" ]; then continue; fi
                        if [ -b "$ARCH_EFI" ]; then
                            read -r -p "Format $ARCH_EFI to FAT32? (y/N): " FORMAT_EFI
                            if [[ "$FORMAT_EFI" =~ ^[Yy]$ ]]; then 
                                wipefs -a "$ARCH_EFI" &>/dev/null || true
                                mkfs.vfat -F 32 "$ARCH_EFI"
                            fi
                            mkdir -p "$TARGET$EFI_DIR"
                            if mount -t vfat "$ARCH_EFI" "$TARGET$EFI_DIR"; then break; else echo "[ERROR] Mount failed."; fi
                        else echo "[ERROR] Invalid partition path."; fi
                    done
                else
                    mkdir -p "$TARGET/boot"
                fi
                
                read -r -p "Enable OS Prober to detect other Operating Systems? (Y/n): " MANUAL_PROBER
                if [[ "$MANUAL_PROBER" =~ ^[Nn]$ ]]; then GRUB_OS_PROBER="false"; else GRUB_OS_PROBER="true"; fi

            else
                echo "[INFO] GUI Advanced Mode active. Applying pre-configured mounts..."
                
                if [ -b "$GUI_ROOT_PART" ]; then
                    wipefs -a "$GUI_ROOT_PART" &>/dev/null || true
                    echo "[INFO] Formatting $GUI_ROOT_PART to $FILESYSTEM..."
                    if [ "$FILESYSTEM" = "btrfs" ]; then mkfs.btrfs -f "$GUI_ROOT_PART"; else mkfs.ext4 -O ^orphan_file,^metadata_csum_seed -F "$GUI_ROOT_PART"; fi
                    mount "$GUI_ROOT_PART" "$TARGET"
                else
                    echo "[ERROR] Invalid or missing GUI ROOT partition."
                    exit 1
                fi
                
                # BUG FIX: Legacy BIOS GUI EFI/Boot mapping 
                if [ -n "$GUI_EFI_PART" ] && [ -b "$GUI_EFI_PART" ]; then
                    if [ -d "/sys/firmware/efi" ]; then
                        echo "[INFO] Safely mounting $GUI_EFI_PART to $EFI_DIR (Preserving existing bootloaders)..."
                        mkdir -p "$TARGET$EFI_DIR"
                        mount -t vfat "$GUI_EFI_PART" "$TARGET$EFI_DIR"
                    else
                        echo "[INFO] Legacy BIOS: Mounting Boot Partition..."
                        mkdir -p "$TARGET/boot"
                        mount -t vfat "$GUI_EFI_PART" "$TARGET/boot" || mount "$GUI_EFI_PART" "$TARGET/boot"
                        
                        # Lenovo strictly requires the active flag!
                        PART_NUM=$(echo "$GUI_EFI_PART" | grep -o '[0-9]*$')
                        parted -s "$TARGET_DRIVE" set "$PART_NUM" boot on || true
                    fi
                else
                    mkdir -p "$TARGET/boot"
                fi
                GRUB_OS_PROBER="true"
            fi
            
            PROVISIONING_COMPLETE=1
            ;;
    esac
done

# =====================================================================
#              ACCOUNT CREATION
# =====================================================================
update_status "PROGRESS: Configuring User Accounts..."
clear
echo "=========================================================="
echo "              STEP 3: ACCOUNT CREATION                    "
echo "=========================================================="
if [ -z "$system_hostname" ]; then
    read -r -p "Enter Hostname: " system_hostname
    system_hostname=$(printf '%s\n' "$system_hostname" | tr -cd 'a-zA-Z0-9-' | tr '[:upper:]' '[:lower:]')
    [ -z "$system_hostname" ] && system_hostname="kestrel-node"
fi
if [ -z "$username" ]; then
    read -r -p "Enter new username: " username
    username=$(printf '%s\n' "$username" | tr -cd 'a-z0-9_')
    [ -z "$username" ] && username="kestrel_user"
fi
if [ -z "$user_password" ]; then
    while true; do read -r -s -p "Enter password for $username: " user_password; echo ""; [ -n "$user_password" ] && break; done
fi
if [ -z "$root_password" ]; then
    read -r -p "Use same password for 'root'? [Y/n]: " SAME_ROOT
    if [[ "$SAME_ROOT" =~ ^[Nn]$ ]]; then
        while true; do read -r -s -p "Enter root password: " root_password; echo ""; [ -n "$root_password" ] && break; done
    else root_password="$user_password"; fi
fi

# =====================================================================
#              SOFTWARE CONFIGURATION & DE MATRIX
# =====================================================================
update_status "PROGRESS: Configuring Software Matrix..."
clear
if [ -z "$BROWSER_CHOICE" ]; then
    if [ "$INSTALL_MODE" = "1" ]; then
        echo "Select your primary web browser:"
        echo " [1] Zen Browser (Recommended)"
        echo " [2] LibreWolf"
        echo " [3] Firefox"
        echo " [4] Brave"
        echo " [5] Falkon Browser (Recommended for KDE/Arch)"
        read -r -p "Choice (1-5): " BROWSER_CHOICE
    else
        echo "[INFO] Offline Mode: Defaulting to Falkon Browser."
        BROWSER_CHOICE="5"
        sleep 3
    fi
fi

case $BROWSER_CHOICE in 
    1) CORE_PKGS="$CORE_PKGS zen-browser-bin" ;;
    2) CORE_PKGS="$CORE_PKGS librewolf" ;; 
    3) CORE_PKGS="$CORE_PKGS firefox" ;; 
    4) CORE_PKGS="$CORE_PKGS brave-bin" ;; 
    5) CORE_PKGS="$CORE_PKGS falkon" ;;
esac

if [ -z "$PERF_CHOICE" ]; then read -r -p "Apply Hyper-Performance Matrix? (ZRAM, Fast I/O) [Y/n]: " PERF_CHOICE; fi

UCODE_IMG=""
if grep -q "AuthenticAMD" /proc/cpuinfo; then CORE_PKGS="$CORE_PKGS amd-ucode"; UCODE_IMG="amd-ucode.img"; elif grep -q "GenuineIntel" /proc/cpuinfo; then CORE_PKGS="$CORE_PKGS intel-ucode"; UCODE_IMG="intel-ucode.img"; fi

HAS_NVIDIA=0; HAS_INTEGRATED=0
if lspci | grep -iq nvidia; then CORE_PKGS="$CORE_PKGS nvidia nvidia-utils nvidia-prime"; HAS_NVIDIA=1; fi
if lspci | grep -E -iq "amd|intel"; then HAS_INTEGRATED=1; fi

if [ "$HAS_NVIDIA" -eq 1 ] && [ "$HAS_INTEGRATED" -eq 1 ]; then
    CORE_PKGS="$CORE_PKGS switcheroo-control"
fi

if lspci | grep -i vga | grep -iq amd; then CORE_PKGS="$CORE_PKGS xf86-video-amdgpu"; fi
if lspci | grep -i vga | grep -iq intel; then CORE_PKGS="$CORE_PKGS intel-media-driver"; fi

if [ -z "$DE_CHOICE" ]; then
    if [ "$INSTALL_MODE" = "2" ]; then
        echo "=== OFFLINE DESKTOP ENVIRONMENT SELECTOR ==="
        echo " [1] Hyprland   - A visually pleasing dynamic tiling Wayland compositor."
        echo " [2] KDE Plasma - A comprehensive, flexible, highly customizable desktop environment."
        echo " [3] XFCE       - Modern, lightweight, stable, traditional drop-down layout."
        while true; do read -r -p "Choice (1-3): " DE_CHOICE; [[ "$DE_CHOICE" =~ ^[1-3]$ ]] && break; done
    else
        echo "=== ONLINE DESKTOP ENVIRONMENT SELECTOR ==="
        echo " [1] Hyprland   - Visually pleasing Wayland compositor using dynamic tiling."
        echo " [2] KDE Plasma - Flexible desktop offering multiple styles of menus and KWin."
        echo " [3] XFCE       - Lightweight and flexible core with traditional desktop flow."
        echo " [4] GNOME      - User-friendly desktop environment with a modern touch layout."
        echo " [5] Sway       - Tiling Wayland compositor; dynamic drop-in i3 upgrade."
        echo " [6] i3-wm      - Popular X11 tiling manager leveraging clean text configurations."
        echo " [7] Cinnamon   - Traditional desktop paradigm balancing advanced internal features."
        echo " [8] Niri       - Scrollable tiling Wayland compositor optimizing fluid layout grid."
        echo " [9] Qtile      - Highly configurable X11/Wayland environment scripted entirely in Python."
        echo " [10] Wayfire    - Wlroots Wayland engine mixing structural performance aesthetics."
        echo " [11] bspwm      - Binary space partitioning X11 architecture tracking strict window layouts."
        echo " [12] Budgie     - Clean and elegant GTK interface prioritizing absolute modern ergonomics."
        echo " [13] Cosmic     - Modern performance Rust workspace constructed for absolute responsiveness."
        echo " [14] LXDE       - Fast, lightweight X11 environment tailored heavily for legacy hardware."
        echo " [15] LXQt       - Blazing fast lightweight environment engineered entirely on the Qt stack."
        echo " [16] Matede     - Classic GNOME 2 fork tracking legacy desktop layouts natively."
        echo " [17] Openbox    - Highly custom X11 window manager featuring extensive internal canvas styling."
        while true; do read -r -p "Choice (1-17): " DE_CHOICE; [[ "$DE_CHOICE" =~ ^[1-9]$|^1[0-7]$ ]] && break; done
    fi
fi

case $DE_CHOICE in
    1) CORE_PKGS="$CORE_PKGS hyprland waybar kitty rofi-wayland xdg-desktop-portal-hyprland polkit-kde-agent thunar gvfs sddm" ;;
    2) CORE_PKGS="$CORE_PKGS plasma-desktop plasma-workspace plasma-nm power-profiles-daemon kscreen konsole dolphin ark kate spectacle discover packagekit-qt6 sddm-kcm sddm" ;;
    3) CORE_PKGS="$CORE_PKGS xfce4 xfce4-goodies xfce4-terminal sddm" ;;
    4) CORE_PKGS="$CORE_PKGS gnome gnome-tweaks gdm"; DISPLAY_MANAGER="gdm" ;;
    5) CORE_PKGS="$CORE_PKGS sway swaybg swaylock swayidle waybar kitty rofi-wayland xdg-desktop-portal-wlr polkit-kde-agent thunar gvfs sddm" ;;
    6) CORE_PKGS="$CORE_PKGS i3-wm i3status i3lock dmenu kitty picom feh polkit-gnome thunar gvfs sddm" ;;
    7) CORE_PKGS="$CORE_PKGS cinnamon nemo-fileroller gnome-terminal sddm" ;;
    8) CORE_PKGS="$CORE_PKGS niri waybar kitty rofi-wayland xdg-desktop-portal-gnome polkit-kde-agent thunar gvfs sddm" ;;
    9) CORE_PKGS="$CORE_PKGS qtile kitty rofi-wayland xdg-desktop-portal-wlr polkit-kde-agent thunar gvfs sddm" ;;
    10) CORE_PKGS="$CORE_PKGS wayfire wayfire-plugins-extra kitty rofi-wayland xdg-desktop-portal-wlr polkit-kde-agent thunar gvfs sddm" ;;
    11) CORE_PKGS="$CORE_PKGS bspwm sxhkd kitty dmenu picom feh polkit-gnome thunar gvfs sddm" ;;
    12) CORE_PKGS="$CORE_PKGS budgie-desktop sddm" ;;
    13) CORE_PKGS="$CORE_PKGS cosmic sddm" ;;
    14) CORE_PKGS="$CORE_PKGS lxde-common lxsession openbox sddm" ;;
    15) CORE_PKGS="$CORE_PKGS lxqt lxqt-session sddm" ;;
    16) CORE_PKGS="$CORE_PKGS mate mate-extra sddm" ;;
    17) CORE_PKGS="$CORE_PKGS openbox obconf tint2 kitty dmenu feh sddm" ;;
esac

# =====================================================================
#              BOOT MANAGER CONFIGURATION STAGE
# =====================================================================
clear
if [ -z "$BOOT_CHOICE" ]; then
    echo "=== BOOT MANAGER CONFIGURATION MATRIX ==="
    echo " [1] GRUB          - Flexible multi-boot loader; supports auto BTRFS snapshot lists."
    echo " [2] systemd-boot  - Light minimalist EFI manager tracking kernel slots in loader.conf."
    echo " [3] rEFInd        - Rich graphical shell automatically identifying boot options."
    echo " [4] Limine        - Modern lightning-fast system deployed cleanly via limine.conf."
    while true; do read -r -p "Choice (1-4): " BOOT_CHOICE; [[ "$BOOT_CHOICE" =~ ^[1-4]$ ]] && break; done
fi

# Gatekeeper: systemd-boot & rEFInd require UEFI
if [ ! -d "/sys/firmware/efi" ]; then
    if [ "$BOOT_CHOICE" = "2" ] || [ "$BOOT_CHOICE" = "3" ]; then
        echo "[WARNING] systemd-boot and rEFInd require UEFI. Falling back to GRUB for Legacy BIOS."
        update_status "WARNING: Selected bootloader requires UEFI. Enforcing GRUB fallback..."
        BOOT_CHOICE="1"
        sleep 2
    fi
fi

case $BOOT_CHOICE in
    1) CORE_PKGS="$CORE_PKGS grub" ;;
    2) : ;; 
    3) CORE_PKGS="$CORE_PKGS refind" ;;
    4) CORE_PKGS="$CORE_PKGS limine" ;;
esac

# =====================================================================
#              REPOSITORY INITIALIZATION & FAILSAFE INJECTOR
# =====================================================================
update_status "PROGRESS: Initializing package databases..."

if [ "$INSTALL_MODE" = "1" ]; then
    echo "[INFO] Syncing Arch & Chaotic-AUR package databases..."
    timedatectl set-ntp true 2>/dev/null || true
    
    if command -v reflector &> /dev/null; then
        reflector --latest 10 --sort rate --save /etc/pacman.d/mirrorlist &>/dev/null || echo 'Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch' > /etc/pacman.d/mirrorlist
    else 
        echo 'Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch' > /etc/pacman.d/mirrorlist
    fi

    pacman-key --init >/dev/null 2>&1 || true
    pacman-key --populate archlinux >/dev/null 2>&1 || true
    pacman-key --recv-key 3056513887B78AEB --keyserver hkps://keyserver.ubuntu.com >/dev/null 2>&1 || true
    pacman-key --lsign-key 3056513887B78AEB >/dev/null 2>&1 || true
    pacman -U 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst' 'https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst' --noconfirm >/dev/null 2>&1 || true

    if ! grep -q "chaotic-aur" /etc/pacman.conf; then
        echo -e "\n[chaotic-aur]\nSigLevel = Optional TrustAll\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> /etc/pacman.conf
    fi

    pacman -Sy --noconfirm >/dev/null 2>&1 || true
fi

if [ "$INSTALL_MODE" = "1" ]; then
    update_status "PROGRESS: Verifying package integrity against Arch mirrors..."
else
    update_status "PROGRESS: Verifying offline cache integrity..."
fi

VALIDATED_PACKAGES=()
for pkg in $CORE_PKGS; do
    if [ "$INSTALL_MODE" = "2" ]; then
        if ls "$ISO_CACHE"/${pkg}-*.pkg.tar.zst >/dev/null 2>&1 || ls "$ISO_CACHE"/${pkg}-*.pkg.tar.xz >/dev/null 2>&1; then
            VALIDATED_PACKAGES+=("$pkg")
        else
            echo "[WARNING] Offline cache missing '$pkg'. Skipping optional package."
        fi
    else
        if pacman -Si "$pkg" &> /dev/null || pacman -Sg "$pkg" &> /dev/null; then
            VALIDATED_PACKAGES+=("$pkg")
        else
            echo "[WARNING] Package '$pkg' is missing or renamed. Skipping."
        fi
    fi
done

# =====================================================================
#              INSTALLATION EXECUTION
# =====================================================================
clear
update_status "PROGRESS: Installing Base System (Pacstrap)..."
sed -i 's/^#Color/Color\nILoveCandy/' /etc/pacman.conf 2>/dev/null || true
sed -i 's/^#ParallelDownloads.*/ParallelDownloads = 10/' /etc/pacman.conf 2>/dev/null || true

echo "STARTING: Running pacstrap (Installing base system)..."

if [ "$INSTALL_MODE" = "2" ]; then
    cat << EOF > /tmp/offline-pacman.conf
[options]
Architecture = auto
SigLevel = Optional TrustAll
[kestrel-offline]
SigLevel = Optional TrustAll
Server = file://$ISO_CACHE/
EOF
    mkdir -p "$TARGET/var/cache/pacman/pkg"
    cp -n "$ISO_CACHE"/* "$TARGET/var/cache/pacman/pkg/" 2>/dev/null || true
    
    echo "[INFO] Executing Offline Pacstrap Phase..."
    pacstrap -C /tmp/offline-pacman.conf -K "$TARGET" --noconfirm "${VALIDATED_PACKAGES[@]}"
    
    cp /etc/pacman.conf "$TARGET/etc/pacman.conf"
else
    trap - ERR 
    DOWNLOAD_SUCCESS=0
    while [ "$DOWNLOAD_SUCCESS" -eq 0 ]; do
        rm -f "$TARGET/var/lib/pacman/db.lck" 2>/dev/null || true
        
        echo "[INFO] Executing Online Pacstrap Phase..."
        if pacstrap -K "$TARGET" --noconfirm "${VALIDATED_PACKAGES[@]}"; then 
            DOWNLOAD_SUCCESS=1; 
        else 
            update_status "WARNING: Pacstrap failed (mirror timeout). Retrying in 5 seconds..."
            echo "[WARNING] Pacstrap failed. Retrying in 5 seconds..."
            sleep 5
        fi
    done
    trap 'error_handler $? $LINENO' ERR 
fi

mkdir -p "$TARGET/etc/pacman.d"
if ! grep -q "chaotic-aur" "$TARGET/etc/pacman.conf"; then
    echo -e "\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist" >> "$TARGET/etc/pacman.conf"
fi
genfstab -U "$TARGET" >> "$TARGET/etc/fstab"

# =====================================================================
#              CHROOT SYSTEM CONFIGURATION & TARGET INITIALIZATION
# =====================================================================
update_status "PROGRESS: Configuring Base System and Chroot..."

if ! arch-chroot "$TARGET" id "$username" &>/dev/null; then
    arch-chroot "$TARGET" useradd -m -G wheel -s /bin/bash "$username"
else
    echo "[INFO] User $username already exists. Updating groups and shell..."
    arch-chroot "$TARGET" usermod -aG wheel -s /bin/bash "$username"
fi

printf '%s:%s\n' "$username" "$user_password" | arch-chroot "$TARGET" chpasswd
printf '%s:%s\n' "root" "$root_password" | arch-chroot "$TARGET" chpasswd
printf '%s\n' "$system_hostname" > "$TARGET/etc/hostname"
echo -e "127.0.0.1 localhost\n::1 localhost\n127.0.1.1 ${system_hostname}.localdomain ${system_hostname}" > "$TARGET/etc/hosts"

mkdir -p "$TARGET/etc/sudoers.d"
echo "%wheel ALL=(ALL:ALL) ALL" > "$TARGET/etc/sudoers.d/wheel"
chmod 440 "$TARGET/etc/sudoers.d/wheel"

echo "en_US.UTF-8 UTF-8" > "$TARGET/etc/locale.gen"
arch-chroot "$TARGET" locale-gen
echo "LANG=en_US.UTF-8" > "$TARGET/etc/locale.conf"

LIVE_KEYMAP=$(localectl status 2>/dev/null | grep "VC Keymap" | awk '{print $3}' || true)
[ -z "$LIVE_KEYMAP" ] && LIVE_KEYMAP="us"
printf 'KEYMAP=%s\n' "$LIVE_KEYMAP" > "$TARGET/etc/vconsole.conf"
arch-chroot "$TARGET" ln -sf /usr/share/zoneinfo/UTC /etc/localtime
arch-chroot "$TARGET" hwclock --systohc

if [ "$INSTALL_MODE" = "1" ]; then
    mkdir -p "$TARGET/var/lib/iwd" "$TARGET/etc/NetworkManager/conf.d"
    cp -r /var/lib/iwd/* "$TARGET/var/lib/iwd/" 2>/dev/null || true
    echo -e "[device]\nwifi.backend=iwd" > "$TARGET/etc/NetworkManager/conf.d/wifi_backend.conf"
fi

arch-chroot "$TARGET" systemctl enable ${DISPLAY_MANAGER}.service || true
arch-chroot "$TARGET" systemctl enable NetworkManager.service iwd.service bluetooth.service systemd-timesyncd.service || true

if [ -f "$TARGET/usr/lib/systemd/system/scx.service" ]; then
    arch-chroot "$TARGET" systemctl enable scx.service || true
fi
if [ -f "$TARGET/usr/lib/systemd/system/switcheroo-control.service" ]; then
    arch-chroot "$TARGET" systemctl enable switcheroo-control.service || true
fi

arch-chroot "$TARGET" systemctl mask systemd-time-wait-sync.service

mkdir -p "$TARGET/etc/bluetooth"
echo -e "[Policy]\nAutoEnable=true" > "$TARGET/etc/bluetooth/main.conf"

if [ "$HAS_NVIDIA" -eq 1 ] && [ "$HAS_INTEGRATED" -eq 1 ]; then
    echo "[INFO] Injecting hardware hybrid graphics modules settings..."
    mkdir -p "$TARGET/etc/modprobe.d"
    echo "options nvidia \"NVreg_DynamicPowerManagement=0x02\"" > "$TARGET/etc/modprobe.d/nvidia.conf"
    
    mkdir -p "$TARGET/etc/udev/rules.d"
    cat << 'EOF' > "$TARGET/etc/udev/rules.d/80-nvidia-pm.rules"
ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x030000", ATTR{power/control}="auto"
ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x040300", ATTR{power/control}="auto"
EOF
fi

cat << 'EOF' >> "$TARGET/home/$username/.bashrc"
alias ls='ls --color=auto'
alias grep='grep --color=auto'
alias pacman='sudo pacman --color auto'
alias update='sudo pacman -Syu'
PS1='\[\e[1;36m\]\u@\h\[\e[0m\]:\[\e[1;34m\]\w\[\e[0m\]\$ '
EOF
arch-chroot "$TARGET" chown -R "$username:$username" "/home/$username"

if [[ "$PERF_CHOICE" =~ ^[Yy]$ || -z "$PERF_CHOICE" ]]; then
    arch-chroot "$TARGET" systemctl enable earlyoom.service || true
    sed -i 's/#MAKEFLAGS="-j2"/MAKEFLAGS="-j$(nproc)"/' "$TARGET/etc/makepkg.conf"
    sed -i 's/^COMPRESSZST=.*/COMPRESSZST=(zstd -c -T0 -)/' "$TARGET/etc/makepkg.conf"
    sed -i 's/^#Color/Color\nILoveCandy/' "$TARGET/etc/pacman.conf" 2>/dev/null || true
    sed -i 's/^#ParallelDownloads.*/ParallelDownloads = 10/' "$TARGET/etc/pacman.conf" 2>/dev/null || true
    echo -e "[zram0]\nzram-size = ram / 2\ncompression-algorithm = zstd" > "$TARGET/etc/systemd/zram-generator.conf"
    if [ "$(lsblk -nd -o ROTA "$TARGET_DRIVE" | head -n 1)" = "0" ]; then arch-chroot "$TARGET" systemctl enable fstrim.timer || true; fi
fi

# =====================================================================
#              CHROOT PROVISIONING: BOOT MANAGERS DEPLOYMENT
# =====================================================================
update_status "PROGRESS: Installing Bootloader Framework..."
echo "STARTING: Installing and deploying selected boot loader framework..."
ROOT_UUID=$(blkid -s UUID -o value "$(lsblk -ln -p -o NAME "$TARGET_DRIVE" | grep -E "^${TARGET_DRIVE}${PART_PREFIX}[0-9]+" | sort -V | tail -n 1)")

NVIDIA_CMDLINE=""
if [ "$HAS_NVIDIA" -eq 1 ]; then
    NVIDIA_CMDLINE=" nvidia-drm.modeset=1"
fi

case $BOOT_CHOICE in
    1)
        # GRUB (Fully BIOS/UEFI Compatible Native)
        echo "GRUB_DISABLE_OS_PROBER=$GRUB_OS_PROBER" >> "$TARGET/etc/default/grub"
        sed -i "s/^GRUB_CMDLINE_LINUX_DEFAULT=.*/GRUB_CMDLINE_LINUX_DEFAULT=\"nowatchdog zswap.enabled=0 quiet splash mitigations=off${NVIDIA_CMDLINE}\"/" "$TARGET/etc/default/grub"
        if [ -d "/sys/firmware/efi" ]; then
            arch-chroot "$TARGET" grub-install --target=x86_64-efi --efi-directory="$EFI_DIR" --bootloader-id=KestrelArch --recheck
        else
            arch-chroot "$TARGET" grub-install --target=i386-pc "$TARGET_DRIVE" --recheck
        fi
        arch-chroot "$TARGET" grub-mkconfig -o /boot/grub/grub.cfg
        ;;
    2)
        # systemd-boot (UEFI Only)
        arch-chroot "$TARGET" bootctl install
        mkdir -p "$TARGET/boot/loader/entries"
        echo -e "default arch.conf\ntimeout 5" > "$TARGET/boot/loader/loader.conf"
        
        {
            echo "title Kestrel Arch"
            echo "linux /$VMLINUZ"
            [ -n "$UCODE_IMG" ] && echo "initrd /${UCODE_IMG}"
            echo "initrd /$INITRAMFS"
            echo "options root=UUID=${ROOT_UUID} rw nowatchdog zswap.enabled=0 quiet splash mitigations=off${NVIDIA_CMDLINE}"
        } > "$TARGET/boot/loader/entries/arch.conf"
        ;;
    3)
        # rEFInd (UEFI Only)
        arch-chroot "$TARGET" refind-install
        UCODE_STR=""
        [ -n "$UCODE_IMG" ] && UCODE_STR="initrd=${UCODE_IMG} "
        echo "\"Boot using default options\" \"root=UUID=${ROOT_UUID} rw ${UCODE_STR}initrd=${INITRAMFS} nowatchdog zswap.enabled=0 quiet splash mitigations=off${NVIDIA_CMDLINE}\"" > "$TARGET/boot/refind_linux.conf"
        ;;
    4)
        # LIMINE (Universal UEFI/BIOS with Custom Multi-Boot Prober)
        cat << EOF > "$TARGET/boot/limine.conf"
timeout: 5
default_entry: 1

/Kestrel Arch
    protocol: linux
    path: boot():/$VMLINUZ
EOF

        if [ -n "$UCODE_IMG" ]; then
            echo "    module_path: boot():/${UCODE_IMG}" >> "$TARGET/boot/limine.conf"
        fi

        cat << EOF >> "$TARGET/boot/limine.conf"
    module_path: boot():/$INITRAMFS
    cmdline: root=UUID=${ROOT_UUID} rw nowatchdog zswap.enabled=0 quiet splash mitigations=off${NVIDIA_CMDLINE}
EOF

        if [ -d "/sys/firmware/efi" ]; then
            # --- UEFI LIMINE DEPLOYMENT & CHAINLOADING ---
            mkdir -p "$TARGET$EFI_DIR/EFI/BOOT"
            cp "$TARGET/usr/share/limine/BOOTX64.EFI" "$TARGET$EFI_DIR/EFI/BOOT/BOOTX64.EFI"
            
            if [ "$GRUB_OS_PROBER" = "true" ]; then
                update_status "PROGRESS: Scanning EFI for other Operating Systems..."
                echo "[INFO] Searching for existing UEFI bootloaders..."
                
                for efi_file in $(find "$TARGET$EFI_DIR/EFI" -iname "*.efi" -type f 2>/dev/null || true); do
                    rel_path=$(echo "$efi_file" | sed "s|$TARGET$EFI_DIR||")
                    if echo "$rel_path" | grep -iqE "bootx64.efi|systemd-boot|limine"; then continue; fi
                    os_name=$(echo "$rel_path" | awk -F'/' '{print $3}')
                    
                    echo "[INFO] Found OS at: $rel_path. Adding to Limine."
                    echo -e "\n/$os_name (Chainload)\n    protocol: efi\n    path: boot():$rel_path" >> "$TARGET/boot/limine.conf"
                done
            fi
        else
            # --- LEGACY BIOS LIMINE DEPLOYMENT ---
            update_status "PROGRESS: Executing Limine BIOS configuration..."
            
            # BULLETPROOF FIX: Use standard native bash file checks (Avoids chroot environment parsing failures)
            mkdir -p "$TARGET/boot/limine"
            if [ -f "$TARGET/usr/share/limine/limine-bios.sys" ]; then
                cp "$TARGET/usr/share/limine/limine-bios.sys" "$TARGET/boot/"
                cp "$TARGET/usr/share/limine/limine-bios.sys" "$TARGET/boot/limine/"
            elif [ -f "$TARGET/usr/share/limine/limine.sys" ]; then
                cp "$TARGET/usr/share/limine/limine.sys" "$TARGET/boot/"
                cp "$TARGET/usr/share/limine/limine.sys" "$TARGET/boot/limine/"
            else
                echo "[WARNING] Limine BIOS system file not found! Bootloader may fail."
            fi
            
            arch-chroot "$TARGET" limine bios-install "$TARGET_DRIVE" || true
            
            if [ "$GRUB_OS_PROBER" = "true" ]; then
                if arch-chroot "$TARGET" fdisk -l "$TARGET_DRIVE" | grep -q "HPFS/NTFS"; then
                    echo "[INFO] NTFS Partition detected on BIOS system. Adding chainload entry..."
                    echo -e "\n/Windows (BIOS)\n    protocol: chainload\n    drive: 1\n    partition: 1" >> "$TARGET/boot/limine.conf"
                fi
            fi
        fi
        ;;
esac

update_status "COMPLETE"
echo "=========================================================="
echo "   KESTREL ARCH DEPLOYED! REBOOTING IN 5 SECONDS...       "
echo "=========================================================="
sleep 5

eject /dev/sr0 || true

if [ "$NON_INTERACTIVE" = "1" ]; then
    trap - ERR; umount -R "$TARGET" 2>/dev/null || true; exit 0
fi

trap - ERR; umount -R "$TARGET" 2>/dev/null || true; reboot || true
