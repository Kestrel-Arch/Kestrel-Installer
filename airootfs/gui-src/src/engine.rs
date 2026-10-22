use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;
use sysinfo::System;

#[cfg(target_os = "linux")]
use nix::mount::{mount, umount2, MntFlags, MsFlags};
#[cfg(target_os = "linux")]
use nix::unistd::{chdir, chroot, sync};
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

#[derive(Debug, Clone)]
pub enum TelemetryEvent {
    Progress(f32),
    Status(String),
    Log(String),
    Completed,
    Fault(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallMode {
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionStrategy {
    WipeDisk,
    ReplacePartition,
    ManualGParted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootloaderType {
    Grub,
    SystemdBoot,
    Refind,
    Limine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareTopology {
    pub uarch_level: String,
    pub cpu_vendor: String,
    pub cpu_threads: usize,
    pub total_ram_mb: u64,
    pub is_uefi: bool,
    pub is_vm: bool,
    pub is_laptop: bool,
    pub is_rotational: bool,
    pub io_scheduler: String,
    pub has_nvidia: bool,
    pub has_amd_gpu: bool,
    pub has_intel_gpu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentManifest {
    pub target_disk: String,
    pub mode: InstallMode,
    pub strategy: PartitionStrategy,
    pub requested_filesystem: String,
    pub replace_partition: String,
    pub manual_root_partition: String,
    pub manual_efi_partition: String,
    pub hostname: String,
    pub username: String,
    pub user_password: String,
    pub root_password: String,
    pub timezone: String,
    pub locale: String,
    pub keymap: String,
    pub desktop_index: u8,
    pub browser_index: u8,
    pub bootloader: BootloaderType,
    pub hyper_performance: bool,
    pub hardware: HardwareTopology,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairManifest {
    pub target_partition: String,
    pub os_id: String,
    pub kernel_pkg: String,
    pub bootloader: BootloaderType,
    pub efi_partition: String,
    pub is_uefi: bool,
}

impl DeploymentManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.target_disk.is_empty() || !self.target_disk.starts_with("/dev/") {
            return Err(format!("Invalid target disk path: '{}'", self.target_disk));
        }
        if self.hostname.trim().is_empty() {
            return Err("Hostname cannot be empty.".into());
        }
        if self.username.trim().is_empty() {
            return Err("Username cannot be empty.".into());
        }
        if self.user_password.is_empty() {
            return Err("User password cannot be empty.".into());
        }
        match self.strategy {
            PartitionStrategy::WipeDisk => {}
            PartitionStrategy::ReplacePartition => {
                if !self.replace_partition.starts_with("/dev/") {
                    return Err("Replace mode requires a valid target partition.".into());
                }
            }
            PartitionStrategy::ManualGParted => {
                if !self.manual_root_partition.starts_with("/dev/") {
                    return Err("Manual mode requires a valid Root (/) partition.".into());
                }
                if !self.manual_efi_partition.is_empty()
                    && self.manual_root_partition == self.manual_efi_partition
                    {
                        return Err("Root and EFI partitions cannot be the same device.".into());
                    }
            }
        }
        if !self.hardware.is_uefi
            && matches!(
                self.bootloader,
                BootloaderType::SystemdBoot | BootloaderType::Refind
            )
            {
                return Err("systemd-boot and rEFInd require UEFI firmware.".into());
            }
            Ok(())
    }
}

pub struct TransactionalGuard {
    target_root: PathBuf,
    mounted_paths: Vec<PathBuf>,
}

impl TransactionalGuard {
    pub fn new(target_root: &str) -> Self {
        Self {
            target_root: PathBuf::from(target_root),
            mounted_paths: Vec::new(),
        }
    }

    pub fn mount_fs(
        &mut self,
        source: &str,
        target: &Path,
        fstype: Option<&str>,
        options: Option<&str>,
    ) -> Result<(), String> {
        fs::create_dir_all(target)
        .map_err(|e| format!("Failed creating mountpoint {}: {}", target.display(), e))?;

        #[cfg(target_os = "linux")]
        {
            let mut flags = MsFlags::empty();
            if options == Some("bind") {
                flags |= MsFlags::MS_BIND;
            }
            let data = if options == Some("bind") { None } else { options };

            if mount(Some(source), target, fstype, flags, data).is_err() {
                let mut cmd = Command::new("mount");
                if let Some(ft) = fstype {
                    cmd.arg("-t").arg(ft);
                }
                if let Some(opts) = options {
                    cmd.arg("-o").arg(opts);
                }
                let status = cmd
                .arg(source)
                .arg(target)
                .status()
                .map_err(|e| format!("Failed spawning mount for {}: {}", source, e))?;
                if !status.success() {
                    return Err(format!("Mount failed: {} -> {}", source, target.display()));
                }
            }
        }
        self.mounted_paths.push(target.to_path_buf());
        Ok(())
    }

    pub fn mount_chroot_api_vfs(&mut self) -> Result<(), String> {
        let root = self.target_root.clone();
        self.mount_fs("proc", &root.join("proc"), Some("proc"), None)?;
        self.mount_fs("sysfs", &root.join("sys"), Some("sysfs"), None)?;
        self.mount_fs("/dev", &root.join("dev"), None, Some("bind"))?;
        self.mount_fs("devpts", &root.join("dev/pts"), Some("devpts"), None)?;
        self.mount_fs("/run", &root.join("run"), None, Some("bind"))?;

        if Path::new("/sys/firmware/efi/efivars").exists() {
            let _ = self.mount_fs(
                "efivarfs",
                &root.join("sys/firmware/efi/efivars"),
                                  Some("efivarfs"),
                                  None,
            );
        }
        Ok(())
    }

    pub fn cleanup_all(&mut self) {
        #[cfg(target_os = "linux")]
        sync();

        let _ = Command::new("swapoff").arg("-a").status();

        while let Some(path) = self.mounted_paths.pop() {
            #[cfg(target_os = "linux")]
            {
                if umount2(&path, MntFlags::MNT_DETACH).is_err() {
                    let _ = Command::new("umount").arg("-R").arg("-l").arg(&path).status();
                }
            }
        }

        #[cfg(target_os = "linux")]
        let _ = umount2(&self.target_root, MntFlags::MNT_DETACH);
        let _ = Command::new("umount")
        .arg("-R")
        .arg("-l")
        .arg(&self.target_root)
        .status();
    }
}

impl Drop for TransactionalGuard {
    fn drop(&mut self) {
        self.cleanup_all();
    }
}

pub fn probe_hardware_topology(disk_path: &str) -> HardwareTopology {
    let mut sys = System::new_all();
    sys.refresh_all();
    let total_ram_mb = sys.total_memory() / (1024 * 1024);

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let uarch_level = if std::arch::is_x86_feature_detected!("avx512f") {
        "x86-64-v4".to_string()
    } else if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("bmi2") {
        "x86-64-v3".to_string()
    } else if std::arch::is_x86_feature_detected!("sse4.2") {
        "x86-64-v2".to_string()
    } else {
        "x86-64-v1".to_string()
    };
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    let uarch_level = "generic".to_string();

    let cpu_info = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let cpu_vendor = if cpu_info.contains("AuthenticAMD") {
        "AMD".to_string()
    } else if cpu_info.contains("GenuineIntel") {
        "Intel".to_string()
    } else {
        "Generic".to_string()
    };

    let dev_name = disk_path.trim_start_matches("/dev/");
    let rot_path = format!("/sys/block/{}/queue/rotational", dev_name);
    let is_rotational = fs::read_to_string(&rot_path)
    .map(|s| s.trim() == "1")
    .unwrap_or(false);

    let io_scheduler = if dev_name.starts_with("nvme") {
        "none".to_string()
    } else if is_rotational {
        "bfq".to_string()
    } else {
        "mq-deadline".to_string()
    };

    let is_laptop = fs::read_dir("/sys/class/power_supply")
    .map(|entries| {
        entries
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with("BAT"))
    })
    .unwrap_or(false);

    let is_uefi = Path::new("/sys/firmware/efi").exists();
    let is_vm = Command::new("systemd-detect-virt")
    .arg("-q")
    .status()
    .map(|s| s.success())
    .unwrap_or(false);

    let mut has_nvidia = false;
    let mut has_amd_gpu = false;
    let mut has_intel_gpu = false;

    if let Ok(pci_entries) = fs::read_dir("/sys/bus/pci/devices") {
        for entry in pci_entries.filter_map(|e| e.ok()) {
            let cls = fs::read_to_string(entry.path().join("class")).unwrap_or_default();
            let vnd = fs::read_to_string(entry.path().join("vendor")).unwrap_or_default();
            if cls.trim().starts_with("0x03") {
                match vnd.trim() {
                    "0x10de" => has_nvidia = true,
                    "0x1002" => has_amd_gpu = true,
                    "0x8086" => has_intel_gpu = true,
                    _ => {}
                }
            }
        }
    }

    let cpu_threads = thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(4);

    HardwareTopology {
        uarch_level,
        cpu_vendor,
        cpu_threads,
        total_ram_mb,
        is_uefi,
        is_vm,
        is_laptop,
        is_rotational,
        io_scheduler,
        has_nvidia,
        has_amd_gpu,
        has_intel_gpu,
    }
}

pub fn detect_partition_fstype(part_path: &str) -> String {
    if let Ok(out) = Command::new("blkid")
        .args(["-s", "TYPE", "-o", "value", part_path])
        .output()
        {
            let fs = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
            if !fs.is_empty() {
                return fs;
            }
        }
        if let Ok(out) = Command::new("lsblk")
            .args(["-no", "FSTYPE", part_path])
            .output()
            {
                let fs = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
                if !fs.is_empty() {
                    return fs;
                }
            }
            String::new()
}

pub fn get_partition_uuid(part_path: &str) -> Result<String, String> {
    let out = Command::new("blkid")
    .args(["-s", "UUID", "-o", "value", part_path])
    .output()
    .map_err(|e| format!("Failed executing blkid on {}: {}", part_path, e))?;
    let uuid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if uuid.is_empty() {
        return Err(format!("Could not resolve UUID for {}", part_path));
    }
    Ok(uuid)
}

fn send_log(tx: &Sender<TelemetryEvent>, msg: impl Into<String>) {
    let s = msg.into();
    let _ = tx.send(TelemetryEvent::Status(s.clone()));
    let _ = tx.send(TelemetryEvent::Log(format!("> {}", s)));
}

fn run_host_cmd(
    tx: &Sender<TelemetryEvent>,
    program: &str,
    args: &[&str],
) -> Result<(), String> {
    let mut child = Command::new(program)
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| format!("Failed spawning {}: {}", program, e))?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let _ = tx.send(TelemetryEvent::Log(format!("> {}", trimmed)));
            }
        }
    }

