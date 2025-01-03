use std::{
    io::{Error, Read, Write},
    net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, ToSocketAddrs},
    sync::{Arc, Condvar, Mutex},
};

use lazy_static::lazy_static;
use smoltcp::{
    iface::{Config, Context, Interface, SocketHandle, SocketSet},
    phy::{Loopback, Medium},
    socket::tcp::{ConnectError, ListenError, Socket},
    storage::RingBuffer,
    time::Instant,
    wire::{EthernetAddress, IpAddress, IpCidr},
};
// use tracing;

pub type SocketBuffer<'a> = RingBuffer<'a, u8>;
pub struct Engine {
    core: Arc<Mutex<Core>>,
    condvar: Arc<Condvar>,
}
struct Core {
    socketset: SocketSet<'static>,
    iface: Interface,
    device: Loopback, // for now
}

lazy_static! {
    static ref ENGINE: Arc<Engine> = Arc::new(Engine::new());
}

impl Engine {
    fn new() -> Self {
        Self {
            core: Arc::new(Mutex::new(Core::new())),
            condvar: Arc::new(Condvar::new()),
        }
    }
    fn add_socket(&self, socket: Socket<'static>) -> SocketHandle {
        self.core.lock().unwrap().add_socket(socket)
    }
    // functions to get sockets
    // Block until f returns Some(R), and then return R. Note that f may be called multiple times,
    // and it may be called spuriously.
    fn blocking<R>(&self, mut f: impl FnMut(&mut Core) -> Option<R>) -> R {
        let mut inner = self.core.lock().unwrap();
        println!("blocking(): polling from blocking");
        // Immediately poll, since we wait to have as up-to-date state as possible.
        inner.poll(&self.condvar);
        loop {
            // We'll need the polling thread to wake up and do work.
            // self.channel.send(()).unwrap();
            match f(&mut *inner) {
                Some(r) => {
                    // We have done work, so again, notify the polling thread.
                    // self.channel.send(()).unwrap();
                    return r;
                }
                None => {
                    println!("blocking(): blocking thread");
                    inner = self.condvar.wait(inner).unwrap();
                }
            }
        }
        // mutex dropped here.
    }
}

impl Core {
    fn new() -> Self {
        let config = Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into()); // change later!
        let mut socketset = SocketSet::new(Vec::new());
        let mut device = Loopback::new(Medium::Ethernet);
        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 9))
                .unwrap();
        });
        Self {
            socketset,
            device,
            iface,
        }
    }
    fn add_socket(&mut self, sock: Socket<'static>) -> SocketHandle {
        self.socketset.add(sock)
    }
    fn get_socket(&mut self, handle: SocketHandle) -> &Socket<'static> {
        self.socketset.get(handle)
    }
    fn get_mutable_socket(&mut self, handle: SocketHandle) -> &mut Socket<'static> {
        self.socketset.get_mut(handle)
    }
    fn poll(&mut self, waiter: &Condvar) -> bool {
        let res = self
            .iface
            .poll(Instant::now(), &mut self.device, &mut self.socketset);
        // When we poll, notify the CV so that other waiting threads can retry their blocking
        // operations.
        // println!("poll(): notify cv");
        // waiter.notify_all();
        res
    }
}

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener {
    socket_handle: Arc<Mutex<SocketHandle>>,
    local_addr: SocketAddr,
    port: u16,
}

