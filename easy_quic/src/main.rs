use anyhow::Result;
use eframe::egui;
use easy_quic::{QuicClient, QuicServer};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
struct Message {
    text: String,
    is_sent: bool,
}

struct ChatApp {
    messages: Arc<Mutex<Vec<Message>>>,
    input_text: String,
    tx: Sender<String>,
    server_port: u16,
}

impl ChatApp {
    fn new(tx: Sender<String>, server_port: u16) -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            input_text: String::new(),
            tx,
            server_port,
        }
    }
    
    fn add_message(&self, text: String, is_sent: bool) {
        if let Ok(mut messages) = self.messages.lock() {
            messages.push(Message { text, is_sent });
        }
    }
}

impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!("QUIC Chat - Server Port: {}", self.server_port));
            
            ui.separator();
            
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    if let Ok(messages) = self.messages.lock() {
                        for msg in messages.iter() {
                            if msg.is_sent {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                    ui.colored_label(egui::Color32::from_rgb(100, 200, 100), &msg.text);
                                });
                            } else {
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                                    ui.colored_label(egui::Color32::from_rgb(100, 150, 200), &msg.text);
                                });
                            }
                        }
                    }
                });
            
            ui.separator();
            
            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut self.input_text);
                
                if ui.button("Send").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    if !self.input_text.trim().is_empty() {
                        let msg = self.input_text.clone();
                        self.add_message(msg.clone(), true);
                        let _ = self.tx.send(msg);
                        self.input_text.clear();
                        response.request_focus();
                    }
                }
            });
        });
        
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn start_server(port: u16, app_messages: Arc<Mutex<Vec<Message>>>) {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let addr = format!("127.0.0.1:{}", port).parse().unwrap();
        let server = QuicServer::new(addr).await.unwrap();
        
        println!("Server started on port {}", port);
        
        loop {
            if let Some(incoming) = server.endpoint.accept().await {
                let connection = incoming.await.unwrap();
                let app_messages = app_messages.clone();
                
                tokio::spawn(async move {
                    loop {
                        match connection.accept_bi().await {
                            Ok((mut send, mut recv)) => {
                                let buffer = recv.read_to_end(64 * 1024).await.unwrap();
                                
                                let message = String::from_utf8(buffer).unwrap();
                                
                                if let Ok(mut messages) = app_messages.lock() {
                                    messages.push(Message {
                                        text: format!("Peer: {}", message),
                                        is_sent: false,
                                    });
                                }
                                
                                let response = format!("Echo: {}", message);
                                send.write_all(response.as_bytes()).await.unwrap();
                                send.finish().unwrap();
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
        }
    });
}

fn start_client(server_port: u16, rx: Receiver<String>, app_messages: Arc<Mutex<Vec<Message>>>) {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        thread::sleep(Duration::from_secs(1));
        
        let client_addr = "127.0.0.1:0".parse().unwrap();
        let server_addr = format!("127.0.0.1:{}", server_port).parse().unwrap();
        
        let client = QuicClient::new(client_addr).await.unwrap();
        let connection = client.connect(server_addr, "localhost").await.unwrap();
        
        println!("Client connected to server");
        
        while let Ok(message) = rx.recv() {
            match connection.send_message(&message).await {
                Ok(response) => {
                    if let Ok(mut messages) = app_messages.lock() {
                        messages.push(Message {
                            text: response,
                            is_sent: false,
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Error sending message: {}", e);
                }
            }
        }
    });
}

fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("easy_quic=warn".parse().unwrap())
                .add_directive("quinn=warn".parse().unwrap())
        )
        .init();
    
    let server_port = 4433;
    let (tx, rx) = mpsc::channel();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 500.0])
            .with_title("QUIC Chat Application"),
        ..Default::default()
    };
    
    let app = ChatApp::new(tx, server_port);
    let app_messages = app.messages.clone();
    
    thread::spawn({
        let app_messages = app_messages.clone();
        move || start_server(server_port, app_messages)
    });
    
    thread::spawn({
        let app_messages = app_messages.clone();
        move || start_client(server_port, rx, app_messages)
    });
    
    eframe::run_native(
        "QUIC Chat",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}