    let status = child
    .wait()
    .map_err(|e| format!("Failed waiting on {}: {}", program, e))?;

    if !status.success() {
        return Err(format!(
            "Command `{} {}` exited with code {}",
            program,
            args.join(" "),
                           status
        ));
    }
    Ok(())
}

fn run_in_chroot(
    tx: &Sender<TelemetryEvent>,
    root_dir: &Path,
    program: &str,
    args: &[&str],
    stdin_data: Option<&str>,
) -> Result<(), String> {
    let root_owned = root_dir.to_path_buf();
    let mut cmd = Command::new(program);
    cmd.args(args)
    .env(
        "PATH",
         "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/bin/site_perl:/usr/bin/vendor_perl:/usr/bin/core_perl",
    )
    .env("HOME", "/root")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    if stdin_data.is_some() {
        cmd.stdin(Stdio::piped());
    }

    #[cfg(target_os = "linux")]
    unsafe {
        cmd.pre_exec(move || {
            chroot(&root_owned).map_err(|e| io::Error::from_raw_os_error(e as i32))?;
            chdir("/").map_err(|e| io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }

    let mut child = cmd
    .spawn()
    .map_err(|e| format!("Failed spawning chroot process {}: {}", program, e))?;

    if let Some(input) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
            .write_all(input.as_bytes())
            .map_err(|e| format!("Failed writing stdin to {}: {}", program, e))?;
        }
    }

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let _ = tx.send(TelemetryEvent::Log(format!("> {}", trimmed)));
            }
        }
    }

    let status = child
    .wait()
    .map_err(|e| format!("Chroot command {} failed to wait: {}", program, e))?;

    if !status.success() {
        return Err(format!(
            "Chroot command `{} {}` failed with status {}",
            program,
            args.join(" "),
                           status
        ));
    }
    Ok(())
}

fn format_partition(
    tx: &Sender<TelemetryEvent>,
    part: &str,
    fstype: &str,
) -> Result<(), String> {
    let _ = Command::new("wipefs").args(["-a", part]).status();
    match fstype {
        "vfat" | "fat32" => {
            send_log(tx, format!("Formatting {} as FAT32...", part));
            run_host_cmd(tx, "mkfs.vfat", &["-F", "32", part])
        }
        "btrfs" => {
            send_log(tx, format!("Formatting {} as Btrfs...", part));
            run_host_cmd(tx, "mkfs.btrfs", &["-f", part])
        }
        "xfs" => {
            send_log(tx, format!("Formatting {} as XFS...", part));
            run_host_cmd(tx, "mkfs.xfs", &["-f", part])
        }
        _ => {
            send_log(tx, format!("Formatting {} as ext4...", part));
            run_host_cmd(
                tx,
                "mkfs.ext4",
                &["-O", "^orphan_file,^metadata_csum_seed", "-F", part],
            )
        }
    }
}

