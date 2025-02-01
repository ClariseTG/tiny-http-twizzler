use std::{
    io::{Error, Read, Write},
    net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, ToSocketAddrs},
    sync::{mpsc, Arc, Condvar, Mutex}, thread::JoinHandle,
};

use lazy_static::lazy_static;
use smoltcp::{
    iface::{Config, Context, Interface, SocketHandle, SocketSet},
    phy::{Loopback, Medium},
    socket::tcp::{ConnectError, ListenError, Socket, State},
    storage::RingBuffer,
    time::{Instant, Duration},
    wire::{EthernetAddress, IpAddress, IpCidr},
};
use crate::port::PortAssigner;
pub type SocketBuffer<'a> = RingBuffer<'a, u8>;
pub struct Engine {
    core: Arc<Mutex<Core>>,
    waiter: Arc<Condvar>,
    channel: mpsc::Sender<()>,
    _polling_thread: JoinHandle<()>
}
struct Core {
    socketset: SocketSet<'static>,
    iface: Interface,
    device: Loopback, // for now
}

lazy_static! {
    static ref ENGINE: Arc<Engine> = Arc::new(Engine::new());
    static ref PORTS: Arc<PortAssigner> = Arc::new(PortAssigner::new());
}

impl Engine {
    fn new() -> Self {
        // daniel's code: https://github.com/dbittman/smoltcp-test/blob/f866cb22a35de18d3a7a28d14e10044b45a87c6c/src/main.rs#L240
        let (sender, receiver) = std::sync::mpsc::channel();
        let core = Arc::new(Mutex::new(Core::new()));
        let waiter = Arc::new(Condvar::new());
        let _inner = core.clone();
        let _waiter = waiter.clone();

        // Okay, here is our background polling thread. It polls the network interface with the SocketSet
        // whenever it needs to, which is:
        // 1. when smoltcp says to based on poll_time() (calls poll_delay internally)
        // 2. when the state changes (eg a new socket is added)
        // 3. when blocking threads need to poll (we get a message on the channel)
        // credit: daniel
        let thread = std::thread::spawn(move || {
            let inner = _inner;
            let waiter = _waiter;
            loop {
                let time = {
                    let mut inner = inner.lock().unwrap();
                    let time = inner.poll_time();

                    // We may need to poll immediately!
                    if matches!(time, Some(Duration::ZERO)) {
                        inner.poll(&*waiter);
                        continue;
                    }
                    time
                };

                // Wait until the designated timeout, or until we get a message on the channel.
                match time {
                    Some(dur) => {
                        let _ = receiver.recv_timeout(dur.into());
                    }
                    None => {
                        receiver.recv().unwrap();
                    }
                }
            }
        });
        Self {
            core: core,
            waiter: waiter,
            channel: sender,
            _polling_thread: thread,
        }
    }
    fn add_socket(&self, socket: Socket<'static>) -> SocketHandle {
        self.core.lock().unwrap().add_socket(socket)
    }
    // Block until f returns Some(R), and then return R. Note that f may be called multiple times, and it may
    // be called spuriously.
    fn blocking<R>(&self, mut f: impl FnMut(&mut Core) -> Option<R>) -> R {
        let mut core = self.core.lock().unwrap();
        // Immediately poll, since we wait to have as up-to-date state as possible.
        core.poll(&self.waiter);
        loop {
            // We'll need the polling thread to wake up and do work.
            self.channel.send(()).unwrap();
            match f(&mut *core) {
                Some(r) => {
                    // We have done work, so again, notify the polling thread.
                    self.channel.send(()).unwrap();
                    drop(core);
                    return r;
                }
                None => {
                    core = self.waiter.wait(core).unwrap();
                }
            }
        }
    }
}

