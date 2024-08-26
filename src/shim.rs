use std::{
    net::{Shutdown, SocketAddr, ToSocketAddrs},
    path::PathBuf,
};
use smoltcp::socket::tcp::{Socket, ListenError, ConnectError};
use smoltcp::wire::IpListenEndpoint;
pub type SocketBuffer<'a> = smoltcp::storage::RingBuffer<'a, u8>;

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener<'a> {
    socket: Socket<'a>
    // NOTE: the name of this type CHANGES between 0.8.2 (the version forked into
    //      Twizzler) and 0.11 (the default that docs.rs shows)⚠️
}

impl<'a> SmolTcpListener<'a> {
    /* bind
    * accepts: address(es) 
    * returns: a tcpsocket
    * creates a tcpsocket and binds the address to that socket. 
    * if multiple addresses given, it will attempt to bind to each until successful
    */
    // each_addr() taken from the standard tcp implementation in Rust 
    // found here: https://doc.rust-lang.org/src/std/net/mod.rs.html

    fn each_addr<A: ToSocketAddrs, F, T>(addr: A, mut f: F) -> Result<T, ListenError>
    where
        F: FnMut(T) -> Result<(), ListenError>,
    {
        let addrs = match addr.to_socket_addrs() {
            Ok(addrs) => addrs,
            Err(e) => return f(Err(e)),
        };
        let mut last_err = None;
        for addr in addrs {
            match f(Ok(&addr)) {
                Ok(l) => return Ok(l),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            println!("invalid input, could not resolve to any addresses")
        }))
    }
    pub fn bind(addr: &ToSocketAddrs) -> SmolTcpListener<'a> {
      // trial socket
      // probably to change the buffers!!!!
      let rx_buffer = SocketBuffer::new(vec![0; 64]);
      let tx_buffer = SocketBuffer::new(vec![0; 64]);
      let mut sock = Socket::new(rx_buffer, tx_buffer);
      let mut stcp_listener = SmolTcpListener{sock};
      Self::each_addr(addr, Socket::listen).map(); // what goes in map?
      stcp_listener
    }
    
    // local_addr
    // return socketaddr from local_endpoint (stcp sock)
    pub fn local_addr(&self) -> IpListenEndpoint {
        self.socket.local_endpoint
    }
    
    // accept
    // create a smoltcpstream object
    // clone socket into the smoltcpstream
    // call connect() (i think?) on the copied socket to change it to stream state
    // transfer ownership of the socket from the listener to the stream object
    // return the stream object and socket address
    pub fn accept(&self) -> Result<(SmolTcpStream, IpListenEndpoint), ConnectError> {
        // clone the listener
        let mut clone = duplicate();
        let mut stream = SmolTcpStream {clone};
        clone.connect();
        (stream, self.local_addr())
    }


    // try_clone
    // smoltcp has no direct way to do this
    pub fn try_clone() {
        duplicate()
    }

    // duplicate
    fn duplicate() {}
}

pub struct SmolTcpStream<'a> {
    // tcpsocket (copy of the one in listener)
    socket: Socket<'a>
}

impl<'a> SmolTcpStream<'a> {
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

    // connect
    // TODO: research what is a RefinedTcpStrema?


}