fn provision_storage(
    manifest: &DeploymentManifest,
    guard: &mut TransactionalGuard,
    tx: &Sender<TelemetryEvent>,
) -> Result<(String, String, String), String> {
    let target_mnt = Path::new("/mnt");
    let disk = &manifest.target_disk;
    let part_prefix = if disk
    .chars()
    .last()
    .map(|c| c.is_ascii_digit())
    .unwrap_or(false)
    {
        "p"
    } else {
        ""
    };

    let requested_fs = manifest.requested_filesystem.trim().to_lowercase();

    match manifest.strategy {
        PartitionStrategy::WipeDisk => {
            send_log(
                tx,
                format!("Wiping partition table on {} and building layout...", disk),
            );
            let _ = Command::new("wipefs").args(["-a", disk]).status();

            if manifest.hardware.is_uefi {
                run_host_cmd(tx, "parted", &["-s", disk, "mklabel", "gpt"])?;
                run_host_cmd(
                    tx,
                    "parted",
                    &["-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", "1MiB", "513MiB"],
                )?;
                run_host_cmd(tx, "parted", &["-s", disk, "set", "1", "esp", "on"])?;
                run_host_cmd(
                    tx,
                    "parted",
                    &[
                        "-s",
                        "-a",
                        "optimal",
                        disk,
                        "mkpart",
                        "primary",
                        &requested_fs,
                        "513MiB",
                        "100%",
                    ],
                )?;
            } else {
                run_host_cmd(tx, "parted", &["-s", disk, "mklabel", "msdos"])?;
                run_host_cmd(
                    tx,
                    "parted",
                    &["-s", "-a", "optimal", disk, "mkpart", "primary", "fat32", "2MiB", "514MiB"],
                )?;
                run_host_cmd(tx, "parted", &["-s", disk, "set", "1", "boot", "on"])?;
                run_host_cmd(
                    tx,
                    "parted",
                    &[
                        "-s",
                        "-a",
                        "optimal",
                        disk,
                        "mkpart",
                        "primary",
                        &requested_fs,
                        "514MiB",
                        "100%",
                    ],
                )?;
            }

            let _ = Command::new("partprobe").arg(disk).status();
            let _ = Command::new("udevadm").arg("settle").status();
            thread::sleep(Duration::from_secs(2));

            let boot_part = format!("{}{}1", disk, part_prefix);
            let root_part = format!("{}{}2", disk, part_prefix);

            format_partition(tx, &boot_part, "vfat")?;
            format_partition(tx, &root_part, &requested_fs)?;

            let root_opts = if requested_fs == "btrfs" {
                Some("noatime,compress=zstd:3")
            } else {
                None
            };
            guard.mount_fs(&root_part, target_mnt, Some(&requested_fs), root_opts)?;
            guard.mount_fs(&boot_part, &target_mnt.join("boot"), Some("vfat"), None)?;

            Ok((root_part, boot_part, requested_fs))
        }

        PartitionStrategy::ReplacePartition => {
            let root_part = manifest.replace_partition.clone();
            send_log(
                tx,
                format!("Replacing partition {} with {}...", root_part, requested_fs),
            );
            format_partition(tx, &root_part, &requested_fs)?;

            let root_opts = if requested_fs == "btrfs" {
                Some("noatime,compress=zstd:3")
            } else {
                None
            };
            guard.mount_fs(&root_part, target_mnt, Some(&requested_fs), root_opts)?;

            let efi_part = manifest.manual_efi_partition.clone();
            if !efi_part.is_empty() && efi_part.starts_with("/dev/") {
                guard.mount_fs(&efi_part, &target_mnt.join("boot"), None, None)?;
                if !manifest.hardware.is_uefi {
                    let part_num: String = efi_part
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                    if !part_num.is_empty() {
                        let _ = Command::new("parted")
                        .args(["-s", disk, "set", &part_num, "boot", "on"])
                        .status();
                    }
                }
            } else {
                fs::create_dir_all(target_mnt.join("boot")).ok();
            }

            Ok((root_part, efi_part, requested_fs))
        }

        PartitionStrategy::ManualGParted => {
            let root_part = manifest.manual_root_partition.clone();
            let efi_part = manifest.manual_efi_partition.clone();

            // GPARTED BTRFS FIX: Read the real superblock created in GParted!
            let existing_fs = detect_partition_fstype(&root_part);
            let active_fs = if matches!(existing_fs.as_str(), "btrfs" | "ext4" | "xfs" | "f2fs") {
                send_log(
                    tx,
                    format!(
                        "Detected existing '{}' filesystem on {} from GParted. Preserving format!",
                        existing_fs, root_part
                    ),
                );
                existing_fs
            } else {
                send_log(
                    tx,
                    format!(
                        "Partition {} is unformatted. Formatting as {}...",
                        root_part, requested_fs
                    ),
                );
                format_partition(tx, &root_part, &requested_fs)?;
                requested_fs
            };

            let root_opts = if active_fs == "btrfs" {
                Some("noatime,compress=zstd:3")
            } else {
                None
            };
            guard.mount_fs(&root_part, target_mnt, Some(&active_fs), root_opts)?;

            if !efi_part.is_empty() && efi_part.starts_with("/dev/") {
                let efi_fs = detect_partition_fstype(&efi_part);
                if efi_fs.is_empty() {
                    format_partition(tx, &efi_part, "vfat")?;
                }
                guard.mount_fs(&efi_part, &target_mnt.join("boot"), None, None)?;

                if !manifest.hardware.is_uefi {
                    let part_num: String = efi_part
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                    if !part_num.is_empty() {
                        let _ = Command::new("parted")
                        .args(["-s", disk, "set", &part_num, "boot", "on"])
                        .status();
                    }
                }
            } else {
                fs::create_dir_all(target_mnt.join("boot")).ok();
            }

            Ok((root_part, efi_part, active_fs))
        }
    }
}

