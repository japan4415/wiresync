use std::error::Error;
use std::process::{Command, Stdio};
use std::path::Path;
use std::fs::File;
use std::io::{self, Read, Write, BufReader};

pub fn interface_config_factory(
    in_wg_ip: &String,
    private_key: &String,
    port: &i32,
    nic_name: &String,
) -> Result<String, Box<dyn Error>> {
    let config_text = format!("
    [Interface]\n
    Address: {}\n
    PrivateKey: [{}]\n
    ListenPort: {}\n
    \n
    PostUp = iptables -A FORWARD -i wg0 -j ACCEPT; iptables -t nat -A POSTROUTING -o {} -j MASQUERADE\n
    PostDown = iptables -D FORWARD -i wg0 -j ACCEPT; iptables -t nat -D POSTROUTING -o {} -j MASQUERADE\n
    ", in_wg_ip, private_key, port, nic_name, nic_name);
    Ok(config_text)
}

pub fn peer_config_factory(
    endpoint: &String,
    port: &i32,
    private_key: &String,
    in_wg_ip: &String,
) -> Result<String, Box<dyn Error>> {
    let mut config_text = "".to_string();
    config_text = config_text
        + format!(
            "
        [Peer]\n
        Endpoint = {}:{}\n
        PublickKey = {}\n
        AllowedIPs = {}/32\n
        PersistentKeepalive = 25\n
        ",
            endpoint,
            port,
            publick_key_factory(private_key)?,
            in_wg_ip,
        )
        .as_str();
    Ok(config_text)
}

pub fn publick_key_factory(private_key: &String) -> Result<String, Box<dyn Error>> {
    let echo_output = Command::new("echo")
        .args([format!("{}", private_key)])
        .stdout(Stdio::piped())
        .spawn()?;
    let wg_output = Command::new("wg")
        .args(["pubkey"])
        .stdin(Stdio::from(echo_output.stdout.unwrap()))
        .output()?;
    Ok(String::from_utf8_lossy(&wg_output.stdout).to_string())
}

pub fn update_config(config: String) -> Result<(), Box<dyn Error>> {
    if Path::new("/etc/wireguard/ws0.conf").exists() {
        let mut file = File::open("/etc/wireguard/ws0.conf")?;
        file.write_all(config.as_bytes())?;
    } else {
        let mut file = File::create("/etc/wireguard/ws0.conf")?;
        file.write_all(config.as_bytes())?;
    }
    Ok(())
}

pub fn restart_wg() -> Result<(), Box<dyn Error>> {
    let mut wg_down = Command::new("wg-quick")
        .args(["down", "ws0"])
        .spawn()?;
    wg_down.wait()?;
    let mut wg_up = Command::new("wg-quick")
        .args(["up", "ws0"])
        .spawn()?;
    wg_up.wait()?;
    Ok(())
}