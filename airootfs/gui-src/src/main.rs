mod engine;

slint::include_modules!();

use engine::{
    probe_hardware_topology, BootloaderType, DeploymentManifest, InstallMode, PartitionStrategy,
    RepairManifest, TelemetryEvent,
};
use slint::{ModelRc, SharedString, VecModel};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
struct ExistingInstall {
    partition: String,
    os_id: String,
    label: String,
}

fn format_size(bytes: u64) -> String {
    let kb = 1024.0;
    let mb = kb * 1024.0;
    let gb = mb * 1024.0;
    let b = bytes as f64;

    if b >= gb {
        format!("{:.1}G", b / gb)
    } else if b >= mb {
        format!("{:.1}M", b / mb)
    } else if b >= kb {
        format!("{:.1}K", b / kb)
    } else {
        format!("{}B", bytes)
    }
}

fn scan_partitions(disk_path: &str) -> (Vec<PartitionData>, Vec<SharedString>) {
    let mut partitions = Vec::new();
    let mut available_dropdown = Vec::new();

    available_dropdown.push("Select Partition...".into());

    let disk_output = Command::new("lsblk")
    .args(["-b", "-n", "-d", "-o", "SIZE", disk_path])
    .output();
    let total_bytes: f64 = if let Ok(out) = disk_output {
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(1.0)
    } else {
        1.0
    };

    let output = Command::new("lsblk")
    .args(["-P", "-p", "-b", "-o", "NAME,FSTYPE,LABEL,MOUNTPOINT,SIZE", disk_path])
    .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let colors = [
            slint::Color::from_rgb_u8(239, 68, 68),
            slint::Color::from_rgb_u8(245, 158, 11),
            slint::Color::from_rgb_u8(16, 185, 129),
            slint::Color::from_rgb_u8(59, 130, 246),
            slint::Color::from_rgb_u8(168, 85, 247),
        ];
        let mut color_idx = 0;

        for line in stdout.lines() {
            let get_val = |key: &str| -> String {
                if let Some(start) = line.find(&format!("{}=\"", key)) {
                    let content_start = start + key.len() + 2;
                    if let Some(end) = line[content_start..].find('"') {
                        return line[content_start..content_start + end].to_string();
                    }
                }
                String::new()
            };

            let name = get_val("NAME");
            if !name.is_empty() && name != disk_path {
                let fs = get_val("FSTYPE");
                let raw_size: u64 = get_val("SIZE").parse().unwrap_or(0);
                let stretch_val = (raw_size as f64 / total_bytes) as f32;

                let part = PartitionData {
                    name: name.clone().into(),
                    fstype: if fs.is_empty() {
                        "Unformatted".into()
                    } else {
                        fs.into()
                    },
                    label: get_val("LABEL").into(),
                    mountpoint: get_val("MOUNTPOINT").into(),
                    size: format_size(raw_size).into(),
                    color_hex: colors[color_idx % colors.len()],
                    stretch: stretch_val,
                };

                partitions.push(part);
                available_dropdown.push(name.into());
                color_idx += 1;
            }
        }
    }

    if partitions.is_empty() {
        partitions.push(PartitionData {
            name: "Unallocated Space".into(),
                        fstype: "None".into(),
                        label: "".into(),
                        mountpoint: "".into(),
                        size: format_size(total_bytes as u64).into(),
                        color_hex: slint::Color::from_rgb_u8(75, 85, 99),
                        stretch: 1.0,
        });
    }

    (partitions, available_dropdown)
}

fn probe_existing_systems() -> (Vec<ExistingInstall>, Vec<String>) {
    let mut targets = Vec::new();
    let mut efi_partitions = Vec::new();
    let temp_mount = "/mnt/kestrel_probe_temp";
    let _ = fs::create_dir_all(temp_mount);

    if let Ok(output) = Command::new("lsblk").args(["-lno", "PATH,FSTYPE"]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let path = parts[0];
            let fstype = parts[1];

            if fstype == "vfat" || fstype == "fat" {
                efi_partitions.push(path.to_string());
            }

            if matches!(fstype, "ext4" | "btrfs" | "xfs" | "f2fs") {
                let mounted = Command::new("mount")
                .args(["-o", "ro,subvol=@", path, temp_mount])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
                || Command::new("mount")
                .args(["-o", "ro", path, temp_mount])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

                if mounted {
                    let arch_rel = Path::new(temp_mount).join("etc/arch-release").exists();
                    let cachy_rel = Path::new(temp_mount).join("etc/cachyos-release").exists();

                    if arch_rel || cachy_rel {
                        let os_id = if cachy_rel {
                            "cachyos".to_string()
                        } else if Path::new(temp_mount).join("etc/kestrel-release").exists() {
                            "kestrel".to_string()
                        } else {
                            "arch".to_string()
                        };

                        let name = match os_id.as_str() {
                            "cachyos" => "CachyOS",
                            "kestrel" => "Kestrel Arch",
                            _ => "Arch Linux",
                        };

                        targets.push(ExistingInstall {
                            partition: path.to_string(),
                                     os_id,
                                     label: format!("{} on {}", name, path),
                        });
                    }
                    let _ = Command::new("umount").arg(temp_mount).status();
                }
            }
        }
    }

    let _ = fs::remove_dir_all(temp_mount);
    (targets, efi_partitions)
}

