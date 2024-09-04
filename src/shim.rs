use std::{
    net::{Shutdown, SocketAddr, ToSocketAddrs},
    path::PathBuf,
};
use smoltcp::{
    socket::{
        WakerRegistration,
        tcp{Socket, ListenError, ConnectError},
    },
    wire::{IpListenEndpoint, IpEndpoint},
    storage::{Assembler, RingBuffer},
};
// NOTE: the name of this type CHANGES between 0.8.2 (the version forked into
//      Twizzler) and 0.11 (the default that docs.rs shows)⚠️

// TODO -------------------------------------------
// - bind function
// - write test script that checks that bind just creates a tcpsocket
// ------------------------------------------------

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener {
    socket: TcpSocket,
    }

pub fn bind<A: ToSocketAddrs>(addr: A)-> Result<SmolTcpLister>{
    // takes an address and creates a listener
    // basically the new() function
}

impl SmolTcpListener {
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

pub struct SmolTcpStream {
    socket: TcpSocket,
    }

impl Read for &SmolTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    // check if the socket can even recieve
    if may_recv(&self) {
        println!("Can recieve!");
        // call recv on up to the size of the buffer + load it
        // return recv's f
    } else {
        println!("Cannot recieve :(");
    }
    }
}

impl SmolTcpStream {
    // read
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
 
    // shutdown
    // specifies shutdown of read, write, or both with an enum.
    // write half shutdown with close().
    // both with abort() though this will send a reset packet
    // TODO: what to do for read half?
  
    // try_clone
    // use try_from on all of the contained elements?
    // more doc reading necessary
   
    
}