impl SmolTcpListener {
    /* each_addr():
     * parameters:
     * helper function for bind()
     * processes each address given to see whether it can implement ToSocketAddr, then tries to
     * listen on that addr keeps trying each address until one of them successfully listens
     */
    fn each_addr<A: ToSocketAddrs>(
        sock_addrs: A,
        s: &mut Socket<'static>,
    ) -> Result<(u16, SocketAddr), ListenError> {
        let addrs = {
            match sock_addrs.to_socket_addrs() {
                Ok(addrs) => addrs,
                Err(e) => return Err(ListenError::InvalidState),
            }
        };
        for addr in addrs {
            match (*s).listen(addr.port()) {
                Ok(_) => return Ok((addr.port(), addr)),
                Err(_) => return Err(ListenError::Unaddressable),
            }
        }
        Err(ListenError::InvalidState) // is that the correct thing to return?
    }
    fn do_bind<A: ToSocketAddrs>(addrs: A) -> Result<(Socket<'static>, u16, SocketAddr), Error> {
        let mut sock = {
            let rx_buffer = SocketBuffer::new(vec![0; 4096]);
            let tx_buffer = SocketBuffer::new(vec![0; 4096]);
            Socket::new(rx_buffer, tx_buffer) // this is the listening socket
        };
        let (port, local_address) = {
            match Self::each_addr(addrs, &mut sock) {
                Ok((port, local_address)) => (port, local_address),
                Err(_) => return Err(Error::other("listening error")),
            }
        };
        Ok((sock, port, local_address))
    }
    /* bind
     * accepts: address(es)
     * returns: a tcpsocket
     * creates a tcpsocket and binds the address to that socket.
     * if multiple addresses given, it will attempt to bind to each until successful
     */
    /*
        example arguments passed to bind:
        "127.0.0.1:0"
        SocketAddr::from(([127, 0, 0, 1], 443))
        let addrs = [ SocketAddr::from(([127, 0, 0, 1], 80)),  SocketAddr::from(([127, 0, 0, 1], 443)), ];
    */
    pub fn bind<A: ToSocketAddrs>(addrs: A) -> Result<SmolTcpListener, Error> {
        let engine = &ENGINE;
        let (sock, port, local_address) = {
            match Self::do_bind(addrs) {
                Ok((sock, port, local_address)) => (sock, port, local_address),
                Err(_) => {
                    return Err(Error::other("listening error"));
                }
            }
        };
        // println!("shim: in bind()");
        let handle = (*engine).add_socket(sock);
        // for later work: allocate a queue to hold pending connection requests. sounds like a semaphore?
        let tcp = SmolTcpListener {
            socket_handle: Arc::new(Mutex::new(handle)),
            port,
            local_addr: local_address,
        };
        Ok(tcp)
    }
    fn get_handle(&self) -> SocketHandle {
        let handle = self.socket_handle.lock().unwrap();
        *handle
    }
    fn change_handle(&self, new_handle: SocketHandle) {
        let mut handle = self.socket_handle.lock().unwrap();
        *handle = new_handle;
    }
    // accept
    // block until there is a waiting connection in the queue
    // create a new socket for tcpstream
    // ^^ creating a new one so that the user can call accept() on the previous one again
    // return tcpstream
    /* accept():
     * parameters: -
     * return: (SmolTcpStream, SocketAddr) upon success; Error upon failure
     * takes the current listener and advances the socket's state (in terms of the smoltcp state machine)
     */
    pub fn accept(&self) -> Result<(SmolTcpStream, SocketAddr), Error> {
        // create another socket to listen on the same port and use that as a listener
        // we can have multiple sockets listening on the same port
        println!("shim: in accept()");
        // this is the listener
        let engine = &ENGINE;
        let stream;
        let mut socket: &mut Socket<'static>;
        loop {
            {
                let mut core = (*engine).core.lock().unwrap();
                core.poll(&engine.condvar);
                socket = core.get_mutable_socket(self.get_handle());
                if socket.is_active() {
                    let remote = socket.remote_endpoint().unwrap();
                    drop(core);
                    println!("shim: accepted connection; in accept()");
                    stream = SmolTcpStream {
                        socket_handle: self.get_handle(),
                        local_addr: self.local_addr,
                        port: self.port,
                        rx_shutdown: Arc::new(Mutex::new(false)),
                    };
                    // the socket addr returned is that of the remote endpoint. ie. the client.
                    let remote_addr = SocketAddr::from((remote.addr, remote.port));
                    { // creating another listener and changing the value of the socket handle in self
                        let next_list = (Self::bind(self.local_addr).unwrap());
                        Self::change_handle(&self, next_list.get_handle());
                    }

                    return Ok((stream, remote_addr));
                }
            } // mutex drops here
        }
        // how are we handling error cases? should there be some sort of a timeout?
        return Err(Error::other("accepting error"));
    }

