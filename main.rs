use clap::{Parser, ValueEnum};
use serde::Deserialize;
use ssh2::Session;
use std::fs::File;
use std::io::Read;
use std::net::{TcpStream};
use std::path::Path;
use std::process;
use wol_rs::WakeOnLan;

/// Azioni disponibili
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Action {
    Up,
    Down,
}

#[derive(Parser, Debug)]
#[command(version, about = "Comserv: Wake-on-LAN e Shutdown via SSH", long_about = None)]
struct Cli {
    /// Azione: up (accendi) o down (spegni)
    #[arg(value_enum)]
    action: Action,

    /// File di configurazione del server (es: serverinoUno.conf)
    config: String,
}

/// Rappresenta il file di configurazione TOML
#[derive(Debug, Deserialize)]
struct ServerConfig {
    server: Server,
}

#[derive(Debug, Deserialize)]
struct Server {
    name: String,
    mac: String,
    ip: String,
    user: String,
    port: Option<u16>,
    ssh_key: String,
}

fn load_config(path: &str) -> ServerConfig {
    let mut file = File::open(path).unwrap_or_else(|_| {
        eprintln!("❌ Impossibile leggere il file di configurazione {}", path);
        process::exit(1);
    });

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    toml::from_str(&contents).expect("❌ Errore nel parsing del file .conf (TOML)")
}

fn wake_on_lan(mac: &str) {
    match WakeOnLan::from_mac(mac).and_then(|wol| wol.send_magic_packet()) {
        Ok(_) => println!("✅ Magic Packet inviato a {}", mac),
        Err(e) => eprintln!("❌ Errore nell'invio del pacchetto WOL: {}", e),
    }
}

fn ssh_shutdown(ip: &str, user: &str, port: u16, key_path: &str) {
    match TcpStream::connect((ip, port)) {
        Ok(tcp) => {
            let mut sess = Session::new().unwrap();
            sess.set_tcp_stream(tcp);
            sess.handshake().unwrap();

            let key_path = Path::new(key_path);
            let pubkey_path = key_path.with_extension("pub");

            sess.userauth_pubkey_file(user, Some(&pubkey_path), &key_path, None)
                .expect("❌ Autenticazione SSH fallita");

            if !sess.authenticated() {
                eprintln!("❌ SSH non autenticato");
                return;
            }

            let mut channel = sess.channel_session().unwrap();
            channel.exec("shutdown -h now").unwrap();
            channel.send_eof().unwrap();
            channel.wait_close().unwrap();
            println!("🛑 Comando shutdown inviato a {}", ip);
        }
        Err(e) => {
            eprintln!("❌ Connessione SSH fallita: {}", e);
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let path = format!("conf.d/{}", cli.config);

    let config = load_config(&path);
    let server = config.server;

    match cli.action {
        Action::Up => wake_on_lan(&server.mac),
        Action::Down => {
            let port = server.port.unwrap_or(22);
            ssh_shutdown(&server.ip, &server.user, port, &server.ssh_key);
        }
    }
}

