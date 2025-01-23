use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

slint::slint! {
    export { AppWindow }
    import { AppWindow } from "ui/client-ui.slint";
}

#[tokio::main]
async fn main() {
    let app_ui = AppWindow::new().expect("Failed to create UI");
    
    // Connect to the chat server
    let stream = match TcpStream::connect("127.0.0.1:8080").await {
        Ok(stream) => {
            println!("Connected to chat server!");
            stream
        }
        Err(e) => {
            eprintln!("Failed to connect to chat server: {}", e);
            return; 
        }
    };

    // Split the stream into read and write parts
    let (mut reader, writer) = stream.into_split();
    let writer = Arc::new(Mutex::new(writer));
    
    // Create a channel for sending messages to the UI
    let (tx, mut rx) = mpsc::channel(100);
    
    // Handle sending messages
    {
        let weak = app_ui.as_weak();
        let writer = Arc::clone(&writer);
        weak.unwrap().on_send_message(move |message| {
            let message = message.trim().to_string();
            println!("Message to be sent to server: {}", message);
            if message.is_empty() {
                return;
            }
            
            let writer = Arc::clone(&writer);
            tokio::spawn(async move {
                let message = format!("{}\n", message);
                if let Err(e) = writer.lock().await.write_all(message.as_bytes()).await {
                    eprintln!("Failed to send message: {}", e);
                }
                if let Err(e) = writer.lock().await.flush().await {
                    eprintln!("Failed to flush message: {}", e);
                }
            });
        });
    }
    
    // Handle receiving messages
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => {
                        println!("Server closed connection");
                        break;
                    }
                    Ok(n) => {
                        if let Ok(msg) = String::from_utf8(buf[..n].to_vec()) {
                            let msg = msg.trim().to_string();
                            println!("Received message from server: {}", msg);
                            if !msg.is_empty() {
                                if let Err(e) = tx.send(msg).await {
                                    eprintln!("Failed to send message to UI: {}", e);
                                    break;
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
        });
    }
    
    // Handle UI updates
    let weak = app_ui.as_weak();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let msg = msg.clone();
            println!("Updating UI with message: {}", msg);
            let weak = weak.clone();
            if let Err(e) = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    let current_text = ui.get_received_text();
                    println!("Current text in UI: {}", current_text);
                    ui.set_received_text(format!("{}\nOther: {}", current_text, msg).into());
                }
            }) {
                eprintln!("Failed to update UI: {}", e);
            }
        }
    });
    
    app_ui.run().expect("Failed to run application");
}