    pub fn local_addr(&self) -> Result<SocketAddr, Error> {
        // rethink this one.
        // smoltcp supports fns listen_endpoint() and local_endpoint(). use one of those instead.
        return Ok(self.local_addr);
    }
}
//@ananya: you are ugly you two-faced devil. -sanjana
#[derive(Debug)]
pub struct SmolTcpStream {
    socket_handle: SocketHandle,
    local_addr: SocketAddr,
    port: u16,
    rx_shutdown: Arc<Mutex<bool>>,
}
impl Read for SmolTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        // "All currently blocked and future reads will return Ok(0)."
        // -- std::net shutdown documentation
        // ^^ check whether the shutdown for read has been called, then return Ok(0)
        if self.get_rx_shutdown() == true {
            return Ok(0);
        }
        let engine = &ENGINE;
        let mut core = engine.core.lock().unwrap();
        let socket = core.get_mutable_socket(self.socket_handle);
        let result = socket.recv_slice(buf);
        // println!(" - {}", String::from_utf8((buf).to_vec()).unwrap());
        drop(core);
        if let Ok(res) = result {
            // println!("shim: read: success: {res}");
            Ok(res)
        } else {
            // error
            Err(Error::other("read error"))
        }
    }
}
impl Write for SmolTcpStream {
    // write
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let engine = &ENGINE;
        let mut core = engine.core.lock().unwrap();
        let socket = core.get_mutable_socket(self.socket_handle);
        let result = socket.send_slice(buf);
        drop(core);
        if let Ok(res) = result {
            // success
            // println!("shim: wrote: {res} bytes:");
            // println!("{}", String::from_utf8((buf).to_vec()).unwrap());
            Ok(res)
        } else {
            // error
            Err(Error::other("write error"))
        }
    }
    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
        // idk this is what std::net::TcpStream::flush() does: 
        // https://doc.rust-lang.org/src/std/net/tcp.rs.html#695
        // also smoltcp doesn't have a flush method for its
    }
}
pub trait From<SmolTcpStream> {
    fn new() {}
    fn from(s: SmolTcpStream) -> Self
    where
        Self: Sized,
    {
        todo!();
    }
}
impl From<SmolTcpStream> for SmolTcpStream {
    fn from(s: SmolTcpStream) -> SmolTcpStream {
        todo!();
    }
}

