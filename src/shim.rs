// NOTES
// what ip/dest

use lazy_static::lazy_static;
use std::{
    sync::{Arc, Condvar, Mutex},
    net::{SocketAddr, ToSocketAddrs, Shutdown, Ipv4Addr, IpAddr},
    io::{Error, Write, Read},
};
use smoltcp::{
    socket::{ 
        tcp::{Socket, ListenError,}
    },
    time::{Instant},
    phy::{Loopback, Medium},
    wire::{EthernetAddress, IpAddress, IpCidr},
    storage::{RingBuffer},
    iface::{Config, Interface, SocketHandle, SocketSet}
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
    device: Loopback, // for now.
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

    // check_socketset_conditions
    // checks whether socket set exists. else makes a new one
    fn check_socketset_conditions(pointer: Option<Arc<Engine>>) -> Arc<Engine> {
        match pointer {
            Some(e) => e,
            None => {
                let e = Arc::new(Self::new());
                e
            }
        }
    }
    // fns to get sockets
    // Block until f returns Some(R), and then return R. Note that f may be called multiple times, and it may
    // be called spuriously.
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
                .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8))
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
    fn poll(&mut self, waiter: &Condvar) -> bool {
        let res = self
            .iface
            .poll(Instant::now(), &mut self.device, &mut self.socketset);
        // When we poll, notify the CV so that other waiting threads can retry their blocking operations.
        println!("poll(): notify cv");
        waiter.notify_all();
        res
    }
}

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener {
    local_addr: SocketAddr,
    socket_handle: SocketHandle,
    port: u16,
}

impl SmolTcpListener {
    /* each_addr
     * helper function for bind()
     * processes each address given to see whether it can implement ToSocketAddr, then tries to listen on that addr
     * keeps trying each address until one of them successfully listens
    */
    fn each_addr<A: ToSocketAddrs>(sock_addrs: A, s: &mut Socket<'static>) -> Result<(u16, SocketAddr), ListenError> {
        let addrs = {
            match sock_addrs.to_socket_addrs() {
                Ok(addrs) => addrs, 
                Err(e) => return Err(ListenError::InvalidState),
            }
        };
        for addr in addrs {
            match (*s).listen(addr.port()) {
                Ok(_) => {
                    return Ok((addr.port(), addr))
                },
                Err(_) => return Err(ListenError::Unaddressable),
            }
        }
        Err(ListenError::InvalidState) // is that the correct thing to return?
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

        let rx_buffer = SocketBuffer::new(Vec::new());
        let tx_buffer = SocketBuffer::new(Vec::new());
        let mut sock: Socket<'static> = Socket::new(rx_buffer, tx_buffer); // this is the listening socket
        let (port, local_address) = {
            match Self::each_addr(addrs, &mut sock) {
                Ok((port, local_address)) => (port, local_address),
                Err(_) => {
                    return Err(Error::other("listening error"))
                },
            }
        };
        let handle = (*engine).add_socket(sock);
        // allocate a queue to hold pending connection requests. sounds like a semaphore
        let tcp = SmolTcpListener { socket_handle: handle, port: port, local_addr: local_address};
        Ok(tcp)
    }

    // accept
    // block until there is a waiting connection in the queue
    // create a new socket for tcpstream
    // ^^ creating a new one so that the user can call accept() on the previous one again
    // return tcpstream
    pub fn accept(&self) -> Result<(SmolTcpStream, SocketAddr), Error> {
        let engine = &ENGINE;
        let mut core = (*engine).core.lock().unwrap();
        let socket = core.get_mutable_socket(self.socket_handle);
        // std::mem::drop(core);
        
        // engine.blocking(|core| {
            // let socket = core.get_mutable_socket(self.socket_handle);
        //     if socket.is_active() {
        //         Some(())
        //     } else {
        //         None
        //     }
        // });
        println!("entered accept(), state of socket is: {}", socket.state()); // returns the state. for begugging information
        let rx_buffer = SocketBuffer::new(Vec::new());
        let tx_buffer = SocketBuffer::new(Vec::new());
        let mut sock: Socket<'static> = Socket::new(rx_buffer, tx_buffer);
        let stream_handle = engine.add_socket(sock);

        println!("accept(), new state of socket now is: {}", socket.state()); // returns the state. for begugging information
        // polling 

        let stream = SmolTcpStream { socket_handle: stream_handle };
        Ok((stream, self.local_addr))
        // add in support for errors!
    }

