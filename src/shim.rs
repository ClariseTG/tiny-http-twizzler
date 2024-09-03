use std::{
    net::{Shutdown, SocketAddr, ToSocketAddrs},
    path::PathBuf,
};
use smoltcp::{
    socket::{
        WakerRegistration, 
        tcp::{Socket, ListenError, ConnectError}
    },
    wire::{IpListenEndpoint, IpEndpoint},
    storage::{Assembler, RingBuffer}
};
pub type SocketBuffer<'a> = RingBuffer<'a, u8>;
// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener<'a> {
    local_addr: SocketAddr,
    socket: Socket<'a>
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

    // fn each_addr<A: ToSocketAddrs, F, T>(addr: A, mut f: F, mut S: &SmolTcpListener<'a>) -> Result<T, ListenError>
    // where
    //     F: FnMut(IpListenEndpoint) -> Result<(), ListenError>,
    // {
    //     let addrs = match addr.to_socket_addrs() {
    //         Ok(addrs) => addrs,
    //         Err(e) => return Err(e),
    //     };
    //     let mut last_err = None;
    //     for addr in addrs {
    //         match f(addr) {
    //             Ok(l) => {
    //                 // populate the local_addr of struct
    //                 S.local_addr = addr;
    //                 return Ok(l)
    //             },
    //             Err(e) => last_err = Some(e),
    //         }
    //     }
    //     Err(last_err.unwrap_or_else(|| {
    //         ListenError::Unaddressable // is this right? lol
    //     }))
    // }
    // pub fn bind(addr: &dyn ToSocketAddrs<Iter=SocketAddr>) -> SmolTcpListener<'a> {
    //   /*
    //   passed to bind: 
    //   "127.0.0.1:0"
    //   SocketAddr::from(([127, 0, 0, 1], 443))
    //   let addrs = [ SocketAddr::from(([127, 0, 0, 1], 80)),  SocketAddr::from(([127, 0, 0, 1], 443)), ];
    //   */
    //   // trial socket
    //   // probably to change the buffers!!!!
    //   let rx_buffer = SocketBuffer::new(vec![0; 64]);
    //   let tx_buffer = SocketBuffer::new(vec![0; 64]);
    //   let mut sock = Socket::new(rx_buffer, tx_buffer);
    //   let mut stcp_listener = SmolTcpListener{local_addr: None, socket: sock};
    //   Self::each_addr(addr, Socket::listen, &stcp_listener); // .map() - what goes in map?
    //   stcp_listener
    // }
    
    // local_addr
    // return socketaddr from local_endpoint (stcp sock)
    // tuple contains a local endpoint and a remote endpoint and is set when connect() is called. 
    // listen_endpoint is what is passed into listen()
    // are they the same thing?
    // pub fn local_addr(&self) -> Option<IpEndpoint> {
    //     self.local_addr
    // }
    
    // accept
    // create a smoltcpstream object
    // clone socket into the smoltcpstream
    // call connect() (i think?) on the copied socket to change it to stream state
    // transfer ownership of the socket from the listener to the stream object
    // return the stream object and socket address
    // pub fn accept(&self) -> Result<(SmolTcpStream<'a>, IpListenEndpoint), ConnectError> {
    //     // clone the listener
    //     let mut clone = Self::try_clone();
    //     let mut stream = SmolTcpStream {socket: clone};
    //     clone.connect();
    //     (stream, self.local_addr())
    // }


    // try_clone
    // smoltcp has no direct way to do this
    // duplicate the smoltcp socket, then copy the local address
    // return SmolTcpListener
    pub fn try_clone(list: &SmolTcpListener<'a>) -> SmolTcpListener<'a>{
        let Self::duplicate(list.socket);

    }

    // duplicate only the smoltcp socket
    fn duplicate(sock: &Socket<'a>) -> Socket<'a>{
        let duplicate = Socket {
            // should double check that a new assembler is fine
            // should double check that sync is fine. i took out the cfgs
            // change the range of buffers?
            assembler: Assembler::new(),
            rx_buffer: SocketBuffer::new(vec![0; 64]),
            tx_buffer: SocketBuffer::new(vec![0; 64]),
            rx_waker: WakerRegistration::new(),
            tx_waker: WakerRegistration::new(),
            ..*sock
        };
        duplicate
    }
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