fn build_package_list(
    manifest: &DeploymentManifest,
) -> (Vec<String>, &'static str, &'static str, &'static str) {
    let (kernel_pkg, kernel_headers, vmlinuz, initramfs) = if manifest.hardware.uarch_level
    == "x86-64-v3"
    || manifest.hardware.uarch_level == "x86-64-v4"
    {
        (
            "linux-cachyos",
         "linux-cachyos-headers",
         "vmlinuz-linux-cachyos",
         "initramfs-linux-cachyos.img",
        )
    } else {
        ("linux", "linux-headers", "vmlinuz-linux", "initramfs-linux.img")
    };

    let mut pkgs: Vec<String> = vec![
        "base",
        kernel_pkg,
        kernel_headers,
        "linux-firmware",
        "scx-scheds",
        "efibootmgr",
        "os-prober",
        "ntfs-3g",
        "networkmanager",
        "iwd",
        "bluez",
        "bluez-utils",
        "blueman",
        "pipewire",
        "pipewire-pulse",
        "wireplumber",
        "brightnessctl",
        "flatpak",
        "xorg-server",
        "sudo",
        "zram-generator",
        "earlyoom",
        "reflector",
        "ttf-dejavu",
        "ttf-liberation",
        "noto-fonts",
        "noto-fonts-emoji",
        "curl",
        "chaotic-keyring",
        "chaotic-mirrorlist",
        "parted",
        "btrfs-progs",
        "dosfstools",
        "e2fsprogs",
        "xfsprogs",
        "foot",
        "git",
        "stow",
        "qt5-wayland",
        "qt6-wayland",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    if manifest.hardware.cpu_vendor == "AMD" {
        pkgs.push("amd-ucode".into());
    } else if manifest.hardware.cpu_vendor == "Intel" {
        pkgs.push("intel-ucode".into());
    }

    if manifest.hardware.has_nvidia {
        pkgs.extend(["nvidia".into(), "nvidia-utils".into(), "nvidia-prime".into()]);
        if manifest.hardware.has_amd_gpu || manifest.hardware.has_intel_gpu {
            pkgs.push("switcheroo-control".into());
        }
    }
    if manifest.hardware.has_amd_gpu {
        pkgs.push("xf86-video-amdgpu".into());
    }
    if manifest.hardware.has_intel_gpu {
        pkgs.push("intel-media-driver".into());
    }
    if manifest.hardware.is_laptop {
        pkgs.extend([
            "sof-firmware".into(),
                    "alsa-ucm-conf".into(),
                    "power-profiles-daemon".into(),
        ]);
    }
    if manifest.hardware.is_vm {
        pkgs.extend([
            "virtualbox-guest-utils".into(),
                    "xf86-video-vmware".into(),
                    "xf86-video-qxl".into(),
                    "qemu-guest-agent".into(),
        ]);
    }

    match manifest.browser_index {
        1 => pkgs.push("zen-browser-bin".into()),
        2 => pkgs.push("librewolf".into()),
        3 => pkgs.push("firefox".into()),
        4 => pkgs.push("brave-bin".into()),
        _ => pkgs.push("falkon".into()),
    }

    let mut dm = "sddm";
    let de_pkgs: &[&str] = match manifest.desktop_index {
        1 => &[
            "hyprland",
            "waybar",
            "kitty",
            "rofi-wayland",
            "xdg-desktop-portal-hyprland",
            "polkit-kde-agent",
            "thunar",
            "gvfs",
            "sddm",
        ],
        2 => &[
            "plasma-desktop",
            "plasma-workspace",
            "plasma-nm",
            "power-profiles-daemon",
            "kscreen",
            "konsole",
            "dolphin",
            "ark",
            "kate",
            "spectacle",
            "sddm-kcm",
            "sddm",
        ],
        3 => &["xfce4", "xfce4-goodies", "xfce4-terminal", "sddm"],
        4 => {
            dm = "gdm";
            &["gnome", "gnome-tweaks", "gdm"]
        }
        5 => &[
            "sway",
            "swaybg",
            "swaylock",
            "swayidle",
            "waybar",
            "kitty",
            "rofi-wayland",
            "xdg-desktop-portal-wlr",
            "polkit-kde-agent",
            "thunar",
            "gvfs",
            "sddm",
        ],
        6 => &[
            "i3-wm",
            "i3status",
            "i3lock",
            "dmenu",
            "kitty",
            "picom",
            "feh",
            "polkit-gnome",
            "thunar",
            "gvfs",
            "sddm",
        ],
        7 => &["cinnamon", "nemo-fileroller", "gnome-terminal", "sddm"],
        8 => &[
            "niri",
            "waybar",
            "kitty",
            "rofi-wayland",
            "xdg-desktop-portal-gnome",
            "polkit-kde-agent",
            "thunar",
            "gvfs",
            "sddm",
        ],
        9 => &[
            "qtile",
            "kitty",
            "rofi-wayland",
            "xdg-desktop-portal-wlr",
            "polkit-kde-agent",
            "thunar",
            "gvfs",
            "sddm",
        ],
        10 => &[
            "wayfire",
            "wayfire-plugins-extra",
            "kitty",
            "rofi-wayland",
            "xdg-desktop-portal-wlr",
            "polkit-kde-agent",
            "thunar",
            "gvfs",
            "sddm",
        ],
        11 => &[
            "bspwm",
            "sxhkd",
            "kitty",
            "dmenu",
            "picom",
            "feh",
            "polkit-gnome",
            "thunar",
            "gvfs",
            "sddm",
        ],
        12 => &["budgie-desktop", "sddm"],
        13 => &["cosmic", "sddm"],
        14 => &["lxde-common", "lxsession", "openbox", "sddm"],
        15 => &["lxqt", "lxqt-session", "sddm"],
        16 => &["mate", "mate-extra", "sddm"],
        _ => &["openbox", "obconf", "tint2", "kitty", "dmenu", "feh", "sddm"],
    };

    for p in de_pkgs {
        pkgs.push((*p).to_string());
    }

    match manifest.bootloader {
        BootloaderType::Grub => pkgs.push("grub".into()),
        BootloaderType::SystemdBoot => {}
        BootloaderType::Refind => pkgs.push("refind".into()),
        BootloaderType::Limine => pkgs.push("limine".into()),
    }

    (pkgs, dm, vmlinuz, initramfs)
}

fn initialize_live_repos(tx: &Sender<TelemetryEvent>, mode: InstallMode) {
    if mode == InstallMode::Online {
        send_log(tx, "Syncing Arch & Chaotic-AUR package databases...");
        let _ = Command::new("timedatectl").args(["set-ntp", "true"]).status();
        if Command::new("reflector")
            .args([
                "--latest",
                "10",
                "--sort",
                "rate",
                "--save",
                "/etc/pacman.d/mirrorlist",
            ])
            .status()
            .map(|s| !s.success())
            .unwrap_or(true)
            {
                let _ = fs::write(
                    "/etc/pacman.d/mirrorlist",
                    "Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch\n",
                );
            }

            let _ = Command::new("pacman-key").arg("--init").status();
            let _ = Command::new("pacman-key").args(["--populate", "archlinux"]).status();
            let _ = Command::new("pacman-key")
            .args(["--recv-key", "3056513887B78AEB", "--keyserver", "hkps://keyserver.ubuntu.com"])
            .status();
            let _ = Command::new("pacman-key")
            .args(["--lsign-key", "3056513887B78AEB"])
            .status();
            let _ = Command::new("pacman")
            .args([
                "-U",
                "--noconfirm",
                "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst",
                "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst",
            ])
            .status();

            if let Ok(mut conf) = fs::read_to_string("/etc/pacman.conf") {
                if !conf.contains("[chaotic-aur]") {
                    conf.push_str("\n[chaotic-aur]\nSigLevel = Optional TrustAll\nInclude = /etc/pacman.d/chaotic-mirrorlist\n");
                    let _ = fs::write("/etc/pacman.conf", conf);
                }
            }
            let _ = Command::new("pacman").args(["-Sy", "--noconfirm"]).status();
    }
}

fn execute_pacstrap_stream(
    tx: &Sender<TelemetryEvent>,
    mode: InstallMode,
    packages: &[String],
) -> Result<(), String> {
    initialize_live_repos(tx, mode);
    let iso_cache = Path::new("/opt/offline_cache");

    let mut validated: Vec<String> = Vec::new();
    for pkg in packages {
        if mode == InstallMode::Offline {
            let has_pkg = fs::read_dir(iso_cache)
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with(&format!("{}-", pkg)) && name.contains(".pkg.tar.")
                })
            })
            .unwrap_or(false);
            if has_pkg {
                validated.push(pkg.clone());
            }
        } else {
            let ok = Command::new("pacman")
            .args(["-Si", pkg])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
            || Command::new("pacman")
            .args(["-Sg", pkg])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
            if ok {
                validated.push(pkg.clone());
            }
        }
    }

    if mode == InstallMode::Offline {
        let offline_conf = "[options]\nArchitecture = auto\nSigLevel = Optional TrustAll\n[kestrel-offline]\nSigLevel = Optional TrustAll\nServer = file:///opt/offline_cache/\n";
        fs::write("/tmp/offline-pacman.conf", offline_conf)
        .map_err(|e| format!("Failed writing offline pacman.conf: {}", e))?;

        let mut args = vec!["-C", "/tmp/offline-pacman.conf", "-K", "/mnt", "--noconfirm"];
        let pkg_refs: Vec<&str> = validated.iter().map(|s| s.as_str()).collect();
        args.extend(pkg_refs);

        run_pacstrap_with_progress(tx, &args)?;
        let _ = fs::copy("/etc/pacman.conf", "/mnt/etc/pacman.conf");
        Ok(())
    } else {
        for attempt in 1..=3 {
            let _ = fs::remove_file("/mnt/var/lib/pacman/db.lck");
            let mut args = vec!["-K", "/mnt", "--noconfirm"];
            let pkg_refs: Vec<&str> = validated.iter().map(|s| s.as_str()).collect();
            args.extend(pkg_refs);

            send_log(
                tx,
                format!("Executing online pacstrap transaction (Attempt {}/3)...", attempt),
            );
            if run_pacstrap_with_progress(tx, &args).is_ok() {
                return Ok(());
            }
            send_log(tx, "Mirror timeout detected. Refreshing mirrorlist...");
            let _ = Command::new("reflector")
            .args([
                "--latest",
                "10",
                "--protocol",
                "https",
                "--sort",
                "rate",
                "--save",
                "/etc/pacman.d/mirrorlist",
            ])
            .status();
            thread::sleep(Duration::from_secs(3));
        }
        Err("Online pacstrap failed after 3 attempts.".into())
    }
}