    pub fn local_addr(&self) -> Result<SocketAddr, Error> {
        return Ok(self.local_addr);
        // add in support for error!
    }

}

#[derive(Debug)]
pub struct SmolTcpStream {
    // tcpsocket (copy of the one in listener)
    socket_handle: SocketHandle
}
impl Read for SmolTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
    // check if the socket can even recieve
    //if may_recv(&self) {
    //    println!("Can recieve!");
        // call recv on up to the size of the buffer + load it
        // return recv's f
    //} else {
    //    println!("Cannot recieve :(");
    //}
    todo!();
    }
}
impl Write for SmolTcpStream {
    // write
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error>  {
        // call can_send
        // call send on buffer, then return f from send
        todo!(); 
    }
    fn flush(&mut self) -> Result<(), Error> {
        // needs to make sure the output buffer is empty... 
        //      maybe a loop of checking can_send until it's false?
        // have to check how the buffer is emptied. it seems automatic?
        todo!()
    }
}
pub trait From<SmolTcpStream> {
    fn new(){}
    fn from(s: SmolTcpStream) -> Self where Self:Sized {todo!();}
}
impl From<SmolTcpStream> for SmolTcpStream {
    fn from(s: SmolTcpStream) -> SmolTcpStream {
        todo!();
    }
}

impl SmolTcpStream {
    /// Opens a TCP connection to a remote host.
    /// addr is an address of the remote host.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<SmolTcpStream, Error> {
        // check that bind() & accept() has alr been called ? if this is a client function, how does that work?
        println!("in connect()");
        let engine = &ENGINE;
        let rx_buffer = SocketBuffer::new(Vec::new());
        let tx_buffer = SocketBuffer::new(Vec::new());
        let mut sock = Socket::new(rx_buffer, tx_buffer);
        let config = Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into()); // change later?
        let mut device = Loopback::new(Medium::Ethernet);
        let mut iface = Interface::new(config, &mut device, Instant::now());
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8))
                .unwrap();
        });
        println!("here?");
        // let mut core = (*engine).core.lock().unwrap();
        // engine.blocking (|core| {

        // });
        let error = sock.connect(iface.context(), (IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234), 49152); // make sure local endpoint matches the server address
        let handle = (*engine).add_socket(sock);
        println!("here2");
        match error {
            Err(e) => {
                println!("connect(): connection error!! {}", e);
                return Err(Error::other("connection error"));
            },
            Ok(()) => {
                println!("ok");
            },
        }
        let tcp = SmolTcpStream { socket_handle: handle };
        Ok(tcp)
    }

    pub fn peer_addr(&self) -> Result<SocketAddr, Error> {
        todo!() 
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<(), Error>{
        // specifies shutdown of read, write, or both with an enum.
        // write half shutdown with close().
        // both with abort() though this will send a reset packet
        // TODO: what to do for read half?
        match how {
            Shutdown::Read => {},
            Shutdown::Write => {},
            Shutdown::Both => {}
        }
        todo!();
    }

    pub fn try_clone(&self) -> Result<SmolTcpStream, Error> {
        // use try_from on all of the contained elements?
        // more doc reading necessary
        todo!()
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
    use crate::shim::SmolTcpListener;
    use std::net::SocketAddr;
    #[test]
    fn make_listener() {
        let listener = SmolTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 443))).unwrap();
        let stream = listener.accept();
    }
}