impl Core {
    fn new() -> Self {
        let config = Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into()); // change later!
        let socketset = SocketSet::new(Vec::new());
        let mut device = Loopback::new(Medium::Ethernet);
        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 9))
                .unwrap();
        });
        Self {
            socketset: socketset,
            device: device,
            iface: iface,
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
    // thank you daniel for polling code
    fn poll(&mut self, waiter: &Condvar) -> bool {
        let res = self.iface.poll(Instant::now(), &mut self.device, &mut self.socketset);
        // When we poll, notify the CV so that other waiting threads can retry their blocking
        // operations.
        waiter.notify_all();
        res
    }
    fn poll_time(&mut self) -> Option<Duration> {
        self.iface.poll_delay(Instant::now(), &mut self.socketset)
    }
}

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener {
    listeners: Vec<Arc<Mutex<SocketHandle>>>, // stores 8 listeners. each listener's socket handle has a mutex.
    local_addr: SocketAddr,
    port: u16,
}
struct listener {
    socket_handle: SocketHandle,
    local_addr: SocketAddr,
    port: u16,
}
// TODO: take out
// create an array of listeners and store their handles in the struct
// when swapping the listener, swap out each listener that has advanced state.

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
                Err(_) => return Err(ListenError::InvalidState),
            }
        };
        for addr in addrs {
            match (*s).listen(addr.port()) {
                Ok(_) => return Ok((addr.port(), addr)),
                Err(_) => return Err(ListenError::Unaddressable),
            }
        }
        Err(ListenError::InvalidState)
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
    fn bind_once<A: ToSocketAddrs>(addrs: A) -> Result<listener, Error> {
        let engine = &ENGINE;
        let (sock, port, local_address) = {
            match Self::do_bind(addrs) {
                Ok((sock, port, local_address)) => (sock, port, local_address),
                Err(_) => {return Err(Error::other("listening error"));}
            }
        };
        let handle = (*engine).add_socket(sock);
        let mut tcp_listener = listener {
            socket_handle: handle,
            port,
            local_addr: local_address,
        };
        Ok(tcp_listener)
    }
    /* bind
     * accepts: address(es)
     * returns: a tcpsocket
     * creates a tcpsocket and binds the address to that socket.
     * if multiple addresses given, it will attempt to bind to each until successful

        example arguments passed to bind:
        "127.0.0.1:0"
        SocketAddr::from(([127, 0, 0, 1], 443))
        let addrs = [ SocketAddr::from(([127, 0, 0, 1], 80)),  SocketAddr::from(([127, 0, 0, 1], 443)), ];
    */
    pub fn bind<A: ToSocketAddrs>(addrs: A) -> Result<SmolTcpListener, Error> {
        let mut listeners = Vec::with_capacity(8);
        let listener = {
            match Self::bind_once(addrs) {
                Ok(listener) => listener,
                Err(_) => {return Err(Error::other("listening error"));}
            }
        };
        let addr = listener.local_addr;
        let port = listener.port;
        listeners.push(Arc::new(Mutex::new(listener.socket_handle)));
        for i in 0..7 { // 7 more times
            match Self::bind_once(addr) {
                Ok(listener) => {listeners.push(Arc::new(Mutex::new(listener.socket_handle)));}
                Err(_) => {return Err(Error::other("listening error"));}
            }
        }
        let smoltcplistener = SmolTcpListener {
            listeners,
            local_addr: addr,
            port,
        };
        // all listeners are now in the socket set and in the array within the SmolTcpListener
        Ok(smoltcplistener) // return the first listener
    }
    // start here in the morning
    fn get_handle(&self, listener_no: usize) -> SocketHandle {
        let handle = self.listeners[listener_no].lock().unwrap();
        *handle
    }
    fn change_handle(&self, listener_no: usize, new_handle: SocketHandle) {
        let mut handle = self.listeners[listener_no].lock().unwrap();
        *handle = new_handle;
    }

    // accept
    // create a new socket for tcpstream
    // ^^ creating a new one so that the user can call accept() on the previous one again
    // return tcpstream
    /* accept():
     * parameters: -
     * return: (SmolTcpStream, SocketAddr) upon success; Error upon failure
     * takes the current listener and advances the socket's state (in terms of the smoltcp state machine)
     */
    // to think about: each socket must be pulled from the engine and checked for activeness.
    pub fn accept(&self) -> Result<(SmolTcpStream, SocketAddr), Error> {
        // this is the listener
        // println!("in accept");
        let mut remote_addr = None;
        let engine = &ENGINE;
        let mut i: usize = 0;
        loop {
            if i > 7 {i = 0;}
            let mut core = (*engine).core.lock().unwrap();
            core.poll(&engine.waiter);
            engine.channel.send(()).unwrap();
            let handle = self.get_handle(i);
            let sock = core.get_mutable_socket(handle);
            if sock.is_active() {
                let remote = sock.remote_endpoint().unwrap(); // the socket addr returned is that of the remote endpoint. ie. the client.
                remote_addr = Some(SocketAddr::from((remote.addr, remote.port)));
                engine.channel.send(()).unwrap(); // ??
                drop(core);
                { // creating another listener and swapping self's socket handle
                    let next_list = Self::bind_once(self.local_addr).unwrap(); // create exactly one listener
                    Self::change_handle(&self, i, next_list.socket_handle);
                }
                i+=1;
                let stream = SmolTcpStream {
                    socket_handle: handle,
                    port: self.port,
                    rx_shutdown: Arc::new(Mutex::new(false)),
                };
                if let None = remote_addr {return Err(Error::other("accept error"));}
                return Ok((stream, remote_addr.unwrap()));
            } else {
                if i<=7 {
                    i+=1;
                    continue;
                }
                core = engine.waiter.wait(core).unwrap();
            }
            return Err(Error::other("accept error"));
        }
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
    port: u16,
    rx_shutdown: Arc<Mutex<bool>>,
}
impl Read for SmolTcpStream {
    /* read():
     * parameters - reference to where the data should be placed upon reading
     * return - number of bytes read upon success; error upon error
     * loads the data read into the buffer given
     * if shutdown(Shutdown::Read) has been called, all reads will return Ok(0)
     */
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        // "All currently blocked and future reads will return Ok(0)."
        // -- std::net shutdown documentation
        // ^^ check whether the shutdown for read has been called, then return Ok(0)
        if self.get_rx_shutdown() == true {
            return Ok(0);
        }
        let mut dequeued = None;
        let engine = &ENGINE;
        engine.blocking(|core| {
            let socket = core.get_mutable_socket(self.socket_handle);
            if socket.can_recv() {
                dequeued = Some(socket.recv_slice(buf).unwrap());
                Some(dequeued)
            } else if socket.state() == State::CloseWait {
                dequeued = Some(0);
                    self.set_rx_shutdown(true);
                    Some(dequeued)
            } else {
                None
            }
        });
        if let Some(deq) = dequeued {
            Ok(deq)
        } else {
            // error
            Err(Error::other("read error"))
        }
    }
}
impl Write for SmolTcpStream {
    /* write():
     * parameters - reference to data to be written (represented as an array of u8)
     * result - number of bytes written upon success; error upon error.
     * writes given data to the connected socket.
     */
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let engine = &ENGINE;
        let mut enqueued = None;
        engine.blocking(|core| {
            let socket = core.get_mutable_socket(self.socket_handle);
            if socket.can_send() {
                enqueued = Some(socket.send_slice(buf).unwrap());
                Some(())
            } else {
                None
            }
        });
        if let Some(enq) = enqueued {
            // success
            Ok(enq)
        } else {
            // error
            Err(Error::other("write error"))
        }
    }
    /* flush():
     */
    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
        // lol this is what std::net::TcpStream::flush() does: 
        // https://doc.rust-lang.org/src/std/net/tcp.rs.html#695
        // also smoltcp doesn't have a flush method for its socket buffer
    }
}
pub trait From<SmolTcpStream> {
    fn from<>(s: SmolTcpStream) -> Self
    where
        Self: Sized,
    {
        todo!()
    }
}
impl From<SmolTcpStream> for SmolTcpStream {
    fn from(s: SmolTcpStream) -> SmolTcpStream {
        s
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
                Err(_) => return Err(ConnectError::InvalidState),
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
     * parameters: address(es) a list of addresses may be given. must take a REMOTE HOST'S address
     * return: a smoltcpstream that is connected to the remote server.
     */
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<SmolTcpStream, Error> {
        let engine = &ENGINE;
        let mut sock = {
            // create new socket
            let rx_buffer = SocketBuffer::new(vec![0; 4096]);
            let tx_buffer = SocketBuffer::new(vec![0; 4096]);
            Socket::new(rx_buffer, tx_buffer)
        };
        let ports = &PORTS;
        let Some(port) = (*ports).get_ephemeral_port() else {return Err(Error::other("dynamic port overflow!"))}; // this is the client's port
        let config = Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into()); // change later?
        let mut device = Loopback::new(Medium::Ethernet);
        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8))
                .unwrap();
        });
        if let Err(e) = Self::each_addr(addr, &mut sock, iface.context(), port) {
            println!("connect(): connection error!! {}", e);
            return Err(Error::other("connection error"));
        } else {
            // success
        }; // note to self: make sure remote endpoint matches the server address!
        let handle = (*engine).add_socket(sock);
        let smoltcpstream = SmolTcpStream {
            socket_handle: handle,
            port: port,
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
        // TODO: add error handling? none so far because this shouldn't fail
    }

    /* shutdown_write():
     * helper function for shutdown()
     */
    fn shutdown_write(&self, socket: &mut Socket<'static>) {
        socket.close(); // close() only closes the transmit half of the connection
    }
    /* shutdown_both():
     * helper function for shutdown()
     */
    fn shutdown_both(&self, socket: &mut Socket<'static>) {
        socket.abort();
        // abort() immediately aborts the connection and closes the socket, sends a reset packet to
        // the remote endpoint
    }
    /* shutdown_read():
     * helper function for shutdown()
     */
    fn shutdown_read(&self) {
        self.set_rx_shutdown(true);
    }
    fn set_rx_shutdown(&self, set: bool) {
        let mut read_shutdown = self.rx_shutdown.lock().unwrap();
        *read_shutdown = set;
    }
    fn get_rx_shutdown(&self) -> bool {
        *self.rx_shutdown.lock().unwrap()
    }
    /* shutdown():
     * parameters: how - an enum of Shutdown that specifies what part of the socket to shutdown.
     *             options are Read, Write, or Both.
     * return: Result<> indicating success, (), or failure, Error
     */
    /*
    "Calling this function multiple times may result in different behavior,
    depending on the operating system. On Linux, the second call will
    return `Ok(())`, but on macOS, it will return `ErrorKind::NotConnected`.
    This may change in the future." -- std::net documentation
    // Twizzler returns Ok(())
    */
    pub fn shutdown(&self, how: Shutdown) -> Result<(), Error> {
        // specifies shutdown of read, write, or both with enum Shutdown
        let engine = &ENGINE;
        let mut core = (*engine).core.lock().unwrap(); // acquire mutex
        let mut socket = core.get_mutable_socket(self.socket_handle);
        if socket.state() == State::Closed { // if already closed, exit early
            drop(core);
            return Ok(());
        }
        let ports = &PORTS;
        (*ports).return_port(self.port);
        match how {
            Shutdown::Read => {
                // "All currently blocked and future reads will return Ok(0)."
                // -- std::net shutdown documentation
                // READ the implementation of may_recv at https://docs.rs/smoltcp/latest/src/smoltcp/socket/tcp.rs.html#1104
                // (*socket).rx_buffer.clear(); <<<--------------------- cannot access private rx_buffer
                self.shutdown_read();
                return Ok(());
            }
            Shutdown::Write => {
                self.shutdown_write(&mut socket);
                return Ok(());
            } // mutex drops here
            Shutdown::Both => {
                self.shutdown_both(&mut socket);
                return Ok(());
            } // mutex drops here
        } // mutex drops here if shutdown(read)
    }

    pub fn try_clone(&self) -> Result<SmolTcpStream, Error> {
        let handle = self.socket_handle.clone();
        Ok(SmolTcpStream { 
            socket_handle: handle, 
            port: self.port, 
            rx_shutdown: Arc::new(Mutex::new(self.get_rx_shutdown())) 
        })
    }
}
// implement impl std::fmt::Debug for SmolTcpStream
// add `#[derive(Debug)]` to `SmolTcpStream` or manually `impl std::fmt::Debug for SmolTcpStream`