impl SmolTcpStream {
    /* each_addr:
     * helper function for connect()
     * processes each address given to see whether it can implement ToSocketAddr, then tries to
     * connect to that addr keeps trying each address until one of them successfully connects
     * parameters: addresses passed into connect(), reference to socket, reference to
     * interface context, and port.
     * return: port and address
     */
    fn each_addr<A: ToSocketAddrs>(
        sock_addrs: A,
        s: &mut Socket<'static>,
        cx: &mut Context,
        port: u16,
    ) -> Result<(), ConnectError> {
        let addrs = {
            match sock_addrs.to_socket_addrs() {
                Ok(addrs) => addrs,
                Err(e) => return Err(ConnectError::InvalidState),
            }
        };
        for addr in addrs {
            match (*s).connect(cx, addr, port) {
                Ok(_) => return Ok(()),
                Err(_) => return Err(ConnectError::Unaddressable),
            }
        }
        Err(ConnectError::InvalidState) // is that the correct thing to return?
    }
    /* connect():
     * parameters: address(es) a list of addresses may be given
     * return: a smoltcpstream that is connected to the remote server.
     */
    /// addr is an address of the remote host.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<SmolTcpStream, Error> {
        println!("shim: in connect()");
        let engine = &ENGINE; // accessing global engine
        let mut sock = {
            // create new socket
            let rx_buffer = SocketBuffer::new(vec![0; 4096]);
            let tx_buffer = SocketBuffer::new(vec![0; 4096]);
            Socket::new(rx_buffer, tx_buffer)
        };
        // TODO: don't hardcode in port. make ephemeral port.
        let PORT = 49152;
        let config = Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into()); // change later?
        let mut device = Loopback::new(Medium::Ethernet);
        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8))
                .unwrap();
        });
        if let Err(e) = Self::each_addr(addr, &mut sock, iface.context(), PORT) {
            println!("connect(): connection error!! {}", e);
            return Err(Error::other("connection error"));
        } else {
            // success
        }; // note to self: make sure remote endpoint matches the server address!
        let handle = (*engine).add_socket(sock);
        let smoltcpstream = SmolTcpStream {
            socket_handle: handle,
            port: PORT,
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), PORT),
            rx_shutdown: Arc::new(Mutex::new(false)),
        };
        Ok(smoltcpstream)
    }

    /* peer_addr():
     * parameters: -
     * return: the remote address of the socket. this is the address of the server
     * note: can only be used if already connected
     */
    pub fn peer_addr(&self) -> Result<SocketAddr, Error> {
        let engine = &ENGINE;
        let mut core = (*engine).core.lock().unwrap();
        let socket = core.get_socket(self.socket_handle);
        let remote = socket.remote_endpoint().unwrap();
        drop(core);
        let remote_addr = SocketAddr::from((remote.addr, remote.port));
        Ok(remote_addr)
        // TODO: add error handling
    }

    /* shutdown_write():
     * helper function for shutdown()
     */
    fn shutdown_write(socket: &mut Socket<'static>) {
        socket.close(); // close() only closes the transmit half of the connection
    }
    /* shutdown_both():
     * helper function for shutdown()
     */
    fn shutdown_both(socket: &mut Socket<'static>) {
        socket.abort();
        // abort() immediately aborts the connection and closes the socket, sends a reset packet to
        // the remote endpoint
    }
    /* shutdown_read():
     * helper function for shutdown()
     */
    fn shutdown_read(&self) {
        let mut read_shutdown = self.rx_shutdown.lock().unwrap();
        *read_shutdown = true;
    }
    fn get_rx_shutdown(&self) -> bool {
        *self.rx_shutdown.lock().unwrap()
    }
    /* shutdown():
     * parameters: how - an enum of Shutdown that specifies what part of the socket to shutdown.
     *             options are Read, Write, or Both.
     * return: Result<> indicating success, (), or failure, Error
     */
    /* TODO: this really is an issue for later rather than sooner, but
    ASK DANIEL how he wants to handle this for Twizzler:

    "Calling this function multiple times may result in different behavior,
    depending on the operating system. On Linux, the second call will
    return `Ok(())`, but on macOS, it will return `ErrorKind::NotConnected`.
    This may change in the future." -- std::net documentation
    */
    pub fn shutdown(&self, how: Shutdown) -> Result<(), Error> {
        // specifies shutdown of read, write, or both with enum Shutdown
        let engine = &ENGINE;
        let mut core = (*engine).core.lock().unwrap(); // acquire mutex
        let mut socket = core.get_mutable_socket(self.socket_handle);
        match how {
            Shutdown::Read => {
                // "All currently blocked and future reads will return Ok(0)."
                // -- std::net shutdown documentation
                // READ the implementation of may_recv at https://docs.rs/smoltcp/latest/src/smoltcp/socket/tcp.rs.html#1104
                // (*socket).rx_buffer.clear(); <<<--------------------- cannot access private
                // rx_buffer. how to clear rx_buffer?
                Self::shutdown_read(&self);
                return Ok(());
            }
            Shutdown::Write => {
                Self::shutdown_write(&mut socket);
                return Ok(());
            } // mutex drops here
            Shutdown::Both => {
                Self::shutdown_both(&mut socket);
                return Ok(());
            } // mutex drops here
        } // mutex drops here if shutdown(read)
    }

    pub fn try_clone(&self) -> Result<SmolTcpStream, Error> {
        // more doc reading necessary?
        // todo!();
        let handle = self.socket_handle.clone();
        Ok(SmolTcpStream { 
            socket_handle: handle, 
            local_addr: self.local_addr, 
            port: self.port, 
            rx_shutdown: Arc::new(Mutex::new(self.get_rx_shutdown())) 
        })
    }
}
// implement impl std::fmt::Debug for SmolTcpStream
// add `#[derive(Debug)]` to `SmolTcpStream` or manually `impl std::fmt::Debug for SmolTcpStream`

/*
tests:
make_listener:

*/
#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use crate::shim::SmolTcpListener;
    #[test]
    fn make_listener() {
        let listener = SmolTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 443))).unwrap();
        let stream = listener.accept();
    }
}
