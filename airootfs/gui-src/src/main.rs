slint::include_modules!();

use std::process::{Command, Stdio};
use std::io::{Read};
use std::thread;
use std::rc::Rc;
use std::time::{Instant, Duration};
use slint::{ModelRc, SharedString, VecModel};

fn format_size(bytes: u64) -> String {
    let kb = 1024.0;
    let mb = kb * 1024.0;
    let gb = mb * 1024.0;
    let b = bytes as f64;
    
    if b >= gb { format!("{:.1}G", b / gb) }
    else if b >= mb { format!("{:.1}M", b / mb) }
    else if b >= kb { format!("{:.1}K", b / kb) }
    else { format!("{}B", bytes) }
}

fn scan_partitions(disk_path: &str) -> (Vec<PartitionData>, Vec<SharedString>) {
    let mut partitions = Vec::new();
    let mut available_dropdown = Vec::new();
    
    // UX FIX: Inject a safe placeholder at the very top of the dropdown lists!
    available_dropdown.push("Select Partition...".into());

    let disk_output = Command::new("lsblk").arg("-b").arg("-n").arg("-d").arg("-o").arg("SIZE").arg(disk_path).output();
    let total_bytes: f64 = if let Ok(out) = disk_output {
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(1.0)
    } else { 1.0 };

    let output = Command::new("lsblk")
        .arg("-P").arg("-p").arg("-b").arg("-o").arg("NAME,FSTYPE,LABEL,MOUNTPOINT,SIZE")
        .arg(disk_path)
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
                    fstype: if fs.is_empty() { "Unformatted".into() } else { fs.into() },
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

fn main() -> Result<(), slint::PlatformError> {
    let ui = InstallerWindow::new()?;
    
    let is_offline = std::path::Path::new("/opt/offline_cache").exists();
    ui.set_is_offline_cached(is_offline);

    let is_efi = std::path::Path::new("/sys/firmware/efi").exists();
    ui.set_is_efi_system(is_efi);

    let falkon_text = if is_offline {
        SharedString::from("Falkon (Offline Default)")
    } else {
        SharedString::from("Falkon (Recommended for KDE/Arch)")
    };
    ui.set_falkon_label(falkon_text);

    if let Ok(status) = Command::new("ping").arg("-c").arg("1").arg("-W").arg("2").arg("archlinux.org").status() {
        if status.success() {
            ui.set_has_ethernet(true);
        }
    }

    let output = Command::new("lsblk")
        .arg("-nd").arg("-o").arg("NAME,SIZE")
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
                    let display_str = format!("/dev/{} - {}", name, size);
                    disks.push(display_str.into());
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
        ui.global::<InstallerLogic>().set_current_partitions(ModelRc::from(Rc::new(VecModel::from(parts))));
        ui.set_available_partitions(ModelRc::from(Rc::new(VecModel::from(avail))));
    }

    let disks_model = Rc::new(VecModel::from(disks));
    ui.set_available_disks(ModelRc::from(disks_model.clone()));

    let ui_handle_fetch = ui.as_weak();
    ui.global::<InstallerLogic>().on_fetch_partitions(move |disk| {
        let pure_disk_path = disk.as_str().split_whitespace().next().unwrap_or("").to_string();
        let (parts, avail) = scan_partitions(&pure_disk_path);
        
        if let Some(ui) = ui_handle_fetch.upgrade() {
            ui.global::<InstallerLogic>().set_current_partitions(ModelRc::from(Rc::new(VecModel::from(parts))));
            ui.set_available_partitions(ModelRc::from(Rc::new(VecModel::from(avail))));
        }
    });

    let ui_handle_net = ui.as_weak();
    ui.global::<InstallerLogic>().on_check_network_and_proceed(move |mode| {
        let ui_handle = ui_handle_net.clone();
        let mode_str = mode.to_string();

        thread::spawn(move || {
            let is_online = mode_str.contains("Online");
            let mut needs_wifi = false;
            
            if is_online {
                let status = Command::new("ping").arg("-c").arg("1").arg("-W").arg("2").arg("archlinux.org").status();
                if status.is_err() || !status.unwrap().success() {
                    needs_wifi = true;
                }
            }

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    if needs_wifi {
                        ui.set_has_ethernet(false);
                        ui.set_active_step(2);
                    } else {
                        ui.set_has_ethernet(true);
                        ui.set_active_step(3);
                    }
                }
            }).unwrap();
        });
    });

    let ui_handle_wifi = ui.as_weak();
    ui.global::<InstallerLogic>().on_rescan_wifi(move || {
        let ui_handle = ui_handle_wifi.clone();
        thread::spawn(move || {
            let iface_output = Command::new("sh").arg("-c").arg("iw dev | awk '$1==\"Interface\"{print $2}' | head -n 1").output();
            if let Ok(out) = iface_output {
                let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !iface.is_empty() {
                    let _ = Command::new("iwctl").arg("station").arg(&iface).arg("scan").status();
                    thread::sleep(std::time::Duration::from_secs(2));
                    
                    let nets_output = Command::new("sh").arg("-c").arg(format!("iwctl station {} get-networks | awk 'NR>4 {{print $2}}'", iface)).output();
                    if let Ok(net_out) = nets_output {
                        let stdout = String::from_utf8_lossy(&net_out.stdout);
                        let mut net_list: Vec<SharedString> = Vec::new();
                        for net in stdout.lines() {
                            let clean_net = net.trim();
                            if !clean_net.is_empty() {
                                net_list.push(clean_net.into());
                            }
                        }
                        if !net_list.is_empty() {
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_handle.upgrade() {
                                    let net_model = Rc::new(VecModel::from(net_list));
                                    ui.set_available_networks(ModelRc::from(net_model.clone()));
                                }
                            }).unwrap();
                        }
                    }
                }
            }
        });
    });

    let ui_handle_connect = ui.as_weak();
    ui.global::<InstallerLogic>().on_connect_wifi(move |ssid, password| {
        let ui_handle = ui_handle_connect.clone();
        let ssid_str = ssid.to_string();
        let pass_str = password.to_string();

        thread::spawn(move || {
            let iface_output = Command::new("sh").arg("-c").arg("iw dev | awk '$1==\"Interface\"{print $2}' | head -n 1").output();
            if let Ok(out) = iface_output {
                let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !iface.is_empty() {
                    if pass_str.is_empty() {
                        let _ = Command::new("iwctl").arg("station").arg(&iface).arg("connect").arg(&ssid_str).status();
                    } else {
                        let _ = Command::new("iwctl").arg("station").arg(&iface).arg("connect").arg(&ssid_str).arg("--passphrase").arg(&pass_str).status();
                    }
                    thread::sleep(std::time::Duration::from_secs(4));
                }
            }

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_active_step(3);
                }
            }).unwrap();
        });
    });

    ui.global::<InstallerLogic>().on_reboot_system(move || {
        let _ = Command::new("systemctl").arg("reboot").spawn();
    });

    ui.global::<InstallerLogic>().on_poweroff_system(move || {
        let _ = Command::new("systemctl").arg("poweroff").spawn();
    });

    ui.global::<InstallerLogic>().on_close_installer(move || {
        std::process::exit(1);
    });

    let ui_handle_gparted = ui.as_weak();
    ui.global::<InstallerLogic>().on_launch_gparted(move |disk| {
        let ui_handle = ui_handle_gparted.clone();
        let pure_disk_path = disk.as_str().split_whitespace().next().unwrap_or("").to_string();
        
        thread::spawn(move || {
            let _ = Command::new("gparted").arg(&pure_disk_path).status();
            let (parts, avail) = scan_partitions(&pure_disk_path);

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.global::<InstallerLogic>().set_current_partitions(ModelRc::from(Rc::new(VecModel::from(parts))));
                    ui.set_available_partitions(ModelRc::from(Rc::new(VecModel::from(avail))));
                }
            }).unwrap();
        });
    });

    let ui_handle = ui.as_weak();
    
    ui.global::<InstallerLogic>().on_start_install(move |
        target_disk, install_mode, part_strategy, 
        filesystem, replace_path,
        gui_root_part, gui_efi_part, 
        hostname, username, password, root_password, 
        browser, perf, selected_de, selected_boot
    | {
        let ui_handle = ui_handle.clone();
        
        let pure_disk_path = target_disk.as_str().split_whitespace().next().unwrap_or("").to_string();
        let mode_num = install_mode.as_str().split('.').next().unwrap_or("2").to_string();
        let part_num = part_strategy.as_str().split('.').next().unwrap_or("1").to_string();
        
        let fs_str = filesystem.as_str().to_string();
        let replace_str = replace_path.as_str().to_string();
        
        // Safety check to strip the dummy placeholder text if it accidentally gets passed
        let mut root_part_str = gui_root_part.as_str().to_string(); 
        if root_part_str.contains("Select") { root_part_str = "".to_string(); }
        
        let mut efi_part_str = gui_efi_part.as_str().to_string();   
        if efi_part_str.contains("Select") { efi_part_str = "".to_string(); }
        
        let host_str = hostname.as_str().to_string();
        let user_str = username.as_str().to_string();
        let pass_str = password.as_str().to_string();
        let root_pass_str = root_password.as_str().to_string();
        
        let perf_char = if perf.as_str().starts_with('Y') { "Y" } else { "N" };
        let de_num = selected_de.as_str().split('.').next().unwrap_or("1").to_string();
        
        let browser_str = browser.as_str();
        let mut browser_num = if browser_str.contains("Zen") { "1" }
        else if browser_str.contains("LibreWolf") { "2" }
        else if browser_str.contains("Firefox") { "3" }
        else if browser_str.contains("Brave") { "4" }
        else { "5" }.to_string();

        let boot_str = selected_boot.as_str();
        let boot_num = if boot_str.contains("Limine") { "4" }
        else if boot_str.contains("rEFInd") { "3" }
        else if boot_str.contains("systemd-boot") { "2" }
        else { "1" }.to_string();

        if mode_num == "2" {
            browser_num = "5".to_string();
        }
        
        thread::spawn(move || {
            let mut child = Command::new("bash")
                .arg("-c")
                .arg("/usr/local/bin/install.sh 2>&1")
                .env("TARGET_DISK", &pure_disk_path)
                .env("INSTALL_MODE", &mode_num)
                .env("PARTITION_STRATEGY", &part_num)
                .env("GUI_FILESYSTEM", &fs_str)
                .env("GUI_REPLACE_PART", &replace_str)
                .env("GUI_ROOT_PART", &root_part_str) 
                .env("GUI_EFI_PART", &efi_part_str)   
                .env("GUI_HOSTNAME", &host_str)
                .env("GUI_USERNAME", &user_str)
                .env("GUI_PASSWORD", &pass_str)
                .env("GUI_ROOT_PASSWORD", &root_pass_str)
                .env("BROWSER_CHOICE", &browser_num)
                .env("PERF_CHOICE", &perf_char)
                .env("DE_CHOICE", &de_num)
                .env("BOOT_CHOICE", &boot_num)
                .env("NON_INTERACTIVE", "1") 
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to execute Kestrel bash script");

            let mut stdout = child.stdout.take().expect("Failed to capture stdout");
            let mut buffer = [0u8; 128]; 
            let mut current_line = String::new();
            let mut in_ansi = false;

            let mut current_progress: f32 = 0.0;
            let mut dynamic_status_text = String::new();
            
            // NEW METRICS FOR LIVE PARSING
            let mut total_packages: f32 = 0.0;
            let mut downloaded_count: f32 = 0.0;
            
            let mut log_lines: Vec<String> = vec![
                "> Initiating Kestrel Arch Deployment Protocol...".to_string(),
                "> Reading configuration matrix...".to_string(),
            ];

            let mut last_ui_update = Instant::now();
            let update_interval = Duration::from_millis(32); 

            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buffer[..n]);
                        
                        for c in chunk.chars() {
                            if in_ansi {
                                if c.is_ascii_alphabetic() {
                                    in_ansi = false; 
                                }
                                continue;
                            }

                            match c {
                                '\n' => {
                                    let output = current_line.trim();
                                    let mut is_package_spam = false;
                                    
                                    if output.contains("Formatting") || output.contains("partition") {
                                        current_progress = 0.10;
                                    } 
                                    // 1. CAPTURE THE TOTAL PACKAGE COUNT
                                    else if output.contains("Packages (") && output.contains(')') {
                                        let text_after = output.split("Packages (").nth(1).unwrap_or("");
                                        let num_str = text_after.split(')').next().unwrap_or("");
                                        if let Ok(total) = num_str.parse::<f32>() {
                                            total_packages = total;
                                            dynamic_status_text = format!("Preparing to download {} packages...", total);
                                            is_package_spam = true;
                                        }
                                    } 
                                    // 2. THE NEW LINE PARSER (Bye-bye dumb file watcher!)
                                    else if output.ends_with(" downloading...") {
                                        downloaded_count += 1.0;
                                        if total_packages > 0.0 {
                                            let ratio = (downloaded_count / total_packages).min(1.0);
                                            current_progress = 0.15 + (0.35 * ratio);
                                            dynamic_status_text = format!("Downloading packages... ({}/{})", downloaded_count as u32, total_packages as u32);
                                        }
                                        is_package_spam = true;
                                    }
                                    // 3. CATCH INTEGRITY CHECKS (Instantly kill download phase)
                                    else if output.contains("checking package integrity") || output.contains("checking keyring") || output.contains("resolving dependencies") {
                                        current_progress = 0.50;
                                        dynamic_status_text = output.to_string();
                                        is_package_spam = true;
                                    }
                                    // 4. CATCH THE INSTALLATION PHASE
                                    else if output.starts_with('(') && output.contains('/') && output.contains(')') {
                                        let bracket_part = output.split(')').next().unwrap_or("");
                                        let nums: String = bracket_part.chars().filter(|c| c.is_ascii_digit() || *c == '/').collect();
                                        let num_parts: Vec<&str> = nums.split('/').collect();
                                        
                                        if num_parts.len() == 2 {
                                            if let (Ok(current), Ok(total)) = (num_parts[0].parse::<f32>(), num_parts[1].parse::<f32>()) {
                                                if total > 0.0 && current <= total {
                                                    current_progress = 0.50 + (0.40 * (current / total));
                                                    is_package_spam = true;
                                                    dynamic_status_text = output.to_string();
                                                }
                                            }
                                        }
                                    } 
                                    else if output.contains("bootloader") || output.contains("grub") || output.contains("limine") {
                                        current_progress = 0.95;
                                    }

                                    if !output.is_empty() {
                                        // Hide spammy download logs from the UI box so it doesn't jitter wildly
                                        if !is_package_spam && !output.contains("Packages (") {
                                            log_lines.push(format!("> {}", output));
                                        }
                                        if !is_package_spam {
                                            dynamic_status_text = output.to_string();
                                        }
                                    }
                                    current_line.clear();
                                }
                                '\r' => { current_line.clear(); }
                                '\x08' => { current_line.pop(); }
                                '\x1B' => { in_ansi = true; }
                                _ => {
                                    if !c.is_control() {
                                        current_line.push(c);
                                    }
                                }
                            }
                        }

                        if last_ui_update.elapsed() >= update_interval {
                            let mut display_log = log_lines.clone();
                            let active_line = current_line.trim();
                            
                            if !active_line.is_empty() {
                                display_log.push(format!("> {}", active_line));
                            }

                            if display_log.len() > 100 {
                                let excess = display_log.len() - 100;
                                display_log.drain(0..excess);
                            }

                            let log_update = display_log.join("\n");
                            
                            let status_text = if !active_line.is_empty() {
                                active_line.to_string()
                            } else if !dynamic_status_text.is_empty() {
                                dynamic_status_text.clone()
                            } else {
                                log_lines.last().unwrap_or(&String::new()).replace("> ", "")
                            };

                            slint::invoke_from_event_loop({
                                let ui_handle = ui_handle.clone();
                                move || {
                                    if let Some(ui) = ui_handle.upgrade() {
                                        ui.global::<InstallerLogic>().set_status_text(status_text.into());
                                        ui.global::<InstallerLogic>().set_progress(current_progress);
                                        ui.global::<InstallerLogic>().set_console_log(log_update.into());
                                    }
                                }
                            }).unwrap();

                            last_ui_update = Instant::now();
                        }
                    }
                    Err(_) => break,
                }
            }
            
            let status = child.wait().expect("Failed to wait on backend process");

            if !status.success() {
                log_lines.push("\n[!] CRITICAL FAULT: Deployment process exited with a non-zero status code. Read logs above for details.".to_string());
            }
            
            let final_log = log_lines.join("\n");

            slint::invoke_from_event_loop({
                let ui_handle = ui_handle.clone();
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        if status.success() {
                            ui.global::<InstallerLogic>().set_progress(1.0);
                            ui.set_active_step(99);
                        } else {
                            ui.global::<InstallerLogic>().set_status_text("Installation failed! Check console output.".into());
                            ui.global::<InstallerLogic>().set_console_log(final_log.into());
                            ui.global::<InstallerLogic>().set_install_failed(true);
                        }
                    }
                }
            }).unwrap();
        });
    });

    ui.run()
}
