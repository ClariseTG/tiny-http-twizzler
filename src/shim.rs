// NOTES
// poll() is the driving feature that drives all state machines forward. poll = try to move forward in work. call in a loop on a separate thread.
// why does daniel use loopback device? other examples use other ones. possibly because we are currently only focused on working with a local host loopback device. later, we can try working with other ones.
// what is the point of the stack/blocking. it allows us to synchronize the use of sockets behind an interface that we create. using a stack also helps with lifetime issues. we only need to deal with integers as socket handles to access sockets in a set.
// how do i find the sizes/actual numbers for everything? a later issue. don't worry about it.
// use socket handles!! use a stack to use sockets!! 
// can bind be called with the same address? probably. try it.

use std::{
    net::{Shutdown, SocketAddr, ToSocketAddrs, IpAddr, Ipv4Addr},
    path::PathBuf,
};
use smoltcp::{
    socket::{ 
        tcp::{Socket, ListenError, ConnectError}
    },
    time::{Instant},
    phy::{Loopback, Medium},
    wire::{IpListenEndpoint, IpEndpoint, EthernetAddress, IpAddress},
    storage::{Assembler, RingBuffer},
    iface::{Config, Interface}
};
pub type SocketBuffer<'a> = RingBuffer<'a, u8>;
// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener<'a> {
    local_addr: SocketAddr, // maybe needed. maybe not. take out afterwards
    socket: Socket<'a>
}

impl<'a> SmolTcpListener<'a> {
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

    // each_addr
    // verifies each address before calling the provided function
    // returns nothing, or error
    fn each_addr<A: ToSocketAddrs, F>(addr: A, mut f: F, mut S: &SmolTcpListener<'a>) -> Result<(), ListenError>
    where
        F: FnMut(dyn Into<IpListenEndpoint>) -> Result<(), ListenError>, {
            let addrs = match addr.to_socket_addrs() {
                Ok(addrs) => addrs,
                Err() => return ListenError::Unaddressable,
            };
            // include error handling for comparison of last error

    }

    /* bind
    * accepts: address(es) 
    * returns: a tcpsocket
    * creates a tcpsocket and binds the address to that socket. 
    * if multiple addresses given, it will attempt to bind to each until successful
    */
    pub fn bind(addr: &dyn ToSocketAddrs<Iter=SocketAddr>) -> SmolTcpListener<'a> {
      /*
      passed to bind: 
      "127.0.0.1:0"
      SocketAddr::from(([127, 0, 0, 1], 443))
      let addrs = [ SocketAddr::from(([127, 0, 0, 1], 80)),  SocketAddr::from(([127, 0, 0, 1], 443)), ];
      */
      // trial socket
      // probably to change the buffers!!!!
      let rx_buffer = SocketBuffer::new(vec![0; 64]);
      let tx_buffer = SocketBuffer::new(vec![0; 64]);
      let mut sock = Socket::new(rx_buffer, tx_buffer);
      // basic default socket addr. change later
      let mut stcp_listener = SmolTcpListener{
        local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080), 
        socket: sock
      };
      Self::each_addr(addr, Socket::listen, &stcp_listener, stcp_listener); // .map() - what goes in map?
      stcp_listener
    }

    // local_addr
    // return socketaddr from local_endpoint (stcp sock)
    // tuple contains a local endpoint and a remote endpoint and is set when connect() is called. 
    // listen_endpoint is what is passed into listen()
    // are they the same thing?
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
    // accept
    // create a smoltcpstream object
    // clone socket into the smoltcpstream
    // call connect() (i think?) on the copied socket to change it to stream state
    // transfer ownership of the socket from the listener to the stream object
    // return the stream object and socket address
    pub fn accept(&self) -> Result<(SmolTcpStream<'a>, SocketAddr), ConnectError> {
        // clone the listener
        let mut dupe = Self::try_clone(&self);
        let mut stream = SmolTcpStream {socket: dupe.socket};
        let mut iface = Self::make_iface();
        // example connection. change later.
        // Assuming fn get_ephemeral_port() -> u16 allocates a port between 49152 and 65535, 
        // call get_ephemeral_port() for connection's local endpoint
        dupe.socket.connect(iface.context(), (IpAddress::v4(10, 0, 0, 1), 80), 49153).unwrap();
        Ok((stream, self.local_addr()))
    } 

    fn make_iface() -> Interface {
        // example config. change later
        let config = Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into());
        // example device. change later
        let mut device = Loopback::new(Medium::Ethernet);
        let iface = Interface::new(config, &mut device, Instant::now());
        iface
    }


    // try_clone
    // smoltcp has no direct way to do this
    // duplicate the smoltcp socket, then copy the local address
    // return SmolTcpListener
    pub fn try_clone(list: &SmolTcpListener<'a>) -> SmolTcpListener<'a>{
        // let sock = Self::duplicate(&list.socket);
        let addr = list.local_addr();
        // let clone = SmolTcpListener{local_addr: addr, socket: sock};
        let clone = Self::bind(addr);
        // verify the ability to listen on the same socket again
        clone
    }

    // duplicate only the smoltcp socket
    // fn duplicate(sock: &Socket<'a>) -> Socket<'a>{
    //     let duplicate = Socket {
    //         // should double check that a new assembler is fine
    //         // should double check that sync is fine. i took out the cfgs
    //         // change the range of buffers?
    //         assembler: Assembler::new(),
    //         rx_buffer: SocketBuffer::new(vec![0; 64]),
    //         tx_buffer: SocketBuffer::new(vec![0; 64]),
    //         rx_waker: WakerRegistration::new(),
    //         tx_waker: WakerRegistration::new(),
    //         ..*sock
    //     };
    //     duplicate
    // }
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