fn attach_telemetry_stream(
    rx: mpsc::Receiver<TelemetryEvent>,
    ui_weak: slint::Weak<InstallerWindow>,
    initial_logs: Vec<String>,
) {
    thread::spawn(move || {
        let mut logs = initial_logs;
        while let Ok(event) = rx.recv() {
            match event {
                TelemetryEvent::Progress(p) => {
                    let ui_copy = ui_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_copy.upgrade() {
                            ui.global::<InstallerLogic>().set_progress(p);
                        }
                    });
                }
                TelemetryEvent::Status(s) => {
                    let ui_copy = ui_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_copy.upgrade() {
                            ui.global::<InstallerLogic>().set_status_text(s.into());
                        }
                    });
                }
                TelemetryEvent::Log(line) => {
                    logs.push(line);
                    if logs.len() > 120 {
                        let excess = logs.len() - 120;
                        logs.drain(0..excess);
                    }
                    let joined = logs.join("\n");
                    let ui_copy = ui_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_copy.upgrade() {
                            ui.global::<InstallerLogic>().set_console_log(joined.into());
                        }
                    });
                }
                TelemetryEvent::Completed => {
                    let ui_copy = ui_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_copy.upgrade() {
                            ui.global::<InstallerLogic>().set_progress(1.0);
                            ui.set_active_step(99);
                        }
                    });
                    break;
                }
                TelemetryEvent::Fault(err) => {
                    logs.push(format!("\n[!] CRITICAL FAULT: {}", err));
                    let joined = logs.join("\n");
                    let ui_copy = ui_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_copy.upgrade() {
                            ui.global::<InstallerLogic>()
                            .set_status_text(format!("Fault: {}", err).into());
                            ui.global::<InstallerLogic>().set_console_log(joined.into());
                            ui.global::<InstallerLogic>().set_install_failed(true);
                        }
                    });
                    break;
                }
            }
        }
    });
}

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = env::args().collect();

    // Headless CLI Repair Mode: kestrel-gui --repair <root_part> <os_id> <kernel> <bootloader> [efi_part]
    if args.len() >= 6 && args[1] == "--repair" {
        let bootloader = match args[5].as_str() {
            "systemd-boot" => BootloaderType::SystemdBoot,
            "Limine" => BootloaderType::Limine,
            "rEFInd" => BootloaderType::Refind,
            _ => BootloaderType::Grub,
        };
        let manifest = RepairManifest {
            target_partition: args[2].clone(),
            os_id: args[3].clone(),
            kernel_pkg: args[4].clone(),
            bootloader,
            efi_partition: args.get(6).cloned().unwrap_or_default(),
            is_uefi: Path::new("/sys/firmware/efi").exists(),
        };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || engine::execute_repair(manifest, tx));
        while let Ok(ev) = rx.recv() {
            match ev {
                TelemetryEvent::Log(l) => println!("{}", l),
                TelemetryEvent::Completed => {
                    println!("==> [SUCCESS] Repair finished.");
                    std::process::exit(0);
                }
                TelemetryEvent::Fault(f) => {
                    eprintln!("==> [FAULT] {}", f);
                    std::process::exit(1);
                }
                _ => {}
            }
        }
        return Ok(());
    }

    let ui = InstallerWindow::new()?;

    let is_offline = Path::new("/opt/offline_cache").exists();
    ui.set_is_offline_cached(is_offline);

    let is_efi = Path::new("/sys/firmware/efi").exists();
    ui.set_is_efi_system(is_efi);

    let repair_bootloaders: Vec<SharedString> = if is_efi {
        vec![
            "GRUB".into(),
            "systemd-boot".into(),
            "Limine".into(),
            "rEFInd".into(),
        ]
    } else {
        vec!["GRUB".into(), "Limine".into()]
    };
    ui.global::<InstallerLogic>()
    .set_available_repair_bootloaders(ModelRc::from(Rc::new(VecModel::from(
        repair_bootloaders,
    ))));

    let falkon_text = if is_offline {
        SharedString::from("Falkon (Offline Default)")
    } else {
        SharedString::from("Falkon (Recommended for KDE/Arch)")
    };
    ui.set_falkon_label(falkon_text);

    if let Ok(status) = Command::new("ping")
        .args(["-c", "1", "-W", "2", "archlinux.org"])
        .status()
        {
            if status.success() {
                ui.set_has_ethernet(true);
            }
        }

        let output = Command::new("lsblk")
        .args(["-nd", "-o", "NAME,SIZE"])
        .output()
        .expect("Failed to execute lsblk");

        let mut disks: Vec<SharedString> = Vec::new();
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0];
                    let size = parts[1];
                    if name.starts_with("sd") || name.starts_with("nvme") || name.starts_with("vd") {
                        disks.push(format!("/dev/{} - {}", name, size).into());
                    }
                }
            }
        }
        if disks.is_empty() {
            disks.push("No drives found!".into());
        }

        if let Some(first_disk) = disks.first() {
            let pure = first_disk.as_str().split_whitespace().next().unwrap_or("");
            let (parts, avail) = scan_partitions(pure);
            ui.global::<InstallerLogic>()
            .set_current_partitions(ModelRc::from(Rc::new(VecModel::from(parts))));
            ui.set_available_partitions(ModelRc::from(Rc::new(VecModel::from(avail))));
        }

        ui.set_available_disks(ModelRc::from(Rc::new(VecModel::from(disks))));

        let detected_targets_state: Arc<Mutex<Vec<ExistingInstall>>> = Arc::new(Mutex::new(Vec::new()));

        let ui_handle_repair_scan = ui.as_weak();
        let state_scan = detected_targets_state.clone();
        ui.global::<InstallerLogic>()
        .on_scan_existing_installations(move || {
            let ui_handle = ui_handle_repair_scan.clone();
            let state_scan = state_scan.clone();

            thread::spawn(move || {
                let (installs, efi_parts) = probe_existing_systems();
                let mut labels: Vec<SharedString> = Vec::new();
                let mut os_ids: Vec<SharedString> = Vec::new();

                if installs.is_empty() {
                    labels.push("No existing Arch/CachyOS installations found.".into());
                    os_ids.push("none".into());
                } else {
                    for inst in &installs {
                        labels.push(inst.label.clone().into());
                        os_ids.push(inst.os_id.clone().into());
                    }
                }

                let mut efi_dropdown: Vec<SharedString> = Vec::new();
                if efi_parts.is_empty() {
                    efi_dropdown.push("None / Legacy MBR".into());
                } else {
                    for efi in efi_parts {
                        efi_dropdown.push(efi.into());
                    }
                }

                *state_scan.lock().unwrap() = installs;

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.global::<InstallerLogic>()
                        .set_detected_install_labels(ModelRc::from(Rc::new(VecModel::from(
                            labels,
                        ))));
                        ui.global::<InstallerLogic>()
                        .set_detected_install_os_ids(ModelRc::from(Rc::new(VecModel::from(
                            os_ids,
                        ))));
                        ui.global::<InstallerLogic>()
                        .set_available_efi_partitions(ModelRc::from(Rc::new(VecModel::from(
                            efi_dropdown,
                        ))));
                    }
                });
            });
        });

        let ui_handle_repair_exec = ui.as_weak();
        let state_exec = detected_targets_state.clone();
        ui.global::<InstallerLogic>().on_start_repair(
            move |install_idx, kernel_choice, boot_choice, efi_part| {
                let ui_handle = ui_handle_repair_exec.clone();
                let targets = state_exec.lock().unwrap().clone();
                let idx = install_idx as usize;

                if idx >= targets.len() {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.global::<InstallerLogic>()
                        .set_status_text("Error: Invalid installation selected.".into());
                        ui.global::<InstallerLogic>().set_install_failed(true);
                    }
                    return;
                }

                let target = targets[idx].clone();
                let bootloader = match boot_choice.as_str() {
                    "systemd-boot" => BootloaderType::SystemdBoot,
                    "Limine" => BootloaderType::Limine,
                    "rEFInd" => BootloaderType::Refind,
                    _ => BootloaderType::Grub,
                };

                let manifest = RepairManifest {
                    target_partition: target.partition.clone(),
                                                      os_id: target.os_id.clone(),
                                                      kernel_pkg: kernel_choice.to_string(),
                                                      bootloader,
                                                      efi_partition: efi_part.to_string(),
                                                      is_uefi: Path::new("/sys/firmware/efi").exists(),
                };

                let (tx, rx) = mpsc::channel();
                attach_telemetry_stream(
                    rx,
                    ui_handle,
                    vec![
                        format!("> Native Rust Repair Engine initialized on {}", target.partition),
                            format!("> Kernel: {} | Bootloader: {:?}", kernel_choice, bootloader),
                    ],
                );

                thread::spawn(move || {
                    engine::execute_repair(manifest, tx);
                });
            },
        );

        let ui_handle_fetch = ui.as_weak();
        ui.global::<InstallerLogic>()
        .on_fetch_partitions(move |disk| {
            let pure = disk.as_str().split_whitespace().next().unwrap_or("");
            let (parts, avail) = scan_partitions(pure);
            if let Some(ui) = ui_handle_fetch.upgrade() {
                ui.global::<InstallerLogic>()
                .set_current_partitions(ModelRc::from(Rc::new(VecModel::from(parts))));
                ui.set_available_partitions(ModelRc::from(Rc::new(VecModel::from(avail))));
            }
        });

        let ui_handle_net = ui.as_weak();
        ui.global::<InstallerLogic>()
        .on_check_network_and_proceed(move |mode| {
            let ui_handle = ui_handle_net.clone();
            let mode_str = mode.to_string();

            thread::spawn(move || {
                let is_online = mode_str.contains("Online");
                let mut needs_wifi = false;

                if is_online {
                    let status = Command::new("ping")
                    .args(["-c", "1", "-W", "2", "archlinux.org"])
                    .status();
                    if status.is_err() || !status.unwrap().success() {
                        needs_wifi = true;
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_has_ethernet(!needs_wifi);
                        ui.set_active_step(if needs_wifi { 2 } else { 3 });
                    }
                });
            });
        });

        let ui_handle_wifi = ui.as_weak();
        ui.global::<InstallerLogic>().on_rescan_wifi(move || {
            let ui_handle = ui_handle_wifi.clone();
            thread::spawn(move || {
                if let Ok(out) = Command::new("sh")
                    .arg("-c")
                    .arg("iw dev | awk '$1==\"Interface\"{print $2}' | head -n 1")
                    .output()
                    {
                        let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if !iface.is_empty() {
                            let _ = Command::new("iwctl")
                            .args(["station", &iface, "scan"])
                            .status();
                            thread::sleep(Duration::from_secs(2));
                            if let Ok(net_out) = Command::new("sh")
                                .arg("-c")
                                .arg(format!(
                                    "iwctl station {} get-networks | awk 'NR>4 {{print $2}}'",
                                    iface
                                ))
                                .output()
                                {
                                    let stdout = String::from_utf8_lossy(&net_out.stdout);
                                    let net_list: Vec<SharedString> = stdout
                                    .lines()
                                    .map(|s| s.trim())
                                    .filter(|s| !s.is_empty())
                                    .map(SharedString::from)
                                    .collect();
                                    if !net_list.is_empty() {
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(ui) = ui_handle.upgrade() {
                                                ui.set_available_networks(ModelRc::from(Rc::new(
                                                    VecModel::from(net_list),
                                                )));
                                            }
                                        });
                                    }
                                }
                        }
                    }
            });
        });

        let ui_handle_connect = ui.as_weak();
        ui.global::<InstallerLogic>()
        .on_connect_wifi(move |ssid, password| {
            let ui_handle = ui_handle_connect.clone();
            let ssid_str = ssid.to_string();
            let pass_str = password.to_string();

            thread::spawn(move || {
                if let Ok(out) = Command::new("sh")
                    .arg("-c")
                    .arg("iw dev | awk '$1==\"Interface\"{print $2}' | head -n 1")
                    .output()
                    {
                        let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if !iface.is_empty() {
                            if pass_str.is_empty() {
                                let _ = Command::new("iwctl")
                                .args(["station", &iface, "connect", &ssid_str])
                                .status();
                            } else {
                                let _ = Command::new("iwctl")
                                .args([
                                    "station",
                                    &iface,
                                    "connect",
                                    &ssid_str,
                                    "--passphrase",
                                    &pass_str,
                                ])
                                .status();
                            }
                            thread::sleep(Duration::from_secs(4));
                        }
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            ui.set_active_step(3);
                        }
                    });
            });
        });

        ui.global::<InstallerLogic>().on_reboot_system(move || {
            let _ = Command::new("systemctl").arg("reboot").spawn();
        });

        ui.global::<InstallerLogic>().on_poweroff_system(move || {
            let _ = Command::new("systemctl").arg("poweroff").spawn();
        });

        ui.global::<InstallerLogic>().on_close_installer(move || {
            std::process::exit(0);
        });

        let ui_handle_gparted = ui.as_weak();
        ui.global::<InstallerLogic>()
        .on_launch_gparted(move |disk| {
            let ui_handle = ui_handle_gparted.clone();
            let pure = disk
            .as_str()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

            thread::spawn(move || {
                let _ = Command::new("gparted").arg(&pure).status();
                let (parts, avail) = scan_partitions(&pure);

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.global::<InstallerLogic>()
                        .set_current_partitions(ModelRc::from(Rc::new(VecModel::from(parts))));
                        ui.set_available_partitions(ModelRc::from(Rc::new(VecModel::from(avail))));
                    }
                });
            });
        });

        let ui_handle_install = ui.as_weak();
        ui.global::<InstallerLogic>().on_start_install(
            move |target_disk,
            install_mode,
            part_strategy,
            filesystem,
            replace_path,
            gui_root_part,
            gui_efi_part,
            hostname,
            username,
            password,
            root_password,
            browser,
            perf,
            selected_de,
            selected_boot| {
                let ui_handle = ui_handle_install.clone();

                let (timezone, locale, keymap) = if let Some(ui) = ui_handle.upgrade() {
                    let logic = ui.global::<InstallerLogic>();
                    (
                        logic.get_selected_timezone().to_string(),
                     logic.get_selected_locale().to_string(),
                     logic.get_selected_keymap().to_string(),
                    )
                } else {
                    ("UTC".into(), "en_US.UTF-8".into(), "us".into())
                };

                let pure_disk = target_disk
                .as_str()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();

                let mode = if install_mode.as_str().starts_with('1') {
                    InstallMode::Online
                } else {
                    InstallMode::Offline
                };

                let strategy = if part_strategy.as_str().starts_with('2') {
                    PartitionStrategy::ReplacePartition
                } else if part_strategy.as_str().starts_with('3') {
                    PartitionStrategy::ManualGParted
                } else {
                    PartitionStrategy::WipeDisk
                };

                let de_index: u8 = selected_de
                .as_str()
                .split('.')
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1);

                let browser_index: u8 = if mode == InstallMode::Offline {
                    5
                } else if browser.as_str().contains("Zen") {
                    1
                } else if browser.as_str().contains("LibreWolf") {
                    2
                } else if browser.as_str().contains("Firefox") {
                    3
                } else if browser.as_str().contains("Brave") {
                    4
                } else {
                    5
                };

                let bootloader = if selected_boot.as_str().contains("Limine") {
                    BootloaderType::Limine
                } else if selected_boot.as_str().contains("rEFInd") {
                    BootloaderType::Refind
                } else if selected_boot.as_str().contains("systemd-boot") {
                    BootloaderType::SystemdBoot
                } else {
                    BootloaderType::Grub
                };

                let hw = probe_hardware_topology(&pure_disk);

                let manifest = DeploymentManifest {
                    target_disk: pure_disk.clone(),
                                                       mode,
                                                       strategy,
                                                       requested_filesystem: filesystem.to_string(),
                                                       replace_partition: replace_path.to_string(),
                                                       manual_root_partition: gui_root_part.to_string(),
                                                       manual_efi_partition: gui_efi_part.to_string(),
                                                       hostname: hostname.to_string(),
                                                       username: username.to_string(),
                                                       user_password: password.to_string(),
                                                       root_password: if root_password.is_empty() {
                                                           password.to_string()
                                                       } else {
                                                           root_password.to_string()
                                                       },
                                                       timezone: timezone.clone(),
                                                       locale: locale.clone(),
                                                       keymap: keymap.clone(),
                                                       desktop_index: de_index,
                                                       browser_index,
                                                       bootloader,
                                                       hyper_performance: perf.as_str().starts_with('Y'),
                                                       hardware: hw.clone(),
                };

                let (tx, rx) = mpsc::channel();
                attach_telemetry_stream(
                    rx,
                    ui_handle,
                    vec![
                        "> Kestrel Pure-Rust Provisioning Engine engaged.".into(),
                                        format!(
                                            "> CPU: {} ({}) | Threads: {} | RAM: {} MB",
                                                hw.cpu_vendor, hw.uarch_level, hw.cpu_threads, hw.total_ram_mb
                                        ),
                                        format!(
                                            "> Regional: TZ={} | Locale={} | Keymap={}",
                                            timezone, locale, keymap
                                        ),
                    ],
                );

                thread::spawn(move || {
                    engine::execute_deployment(manifest, tx);
                });
            },
        );

        ui.run()
}