fn run_pacstrap_with_progress(
    tx: &Sender<TelemetryEvent>,
    args: &[&str],
) -> Result<(), String> {
    let mut child = Command::new("pacstrap")
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| format!("Failed spawning pacstrap: {}", e))?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let clean = line.trim();
            if clean.is_empty() {
                continue;
            }
            if clean.starts_with('(') && clean.contains('/') && clean.contains(')') {
                if let Some(bracket) = clean.split(')').next() {
                    let nums: String = bracket
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '/')
                    .collect();
                    let parts: Vec<&str> = nums.split('/').collect();
                    if parts.len() == 2 {
                        if let (Ok(cur), Ok(tot)) =
                            (parts[0].parse::<f32>(), parts[1].parse::<f32>())
                            {
                                if tot > 0.0 {
                                    let _ = tx.send(TelemetryEvent::Progress(0.25 + 0.50 * (cur / tot)));
                                }
                            }
                    }
                }
                let _ = tx.send(TelemetryEvent::Status(clean.to_string()));
            } else {
                let _ = tx.send(TelemetryEvent::Log(format!("> {}", clean)));
            }
        }
    }

    let status = child
    .wait()
    .map_err(|e| format!("Failed waiting on pacstrap: {}", e))?;
    if !status.success() {
        return Err(format!("pacstrap exited with status {}", status));
    }
    Ok(())
}

fn write_native_fstab(
    root_part: &str,
    boot_part: &str,
    root_fs: &str,
) -> Result<(), String> {
    let root_uuid = get_partition_uuid(root_part)?;
    let root_opts = if root_fs == "btrfs" {
        "rw,noatime,compress=zstd:3,space_cache=v2"
    } else {
        "rw,relatime"
    };

    let mut fstab = format!(
        "# /etc/fstab: static file system information.\n\
# Generated natively by Kestrel Arch Rust Engine\n\
UUID={}\t/\t{}\t{}\t0 1\n",
root_uuid, root_fs, root_opts
    );

    if !boot_part.is_empty() && boot_part.starts_with("/dev/") {
        if let Ok(boot_uuid) = get_partition_uuid(boot_part) {
            fstab.push_str(&format!(
                "UUID={}\t/boot\tvfat\trw,relatime,fmask=0022,dmask=0022,codepage=437,iocharset=ascii,shortname=mixed,utf8,errors=remount-ro\t0 2\n",
                boot_uuid
            ));
        }
    }

    fs::write("/mnt/etc/fstab", fstab)
    .map_err(|e| format!("Failed writing /mnt/etc/fstab: {}", e))
}

pub fn install_self_healing_alpm_guard(target_root: &Path) -> Result<(), String> {
    let bin_dir = target_root.join("usr/local/bin");
    let hook_dir = target_root.join("etc/pacman.d/hooks");
    fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&hook_dir).map_err(|e| e.to_string())?;

    let doctor_script = r#"#!/usr/bin/env bash
    set -euo pipefail
    if [[ $EUID -ne 0 ]]; then exec sudo bash "$0" "$@"; fi
        if [[ -f /var/lib/pacman/db.lck ]] && ! pgrep -x pacman >/dev/null; then
            rm -f /var/lib/pacman/db.lck
            fi
            for kver_dir in /usr/lib/modules/*; do
                [[ -d "$kver_dir" ]] || continue
                if [[ -f "${kver_dir}/pkgbase" ]]; then
                    pkgbase=$(cat "${kver_dir}/pkgbase")
                    if [[ ! -f "/boot/vmlinuz-${pkgbase}" ]] || [[ ! -s "/boot/initramfs-${pkgbase}.img" ]]; then
                        echo "[Kestrel Guard] Rebuilding missing boot image for ${pkgbase}..."
                        cp -f "${kver_dir}/vmlinuz" "/boot/vmlinuz-${pkgbase}" 2>/dev/null || true
                        mkinitcpio -p "${pkgbase}"
                        fi
                        fi
                        done
                        "#;

                        let doctor_path = bin_dir.join("kestrel-doctor");
                        fs::write(&doctor_path, doctor_script).map_err(|e| e.to_string())?;
                        #[cfg(target_os = "linux")]
                        fs::set_permissions(&doctor_path, fs::Permissions::from_mode(0o755))
                        .map_err(|e| e.to_string())?;

                        let hook_content = r#"[Trigger]
                        Operation = Install
                        Operation = Upgrade
                        Type = Path
                        Target = usr/lib/modules/*/vmlinuz

                        [Action]
                        Description = [Kestrel Guard] Verifying kernel & initramfs boot integrity...
                        When = PostTransaction
                        Exec = /usr/local/bin/kestrel-doctor
                        "#;
                        fs::write(hook_dir.join("99-kestrel-guard.hook"), hook_content).map_err(|e| e.to_string())?;
                        Ok(())
}

