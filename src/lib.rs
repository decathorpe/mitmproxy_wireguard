mod tcp;
mod wg;

use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use boringtun::crypto::{X25519PublicKey, X25519SecretKey};
use pyo3::prelude::*;
use tokio::sync::mpsc;

use crate::tcp::{ConnectionHandler, TcpConnection};
use crate::wg::WgServer;

type ConnectionId = u32;
type SendQueue = Arc<RwLock<VecDeque<Vec<u8>>>>;

#[derive(Debug)]
struct EventAdapter {
    send_queues: Arc<RwLock<HashMap<ConnectionId, SendQueue>>>,
    event_push: mpsc::Sender<Events>,
    sock_count: Arc<RwLock<u32>>,
    reuse_sock: Arc<RwLock<Vec<u32>>>,
}

impl EventAdapter {
    fn new(event_push: mpsc::Sender<Events>) -> Self {
        EventAdapter {
            send_queues: Default::default(),
            event_push,
            sock_count: Default::default(),
            reuse_sock: Default::default(),
        }
    }

    fn send_queues(&self) -> Arc<RwLock<HashMap<ConnectionId, SendQueue>>> {
        self.send_queues.clone()
    }

    async fn ready(&self) {
        // FIXME: use a custom waker?
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}

#[async_trait::async_trait]
impl ConnectionHandler for EventAdapter {
    async fn handle(&self, connection: TcpConnection) {
        let socket = connection.socket;
        let src_addr = connection.src_addr;
        let dst_addr = connection.dst_addr;
        let mut reader = connection.conn_forw_recv;
        let writer = connection.conn_back_send;

        let connection_id: u32 = {
            if let Some(connection_id) = self.reuse_sock.write().unwrap().pop() {
                connection_id
            } else {
                let mut sock_count = self.sock_count.write().unwrap();
                *sock_count += 1;
                *sock_count
            }
        };

        // initialize and register send queue for this connection
        let send_queue = SendQueue::default();
        self.send_queues
            .write()
            .unwrap()
            .insert(connection_id, send_queue.clone());

        self.event_push
            .send(Events::ConnectionEstablished(ConnectionEstablished {
                connection_id,
                src_addr: PySockAddr(src_addr),
                dst_addr: PySockAddr(dst_addr),
            }))
            .await
            .unwrap();

        loop {
            tokio::select!(
                ret = reader.recv() => {
                    if let Some(data) = ret {
                        // pending data on the socket
                        self.event_push.send(
                            Events::DataReceived(DataReceived { connection_id, data })
                        ).await.unwrap();
                    } else {
                        // connection was closed
                        self.event_push.send(
                            Events::ConnectionClosed(ConnectionClosed { connection_id })
                        ).await.unwrap();

                        self.reuse_sock.write().unwrap().push(connection_id);
                        // FIXME: stop attempting to read after the connection was closed
                    }
                },
                _ = self.ready() => {
                    loop {
                        let result = send_queue.write().unwrap().pop_front();
                        if let Some(data) = result {
                            writer.send((socket, data)).await.unwrap();
                        } else {
                            break;
                        }
                    }
                }
            )
        }
    }
}

#[derive(Clone, Debug)]
struct PySockAddr(SocketAddr);

impl Display for PySockAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl IntoPy<PyObject> for PySockAddr {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self.0 {
            SocketAddr::V4(addr) => (addr.ip().to_string(), addr.port()).into_py(py),
            SocketAddr::V6(addr) => {
                log::debug!(
                    "converting ipv6 to python, not sure if this is correct: {:?}",
                    (addr.ip().to_string(), addr.port())
                );
                (addr.ip().to_string(), addr.port()).into_py(py)
            },
        }
    }
}

#[pyclass]
#[derive(Debug)]
struct ConnectionEstablished {
    #[pyo3(get)]
    connection_id: ConnectionId,
    #[pyo3(get)]
    src_addr: PySockAddr,
    #[pyo3(get)]
    dst_addr: PySockAddr,
}

#[pymethods]
impl ConnectionEstablished {
    fn __repr__(&self) -> String {
        format!(
            "ConnectionEstablished({}, {}, {})",
            self.connection_id, self.src_addr, self.dst_addr
        )
    }
}

#[pyclass]
#[derive(Debug)]
struct DataReceived {
    #[pyo3(get)]
    connection_id: ConnectionId,
    #[pyo3(get)]
    data: Vec<u8>,
}

#[pymethods]
impl DataReceived {
    fn __repr__(&self) -> String {
        format!("DataReceived({}, {:x?})", self.connection_id, self.data)
    }
}

