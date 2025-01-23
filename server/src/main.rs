use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast,
    io::{AsyncWriteExt, AsyncReadExt},
};
use std::error::Error;
use std::net::SocketAddr;

#[derive(Clone, Debug)]
struct Message {
    from: SocketAddr,
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a new TCP listener bound to "127.0.0.1:8080"
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Chat server listening on 127.0.0.1:8080");

    // Create a broadcast channel for messages
    let (tx, _) = broadcast::channel::<Message>(100);

    loop {
        // Accept new connections
        let (socket, addr) = listener.accept().await?;
        println!("New client connected: {}", addr);

        // Clone the sender for this connection
        let tx = tx.clone();
        // Create a new receiver for this connection
        let mut rx = tx.subscribe();

        // Spawn a new task to handle this client
        tokio::spawn(async move {
            let mut socket = socket;
            handle_connection(socket, addr, tx, rx).await;
        });
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    tx: broadcast::Sender<Message>,
    mut rx: broadcast::Receiver<Message>,
) {
    let mut buffer = [0u8; 1024];
    
    // Split the socket into a reader and writer
    let (mut reader, mut writer) = socket.split();
    
    loop {
        tokio::select! {
            // Handle incoming messages from this client
            result = reader.read(&mut buffer) => {
                match result {
                    Ok(0) => {
                        println!("Client disconnected: {}", addr);
                        break;
                    }
                    Ok(n) => {
                        let message = String::from_utf8_lossy(&buffer[..n]).to_string();
                        println!("Received message from {}: {}", addr, message);
                        
                        // Create and broadcast the message
                        let msg = Message {
                            from: addr,
                            content: message.trim().to_string(),
                        };
                        
                        // Broadcast the message to all clients
                        if let Err(e) = tx.send(msg) {
                            eprintln!("Failed to broadcast message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read from socket: {}", e);
                        break;
                    }
                }
            }
            
            // Handle messages from other clients
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        // Only forward messages to other clients (not back to sender)
                        if msg.from != addr {
                            println!("Forwarding message to {}: {}", addr, msg.content);
                            // Send the message content with a newline to ensure proper display
                            let message = format!("{}\n", msg.content);
                            println!("Sending message: {}", message);
                            if let Err(e) = writer.write_all(message.as_bytes()).await {
                                eprintln!("Failed to write to socket: {}", e);
                                break;
                            }
                            if let Err(e) = writer.flush().await {
                                eprintln!("Failed to flush socket: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to receive broadcast: {}", e);
                        break;
                    }
                }
            }
        }
    }
}