fn configure_target_system_in_chroot(
    manifest: &DeploymentManifest,
    display_manager: &str,
    tx: &Sender<TelemetryEvent>,
) -> Result<(), String> {
    let mnt = Path::new("/mnt");

    let kestrel_etc = mnt.join("etc/kestrel");
    fs::create_dir_all(&kestrel_etc).map_err(|e| e.to_string())?;
    if let Ok(toml_str) = toml::to_string_pretty(manifest) {
        let _ = fs::write(kestrel_etc.join("deployment.toml"), toml_str);
    }

    if let Ok(mut pconf) = fs::read_to_string(mnt.join("etc/pacman.conf")) {
        pconf = pconf.replace("#Color", "Color\nILoveCandy");
        pconf = pconf.replace("#ParallelDownloads = 5", "ParallelDownloads = 10");
        if !pconf.contains("[chaotic-aur]") {
            pconf.push_str("\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist\n");
        }
        let _ = fs::write(mnt.join("etc/pacman.conf"), pconf);
    }

    fs::write(mnt.join("etc/hostname"), format!("{}\n", manifest.hostname))
    .map_err(|e| e.to_string())?;
    let hosts = format!(
        "127.0.0.1\tlocalhost\n::1\t\tlocalhost\n127.0.1.1\t{}.localdomain\t{}\n",
        manifest.hostname, manifest.hostname
    );
    fs::write(mnt.join("etc/hosts"), hosts).map_err(|e| e.to_string())?;

    send_log(
        tx,
        format!(
            "Applying Timezone ({}), Locale ({}), and Keymap ({})...",
                manifest.timezone, manifest.locale, manifest.keymap
        ),
    );
    run_in_chroot(
        tx,
        mnt,
        "ln",
        &[
            "-sf",
            &format!("/usr/share/zoneinfo/{}", manifest.timezone),
                  "/etc/localtime",
        ],
        None,
    )?;
    let _ = run_in_chroot(tx, mnt, "hwclock", &["--systohc"], None);

    let locale_entry = if manifest.locale == "en_US.UTF-8" {
        "en_US.UTF-8 UTF-8\n".to_string()
    } else {
        format!("en_US.UTF-8 UTF-8\n{} UTF-8\n", manifest.locale)
    };
    fs::write(mnt.join("etc/locale.gen"), locale_entry).map_err(|e| e.to_string())?;
    run_in_chroot(tx, mnt, "locale-gen", &[], None)?;
    fs::write(
        mnt.join("etc/locale.conf"),
              format!("LANG={}\n", manifest.locale),
    )
    .map_err(|e| e.to_string())?;

    fs::write(
        mnt.join("etc/vconsole.conf"),
              format!("KEYMAP={}\n", manifest.keymap),
    )
    .map_err(|e| e.to_string())?;

    let x11_dir = mnt.join("etc/X11/xorg.conf.d");
    fs::create_dir_all(&x11_dir).ok();
    let x11_kbd = format!(
        "Section \"InputClass\"\n\
\tIdentifier \"system-keyboard\"\n\
\tMatchIsKeyboard \"on\"\n\
\tOption \"XkbLayout\" \"{}\"\n\
EndSection\n",
manifest.keymap
    );
    let _ = fs::write(x11_dir.join("00-keyboard.conf"), x11_kbd);

    send_log(
        tx,
        format!("Creating user '{}' and wheel sudoers policy...", manifest.username),
    );
    let _ = run_in_chroot(
        tx,
        mnt,
        "useradd",
        &["-m", "-G", "wheel", "-s", "/bin/bash", &manifest.username],
        None,
    );
    let chpasswd_payload = format!(
        "{}:{}\nroot:{}\n",
        manifest.username, manifest.user_password, manifest.root_password
    );
    run_in_chroot(tx, mnt, "chpasswd", &[], Some(&chpasswd_payload))?;

    let sudoers_dir = mnt.join("etc/sudoers.d");
    fs::create_dir_all(&sudoers_dir).map_err(|e| e.to_string())?;
    let wheel_file = sudoers_dir.join("wheel");
    fs::write(&wheel_file, "%wheel ALL=(ALL:ALL) ALL\n").map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    fs::set_permissions(&wheel_file, fs::Permissions::from_mode(0o440)).ok();

    let bashrc_path = mnt.join("home").join(&manifest.username).join(".bashrc");
    if let Ok(mut bashrc) = fs::read_to_string(&bashrc_path) {
        bashrc.push_str(
            "\nalias ls='ls --color=auto'\n\
alias grep='grep --color=auto'\n\
alias pacman='sudo pacman --color auto'\n\
alias update='sudo pacman -Syu'\n\
PS1='\\[\\e[1;36m\\]\\u@\\h\\[\\e[0m\\]:\\[\\e[1;34m\\]\\w\\[\\e[0m\\]\\$ '\n",
        );
        let _ = fs::write(&bashrc_path, bashrc);
    }
    let _ = run_in_chroot(
        tx,
        mnt,
        "chown",
        &[
            "-R",
            &format!("{}:{}", manifest.username, manifest.username),
                          &format!("/home/{}", manifest.username),
        ],
        None,
    );

    if manifest.mode == InstallMode::Online && Path::new("/var/lib/iwd").exists() {
        let target_iwd = mnt.join("var/lib/iwd");
        let nm_conf_dir = mnt.join("etc/NetworkManager/conf.d");
        fs::create_dir_all(&target_iwd).ok();
        fs::create_dir_all(&nm_conf_dir).ok();
        if let Ok(entries) = fs::read_dir("/var/lib/iwd") {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().is_file() {
                    let _ = fs::copy(entry.path(), target_iwd.join(entry.file_name()));
                }
            }
        }
        let _ = fs::write(
            nm_conf_dir.join("wifi_backend.conf"),
                          "[device]\nwifi.backend=iwd\n",
        );
    }

    let bt_dir = mnt.join("etc/bluetooth");
    fs::create_dir_all(&bt_dir).ok();
    let _ = fs::write(bt_dir.join("main.conf"), "[Policy]\nAutoEnable=true\n");

    let udev_dir = mnt.join("etc/udev/rules.d");
    fs::create_dir_all(&udev_dir).ok();
    let dev_name = manifest.target_disk.trim_start_matches("/dev/");
    let iosched_rule = format!(
        "ACTION==\"add|change\", KERNEL==\"{}\", ATTR{{queue/scheduler}}=\"{}\"\n",
        dev_name, manifest.hardware.io_scheduler
    );
    let _ = fs::write(udev_dir.join("60-kestrel-iosched.rules"), iosched_rule);

    if manifest.hardware.has_nvidia
        && (manifest.hardware.has_amd_gpu || manifest.hardware.has_intel_gpu)
        {
            let modprobe_dir = mnt.join("etc/modprobe.d");
            fs::create_dir_all(&modprobe_dir).ok();
            let _ = fs::write(
                modprobe_dir.join("nvidia.conf"),
                              "options nvidia \"NVreg_DynamicPowerManagement=0x02\"\n",
            );
            let nvidia_udev = "ACTION==\"add\", SUBSYSTEM==\"pci\", ATTR{vendor}==\"0x10de\", ATTR{class}==\"0x030000\", ATTR{power/control}=\"auto\"\n\
ACTION==\"add\", SUBSYSTEM==\"pci\", ATTR{vendor}==\"0x10de\", ATTR{class}==\"0x040300\", ATTR{power/control}=\"auto\"\n";
            let _ = fs::write(udev_dir.join("80-nvidia-pm.rules"), nvidia_udev);
        }

        if manifest.hyper_performance {
            let _ = fs::write(
                mnt.join("etc/systemd/zram-generator.conf"),
                              "[zram0]\nzram-size = ram / 2\ncompression-algorithm = zstd\n",
            );
            if let Ok(mut mkpkg) = fs::read_to_string(mnt.join("etc/makepkg.conf")) {
                mkpkg = mkpkg.replace(
                    "#MAKEFLAGS=\"-j2\"",
                    &format!("MAKEFLAGS=\"-j{}\"", manifest.hardware.cpu_threads),
                );
                mkpkg = mkpkg.replace(
                    "COMPRESSZST=(zstd -c -T0 --ultra -20 -)",
                                      "COMPRESSZST=(zstd -c -T0 -)",
                );
                let _ = fs::write(mnt.join("etc/makepkg.conf"), mkpkg);
            }
            let _ = run_in_chroot(tx, mnt, "systemctl", &["enable", "earlyoom.service"], None);
            if !manifest.hardware.is_rotational {
                let _ = run_in_chroot(tx, mnt, "systemctl", &["enable", "fstrim.timer"], None);
            }
        }

        install_self_healing_alpm_guard(mnt)?;

        for svc in [
            display_manager,
            "NetworkManager.service",
            "iwd.service",
            "bluetooth.service",
            "systemd-timesyncd.service",
        ] {
            let _ = run_in_chroot(tx, mnt, "systemctl", &["enable", svc], None);
        }

        if mnt.join("usr/lib/systemd/system/scx.service").exists() {
            let _ = run_in_chroot(tx, mnt, "systemctl", &["enable", "scx.service"], None);
        }
        if mnt
            .join("usr/lib/systemd/system/switcheroo-control.service")
            .exists()
            {
                let _ = run_in_chroot(
                    tx,
                    mnt,
                    "systemctl",
                    &["enable", "switcheroo-control.service"],
                    None,
                );
            }
            let _ = run_in_chroot(
                tx,
                mnt,
                "systemctl",
                &["mask", "systemd-time-wait-sync.service"],
                None,
            );

            Ok(())
}

