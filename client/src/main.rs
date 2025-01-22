use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

slint::slint! {
    export { AppWindow }
    import { AppWindow } from "ui/client-ui.slint";
}

#[tokio::main]
async fn main() {
    let app_ui = AppWindow::new().expect("Failed to create UI");
    let chat_stream = Arc::new(Mutex::new(None::<TcpStream>));
    
    // Connect to the chat server
    match TcpStream::connect("127.0.0.1:8080").await {
        Ok(stream) => {
            println!("Connected to chat server!");
            *chat_stream.lock().await = Some(stream);
        }
        Err(e) => {
            eprintln!("Failed to connect to chat server: {}", e);
            return; 
        }
    }
    
    // Clone stream for the message sender
    let chat_stream_clone = Arc::clone(&chat_stream);
    
    // Handle sending messages
    {
        let weak = app_ui.as_weak();
        weak.unwrap().on_send_message(move |message| {
            let message = message.trim().to_string();
            if message.is_empty() {
                return;
            }
            
            let chat_stream = chat_stream_clone.clone();
            tokio::spawn(async move {
                let message = message.clone();
                if let Some(stream) = &mut *chat_stream.lock().await {
                    if let Err(e) = stream.write_all(message.as_bytes()).await {
                        eprintln!("Failed to send message: {}", e);
                    }
                }
            });
        });
    }
    
    // Handle receiving messages
    {
        let weak = app_ui.as_weak();
        let chat_stream = Arc::clone(&chat_stream);
        tokio::spawn(async move {
            if let Some(stream) = &mut *chat_stream.lock().await {
                let mut buf = [0u8; 1024];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(n) if n == 0 => break, // Connection closed
                        Ok(n) => {
                            if let Ok(msg) = String::from_utf8(buf[..n].to_vec()) {
                                let msg = msg.trim();
                                if !msg.is_empty() {
                                    if let Some(ui) = weak.upgrade() {
                                        let current_text = ui.get_received_text();
                                        ui.set_received_text(format!("{}\nOther: {}", current_text, msg).into());
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read from socket: {}", e);
                            break;
                        }
                    }
                }
            }
        });
    }
    
    app_ui.run().expect("Failed to run application");
}