#[pyclass]
#[derive(Debug)]
struct ConnectionClosed {
    #[pyo3(get)]
    connection_id: ConnectionId,
}

#[pymethods]
impl ConnectionClosed {
    fn __repr__(&self) -> String {
        format!("ConnectionClosed({})", self.connection_id)
    }
}

/*
#[pyclass]
#[derive(Debug)]
struct DatagramReceived {
    #[pyo3(get)]
    src_addr: PySockAddr,
    #[pyo3(get)]
    dst_addr: PySockAddr,
    #[pyo3(get)]
    data: Vec<u8>,
}

#[pymethods]
impl DatagramReceived {
    fn __repr__(&self) -> String {
        format!(
            "DatagramReceived({}, {}, {:x?})",
            self.src_addr, self.dst_addr, self.data
        )
    }
}
*/

#[derive(Debug)]
enum Events {
    ConnectionEstablished(ConnectionEstablished),
    DataReceived(DataReceived),
    ConnectionClosed(ConnectionClosed),
    //DatagramReceived(DatagramReceived),
}

impl IntoPy<PyObject> for Events {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Events::ConnectionEstablished(e) => e.into_py(py),
            Events::DataReceived(e) => e.into_py(py),
            Events::ConnectionClosed(e) => e.into_py(py),
            //Events::DatagramReceived(e) => e.into_py(py),
        }
    }
}

#[pyclass]
struct ServerConnection {
    send_queues: Arc<RwLock<HashMap<ConnectionId, SendQueue>>>,
}

#[pymethods]
impl ServerConnection {
    fn send(&self, connection_id: ConnectionId, data: Vec<u8>) -> PyResult<()> {
        let queues = self.send_queues.write().unwrap();
        let inner = queues.get(&connection_id).unwrap();
        let queue = &mut *inner.write().unwrap();
        queue.push_back(data);
        Ok(())
    }

    fn close(&self, _connection_id: ConnectionId) -> PyResult<()> {
        todo!()
    }
}

struct WireguardServer {
    server: WgServer,
    event_receiver: mpsc::Receiver<Events>,
    send_queues: Arc<RwLock<HashMap<ConnectionId, SendQueue>>>,
}

impl WireguardServer {
    fn new(host: String, port: u16) -> WireguardServer {
        // TODO: make configurable
        let server_priv_key: X25519SecretKey = "c72d788fd0916b1185177fd7fa392451192773c889d17ac739571a63482c18bb"
            .parse()
            .unwrap();

        // TODO: make configurable
        let peer_pub_key: X25519PublicKey = "DbwqnNYZWk5e19uuSR6WomO7VPaVbk/uKhmyFEnXdH8=".parse().unwrap();

        let server_addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();

        let (tx, rx) = mpsc::channel::<Events>(64);

        let eventer = EventAdapter::new(tx);
        let send_queues = eventer.send_queues();

        let mut server = WgServer::new(server_addr, server_priv_key, Box::new(eventer));

        // TODO: make configurable
        server.add_peer(Arc::new(peer_pub_key), None).unwrap();

        WireguardServer {
            server,
            event_receiver: rx,
            send_queues,
        }
    }

    fn connection(&self) -> ServerConnection {
        ServerConnection {
            send_queues: self.send_queues.clone(),
        }
    }

    fn _stop(&self) {
        //self.python_callback_task.abort();
        // TODO: this is not completely trivial. we should probably close all connections somehow.
        //       does smoltcp do something for us here?
    }
}

#[pyfunction]
fn start_server(py: Python<'_>, host: String, port: u16, event_handler: PyObject) -> PyResult<&PyAny> {
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let mut server = WireguardServer::new(host, port);

        let connection = server.connection();

        // spawn Python event handler
        tokio::spawn(async move {
            while let Some(event) = server.event_receiver.recv().await {
                Python::with_gil(|py| {
                    if let Err(err) = event_handler.call1(py, (event,)) {
                        err.print(py);
                    }
                });
            }
        });

        // spawn WireGuard server
        tokio::spawn(server.server.serve());
        Ok(connection)
    })
}

#[pymodule]
fn mitmproxy_wireguard(_py: Python, m: &PyModule) -> PyResult<()> {
    // set up the Rust logger to send messages to the Python logger
    pyo3_log::init();

    m.add_function(wrap_pyfunction!(start_server, m)?)?;
    m.add_class::<ServerConnection>()?;
    m.add_class::<ConnectionEstablished>()?;
    m.add_class::<DataReceived>()?;
    m.add_class::<ConnectionClosed>()?;
    //m.add_class::<DatagramReceived>()?;
    Ok(())
}
