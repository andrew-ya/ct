//
//
// use futures_util::{future, pin_mut, StreamExt};
// use tokio::io::{AsyncReadExt, AsyncWriteExt};
// use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
// use url;
//
//
// #[tokio::main]
// async fn main() {
//
//     let connect_addr = "wss://testnet.binance.vision/ws/bnbusdt@depth5";
//
//     let url = url::Url::parse(&connect_addr).unwrap();
//
//     let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
//     // tokio::spawn(read_stdin(stdin_tx));
//
//     let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
//     println!("WebSocket handshake has been successfully completed");
//
//     let (write, read) = ws_stream.split();
//
//     let stdin_to_ws = stdin_rx.map(Ok).forward(write);
//     let ws_to_stdout = {
//         read.for_each(|message| async {
//             let mut data: Vec<u8> = message.unwrap().into_data();
//             data.extend("\n".to_owned().into_bytes());
//             tokio::io::stdout().write_all(&data).await.unwrap();
//         })
//     };
//
//     stdin_to_ws.write("".into());
//
//     pin_mut!(stdin_to_ws, ws_to_stdout);
//     future::select(stdin_to_ws, ws_to_stdout).await;
// }
