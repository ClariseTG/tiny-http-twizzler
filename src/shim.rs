// NOTES
// what ip/dest

use lazy_static::lazy_static;
use std::{
    sync::{Arc, Condvar, Mutex},
    net::{Shutdown, SocketAddr, ToSocketAddrs, IpAddr, Ipv4Addr},
    path::PathBuf,
    io::Error,
};
use smoltcp::{
    socket::{ 
        tcp::{Socket, ListenError, ConnectError}
    },
    time::{Instant},
    phy::{Loopback, Medium},
    wire::{IpListenEndpoint, IpEndpoint, EthernetAddress, IpAddress, IpCidr},
    storage::{Assembler, RingBuffer},
    iface::{Config, Interface, SocketHandle, SocketSet}
};
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
    fn block(){}
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
        todo!();
    }
}

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener {
    local_addr: SocketAddr,
    socket_handle: SocketHandle,
    port: u16,
}

impl SmolTcpListener {
    // each_addr
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
        let mut sock = Socket::new(rx_buffer, tx_buffer);
        let (port, local_address) = {
            match Self::each_addr(addrs, &mut sock) {
                Ok((port, local_address)) => (port, local_address),
                Err(_) => {
                    return Err(Error::Other("listening error"))
                },
            }
        };

        let handle = (*engine).add_socket(sock);
        let tcp = SmolTcpListener { socket_handle: handle, port: port, local_addr: local_address};
        Ok(tcp)
    }

    // accept
    // get socket from the socket set and connect()
    // create tcpstream and return tcpstream
    pub fn accept(&self) -> Result<(SmolTcpStream, SocketAddr), Error> {
        let engine = &ENGINE;
        let mut core = (*engine).core.lock().unwrap();
        let cx = core.iface.context();
        let mut core = (*engine).core.lock().unwrap();
        let socket = core.get_mutable_socket(self.socket_handle);
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let _ = socket.connect(cx, (addr, 1234), self.port).unwrap();
        let stream = SmolTcpStream { socket_handle: self.socket_handle };
        Ok((stream, SocketAddr::new(addr, self.port)))
        // add in support for errors!
    }

    pub fn local_addr(&self) -> Result<SocketAddr, Error> {
        return Ok(self.local_addr);
        // add in support for error!
    }

}

pub struct SmolTcpStream {
    // tcpsocket (copy of the one in listener)
    socket_handle: SocketHandle
}

impl SmolTcpStream {
    // imp. read
    // call can_recv
    // call recv on up to the size of the buffer + load it
    // return recv's f

    // write
    // call can_send
    // call send on buffer, then return f from send

    // flush
    // needs to make sure the output buffer is empty... 
    //      maybe a loop of checking can_send until it's false?
    // have to check how the buffer is emptied. it seems automatic?

    // peer_addr
    // remote_endpoint...?
    // TODO: what in the WORLD is a peer address i still haven't found the answer
    // ^^ it's the ip address of the router i think
 
    // shutdown
    // specifies shutdown of read, write, or both with an enum.
    // write half shutdown with close().
    // both with abort() though this will send a reset packet
    // TODO: what to do for read half?
  
    // try_clone
    // use try_from on all of the contained elements?
    // more doc reading necessary
   
    // from
    // shutdown
    // peet_addr
    // flush

    // connect
    // TODO: research what is a RefinedTcpStrema?


}


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
        let _listener = SmolTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 443))).unwrap();
    }
}