fn deploy_bootloader(
    manifest: &DeploymentManifest,
    root_part: &str,
    vmlinuz: &str,
    initramfs: &str,
    tx: &Sender<TelemetryEvent>,
) -> Result<(), String> {
    let mnt = Path::new("/mnt");
    let root_uuid = get_partition_uuid(root_part)?;
    let ucode = if manifest.hardware.cpu_vendor == "AMD" {
        Some("amd-ucode.img")
    } else if manifest.hardware.cpu_vendor == "Intel" {
        Some("intel-ucode.img")
    } else {
        None
    };
    let nvidia_arg = if manifest.hardware.has_nvidia {
        " nvidia-drm.modeset=1"
    } else {
        ""
    };

    send_log(
        tx,
        format!("Deploying {:?} bootloader configuration...", manifest.bootloader),
    );

    match manifest.bootloader {
        BootloaderType::Grub => {
            let grub_default = mnt.join("etc/default/grub");
            let mut content = fs::read_to_string(&grub_default).unwrap_or_default();
            content.push_str("\nGRUB_DISABLE_OS_PROBER=false\n");
            content.push_str(&format!(
                "GRUB_CMDLINE_LINUX_DEFAULT=\"nowatchdog zswap.enabled=0 quiet splash mitigations=off{}\"\n",
                nvidia_arg
            ));
            fs::write(&grub_default, content).ok();

            if manifest.hardware.is_uefi {
                run_in_chroot(
                    tx,
                    mnt,
                    "grub-install",
                    &[
                        "--target=x86_64-efi",
                        "--efi-directory=/boot",
                        "--bootloader-id=KestrelArch",
                        "--recheck",
                    ],
                    None,
                )?;
            } else {
                run_in_chroot(
                    tx,
                    mnt,
                    "grub-install",
                    &["--target=i386-pc", &manifest.target_disk, "--recheck"],
                    None,
                )?;
            }
            run_in_chroot(tx, mnt, "grub-mkconfig", &["-o", "/boot/grub/grub.cfg"], None)?;
        }

        BootloaderType::SystemdBoot => {
            run_in_chroot(tx, mnt, "bootctl", &["install"], None)?;
            let entries_dir = mnt.join("boot/loader/entries");
            fs::create_dir_all(&entries_dir).map_err(|e| e.to_string())?;
            fs::write(
                mnt.join("boot/loader/loader.conf"),
                      "default arch.conf\ntimeout 5\nconsole-mode max\neditor no\n",
            )
            .map_err(|e| e.to_string())?;

            let mut entry = format!("title   Kestrel Arch\nlinux   /{}\n", vmlinuz);
            if let Some(u) = ucode {
                entry.push_str(&format!("initrd  /{}\n", u));
            }
            entry.push_str(&format!(
                "initrd  /{}\noptions root=UUID={} rw nowatchdog zswap.enabled=0 quiet splash mitigations=off{}\n",
                initramfs, root_uuid, nvidia_arg
            ));
            fs::write(entries_dir.join("arch.conf"), entry).map_err(|e| e.to_string())?;
        }

        BootloaderType::Refind => {
            run_in_chroot(tx, mnt, "refind-install", &[], None)?;
            let ucode_opt = ucode.map(|u| format!("initrd={} ", u)).unwrap_or_default();
            let conf = format!(
                "\"Boot using default options\" \"root=UUID={} rw {}initrd={} nowatchdog zswap.enabled=0 quiet splash mitigations=off{}\"\n",
                root_uuid, ucode_opt, initramfs, nvidia_arg
            );
            fs::write(mnt.join("boot/refind_linux.conf"), conf).map_err(|e| e.to_string())?;
        }

        BootloaderType::Limine => {
            let mut limine_cfg = format!(
                "timeout: 5\ndefault_entry: 1\n\n/Kestrel Arch\n    protocol: linux\n    path: boot():/{}\n",
                                         vmlinuz
            );
            if let Some(u) = ucode {
                limine_cfg.push_str(&format!("    module_path: boot():/{}\n", u));
            }
            limine_cfg.push_str(&format!(
                "    module_path: boot():/{}\n    cmdline: root=UUID={} rw nowatchdog zswap.enabled=0 quiet splash mitigations=off{}\n",
                                         initramfs, root_uuid, nvidia_arg
            ));

            if manifest.hardware.is_uefi {
                let efi_boot = mnt.join("boot/EFI/BOOT");
                fs::create_dir_all(&efi_boot).map_err(|e| e.to_string())?;
                fs::copy(
                    mnt.join("usr/share/limine/BOOTX64.EFI"),
                         efi_boot.join("BOOTX64.EFI"),
                )
                .map_err(|e| format!("Failed copying Limine EFI binary: {}", e))?;

                if let Ok(efi_dirs) = fs::read_dir(mnt.join("boot/EFI")) {
                    for dir in efi_dirs.filter_map(|d| d.ok()) {
                        let folder_name = dir.file_name().to_string_lossy().to_string();
                        if folder_name.eq_ignore_ascii_case("BOOT")
                            || folder_name.eq_ignore_ascii_case("systemd")
                            || folder_name.eq_ignore_ascii_case("limine")
                            {
                                continue;
                            }
                            if let Ok(files) = fs::read_dir(dir.path()) {
                                for f in files.filter_map(|f| f.ok()) {
                                    let fname = f.file_name().to_string_lossy().to_string();
                                    if fname.to_lowercase().ends_with(".efi") {
                                        limine_cfg.push_str(&format!(
                                            "\n/{} (Chainload)\n    protocol: efi\n    path: boot():/EFI/{}/{}\n",
                                                                     folder_name, folder_name, fname
                                        ));
                                    }
                                }
                            }
                    }
                }
            } else {
                let limine_dir = mnt.join("boot/limine");
                fs::create_dir_all(&limine_dir).ok();
                for candidate in ["limine-bios.sys", "limine.sys"] {
                    let src = mnt.join("usr/share/limine").join(candidate);
                    if src.exists() {
                        let _ = fs::copy(&src, mnt.join("boot").join(candidate));
                        let _ = fs::copy(&src, limine_dir.join(candidate));
                    }
                }
                let _ = run_in_chroot(
                    tx,
                    mnt,
                    "limine",
                    &["bios-install", &manifest.target_disk],
                    None,
                );
            }

            fs::write(mnt.join("boot/limine.conf"), limine_cfg).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

pub fn execute_deployment(manifest: DeploymentManifest, tx: Sender<TelemetryEvent>) {
    let run = || -> Result<(), String> {
        send_log(&tx, "Validating typed deployment manifest...");
        manifest.validate()?;
        let _ = tx.send(TelemetryEvent::Progress(0.05));

        let mut guard = TransactionalGuard::new("/mnt");
        guard.cleanup_all();

        let _ = tx.send(TelemetryEvent::Progress(0.10));
        let (root_part, boot_part, active_fs) = provision_storage(&manifest, &mut guard, &tx)?;

        let _ = tx.send(TelemetryEvent::Progress(0.20));
        let (pkgs, dm, vmlinuz, initramfs) = build_package_list(&manifest);
        execute_pacstrap_stream(&tx, manifest.mode, &pkgs)?;

        let _ = tx.send(TelemetryEvent::Progress(0.78));
        send_log(&tx, "Generating UUID-pinned /etc/fstab...");
        write_native_fstab(&root_part, &boot_part, &active_fs)?;

        send_log(&tx, "Binding kernel virtual filesystems for native chroot...");
        guard.mount_chroot_api_vfs()?;

        let _ = tx.send(TelemetryEvent::Progress(0.85));
        configure_target_system_in_chroot(&manifest, dm, &tx)?;

        let _ = tx.send(TelemetryEvent::Progress(0.92));
        deploy_bootloader(&manifest, &root_part, vmlinuz, initramfs, &tx)?;

        guard.cleanup_all();
        let _ = tx.send(TelemetryEvent::Progress(1.0));
        send_log(&tx, "Kestrel Arch deployment completed cleanly!");
        Ok(())
    };

    match run() {
        Ok(()) => {
            let _ = tx.send(TelemetryEvent::Completed);
        }
        Err(err) => {
            let _ = tx.send(TelemetryEvent::Log(format!(
                "> [TRANSACTIONAL ROLLBACK] Safely unmounted target volumes after error: {}",
                err
            )));
            let _ = tx.send(TelemetryEvent::Fault(err));
        }
    }
}

pub fn execute_repair(manifest: RepairManifest, tx: Sender<TelemetryEvent>) {
    let run = || -> Result<(), String> {
        let target_mnt = Path::new("/mnt/kestrel_repair");
        let mut guard = TransactionalGuard::new("/mnt/kestrel_repair");

        let _ = tx.send(TelemetryEvent::Progress(0.15));
        send_log(
            &tx,
            format!(
                "[1/5] Validating firmware and mounting {}...",
                manifest.target_partition
            ),
        );

        if !manifest.is_uefi
            && matches!(
                manifest.bootloader,
                BootloaderType::SystemdBoot | BootloaderType::Refind
            )
            {
                return Err("Selected bootloader requires UEFI firmware.".into());
            }

            let fstype = detect_partition_fstype(&manifest.target_partition);
        if fstype == "btrfs" {
            if guard
                .mount_fs(
                    &manifest.target_partition,
                    target_mnt,
                    Some("btrfs"),
                          Some("subvol=@"),
                )
                .is_err()
                {
                    guard.mount_fs(&manifest.target_partition, target_mnt, Some("btrfs"), None)?;
                }
        } else {
            guard.mount_fs(&manifest.target_partition, target_mnt, None, None)?;
        }

        let _ = fs::remove_file(target_mnt.join("var/lib/pacman/db.lck"));

        if manifest.is_uefi
            && !manifest.efi_partition.is_empty()
            && manifest.efi_partition.starts_with("/dev/")
            {
                guard.mount_fs(
                    &manifest.efi_partition,
                    &target_mnt.join("boot"),
                               Some("vfat"),
                               None,
                )?;
            }

            let _ = tx.send(TelemetryEvent::Progress(0.40));
            let kernel = if manifest.os_id == "cachyos" {
                "linux-cachyos"
            } else {
                manifest.kernel_pkg.as_str()
            };
            let headers = format!("{}-headers", kernel);
            send_log(&tx, format!("[2/5] Reinstalling {} and headers...", kernel));

            run_host_cmd(
                &tx,
                "pacstrap",
                &[
                    "-K",
                    "/mnt/kestrel_repair",
                    "--noconfirm",
                    kernel,
                    &headers,
                    "mkinitcpio",
                ],
            )?;

            let _ = tx.send(TelemetryEvent::Progress(0.65));
            send_log(
                &tx,
                "[3/5] Installing Self-Healing ALPM Guard & binding chroot VFS...",
            );
            install_self_healing_alpm_guard(target_mnt)?;
            guard.mount_chroot_api_vfs()?;

            let _ = tx.send(TelemetryEvent::Progress(0.80));
            send_log(&tx, "[4/5] Rebuilding initramfs images via mkinitcpio -P...");
            run_in_chroot(&tx, target_mnt, "mkinitcpio", &["-P"], None)?;

            let _ = tx.send(TelemetryEvent::Progress(0.90));
            send_log(
                &tx,
                format!("[5/5] Restoring {:?} bootloader...", manifest.bootloader),
            );

            let root_uuid = get_partition_uuid(&manifest.target_partition)?;
            match manifest.bootloader {
                BootloaderType::SystemdBoot => {
                    let _ = run_in_chroot(&tx, target_mnt, "bootctl", &["install"], None)
                    .or_else(|_| run_in_chroot(&tx, target_mnt, "bootctl", &["update"], None));
                    let entries = target_mnt.join("boot/loader/entries");
                    fs::create_dir_all(&entries).ok();
                    let entry = format!(
                        "title   Arch Linux ({})\nlinux   /vmlinuz-{}\ninitrd  /initramfs-{}.img\noptions root=UUID={} rw quiet\n",
                                        kernel, kernel, kernel, root_uuid
                    );
                    fs::write(entries.join("arch.conf"), entry).map_err(|e| e.to_string())?;
                }
                BootloaderType::Grub => {
                    run_host_cmd(
                        &tx,
                        "pacstrap",
                        &["-K", "/mnt/kestrel_repair", "--noconfirm", "grub", "os-prober"],
                    )?;
                    if manifest.is_uefi {
                        run_in_chroot(
                            &tx,
                            target_mnt,
                            "grub-install",
                            &[
                                "--target=x86_64-efi",
                                "--efi-directory=/boot",
                                "--bootloader-id=GRUB",
                                "--recheck",
                            ],
                            None,
                        )?;
                    }
                    run_in_chroot(
                        &tx,
                        target_mnt,
                        "grub-mkconfig",
                        &["-o", "/boot/grub/grub.cfg"],
                        None,
                    )?;
                }
                BootloaderType::Limine => {
                    run_host_cmd(
                        &tx,
                        "pacstrap",
                        &["-K", "/mnt/kestrel_repair", "--noconfirm", "limine"],
                    )?;
                    if manifest.is_uefi {
                        let efi_boot = target_mnt.join("boot/EFI/BOOT");
                        fs::create_dir_all(&efi_boot).ok();
                        let _ = fs::copy(
                            target_mnt.join("usr/share/limine/BOOTX64.EFI"),
                                         efi_boot.join("BOOTX64.EFI"),
                        );
                    }
                }
                BootloaderType::Refind => {
                    run_host_cmd(
                        &tx,
                        "pacstrap",
                        &["-K", "/mnt/kestrel_repair", "--noconfirm", "refind", "efibootmgr"],
                    )?;
                    run_in_chroot(&tx, target_mnt, "refind-install", &[], None)?;
                }
            }

            guard.cleanup_all();
            let _ = tx.send(TelemetryEvent::Progress(1.0));
            send_log(&tx, "System recovery completed cleanly!");
            Ok(())
    };

    match run() {
        Ok(()) => {
            let _ = tx.send(TelemetryEvent::Completed);
        }
        Err(err) => {
            let _ = tx.send(TelemetryEvent::Fault(err));
        }
    }
}
