use lazy_static::lazy_static;
use std::{
    net::{Shutdown, SocketAddr, ToSocketAddrs, IpAddr, Ipv4Addr,},
    path::PathBuf,
    io::{self, Read, Write, Error},
    sync::Mutex,
};
use smoltcp::{
    socket::{
        tcp::{Socket, ListenError, ConnectError},
    },
    time::{Instant},
    phy::{Loopback, Medium}
    wire::{IpListenEndpoint, IpEndpoint, EthernetAddress, IpAddress, IpCidr},
    storage::{Assembler, RingBuffer},
    iface::{Config, Interface, SocketHandle, SocketSet},
};
use managed::ManagedSlice;
use crate::{
    sys::unsupported, Arc,};

// TODO ----------------------------
// 
// 
// 
// ---------------------------------

#[derive(Debug)]
pub struct Engine {
    // parts that need to be mutexed into: the core
    core: Arc<Mutex<Core>>,
    condvar: Arc<Condvar>,
}

struct Core {
    socketset: SocketSet<'static>,
    iface: Interface,
    device: Loopback,
    // init: bool,
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
    
    /// add_socket(Socket<'static>) adds a static socket to the socket set.
    fn add_socket(&self, socket: Socket<'static>) -> SocketHandle {
        // unlock mutexed core
        self.core.lock()
            // maybe change this to expect() later?
            .unwrap()
            // add the socket. this returns the handle, which we just pass out.
            .add_socket(socket)
    }

    fn check_socketset_conditions(pointer: Option<Arc<Engine>>) -> Arc<Engine> {
        match pointer {
            Some(e) => e,
            None => {
                let e = Arc::new(Self::new());
                e
            }
        }
    }
}

impl Core {
    fn new() -> Self {
        let config = Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into());
        let mut socketset = SocketSet::new(Vec::new());
        let mut device = Loopback::new(Medium::Ethernet);
        let mut iface = Interface::new(config, &mut device, Instant::now());
        
        // 
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(127,0,0,1), 8))
                .unwrap();
        });

        // return core
        Self {
            socketset: socketset,
            device: device,
            iface: iface,
        }
    }
}

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener {
    socket_handle: usize,
    // one more thing...?
}

pub fn init() {
    // TODO
    // global struct containing all of the actual sockets
    //let static mut socket_set: Mutex<SocketSet<'static>>;
    // heap var with a pointer that everybody knows?
    // the socket SET isnt owned, so there's a mutex on it
    //socket_set = SocketSet::new(Vec::new());
}



impl SmolTcpListener {
pub fn bind<A: ToSocketAddrs>(addr: A)-> Result<SmolTcpListener, Error> {
    // takes an address and creates a listener
    // address is the "remote endpoint"
    // basically the new() function
   
    // questions to research:
    // - should this fn initialize the socket array if it doesn't exist? or should that go
    //      elsewhere?
    // - what type goes in the rx/tx buffers?
    // - global variables?
    //
    // TODO: what does this vector hold??
    let rcv_buf = Vec::with_capacity(4);
    let trs_buf = Vec::with_capacity(4);
    let rx_buffer = RingBuffer::from(ManagedSlice::Owned(rcv_buf));
    let tx_buffer = RingBuffer::from(ManagedSlice::Owned(trs_buf));

    let socket = Socket::new(rx_buffer, tx_buffer);
    // place socket into socket array + extract the socket handle
    //let socket_id = add(socket);
    


    // put socket handle into SmolTcpListener
    let listener = SmolTcpListener {
        socket_handle: socket,
    };

    // return:
    Ok(listener)
}
    // from
    // listener creates a smoltcp::socket, then calls listen() on it
    
    // local_addr
    // return socketaddr from local_endpoint (stcp sock)
    
    // accept
    // create a smoltcpstream object
    // clone socket into the smoltcpstream
    // call connect() (i think?) on the copied socket to change it to stream state
    // transfer ownership of the socket from the listener to the stream object
    // return the stream object  
}

#[derive(Debug)]
pub struct SmolTcpStream {
    socket_handle: usize,
    // ???
}

impl Read for &SmolTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    // check if the socket can even recieve
    //if may_recv(&self) {
    //    println!("Can recieve!");
        // call recv on up to the size of the buffer + load it
        // return recv's f
    //} else {
    //    println!("Cannot recieve :(");
    //}
    unsupported()
    }
}

impl Write for &SmolTcpStream {
    // write
    fn write(&mut self, buf: &[u8]) -> io::Result<usize, Error>  {
        //TODO: UNTESTED AND UNFINISHED
        // get socket
        let engine = &ENGINE;
        // create a do_r_wr fn of some kind, match this to sock, port, local_address
        if (sock.can_send()) {
        // call can_send
        // call send on buffer, then return f from send
            // TODO this will panic most likely lmao
            sock.send_slice(buf).unwrap();
        }
    }
    // flush
    pub fn flush(&mut self) -> Result<(), Error> {
        // needs to make sure the output buffer is empty... 
        //      maybe a loop of checking can_send until it's false?
        // have to check how the buffer is emptied. it seems automatic?
        unsupported()
    }


}

impl SmolTcpStream {
    
    /// Opens a TCP connection to a remote host.
    /// addr is an address of the remote host.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<SmolTcpStream, Error> {
        // probably changing the state of the socket, then doing a poll (for now)
        //
        unsupported()
    }

    // peer_addr
    pub fn peer_addr(&self) -> Result<SocketAddr, Error> {
        // remote_endpoint...?
        // TODO: what in the WORLD is a peer address i still haven't found the answer
       unsupported() 
    }
 
    // shutdown
    pub fn shutdown(&self, how: Shutdown) -> Result<(), Error>{
        // specifies shutdown of read, write, or both with an enum.
        // write half shutdown with close().
        // both with abort() though this will send a reset packet
        // TODO: what to do for read half?
        unsupported()
    }
  
    // try_clone
    pub fn try_clone(&self) -> Result<SmolTcpStream, Error> {
        // use try_from on all of the contained elements?
        // more doc reading necessary
        unsupported()
    }
   
    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_socket(){
        
    }
}
