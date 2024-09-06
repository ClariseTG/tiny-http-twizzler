use std::{
    net::{Shutdown, SocketAddr, ToSocketAddrs},
    path::PathBuf,
    io::{self, Read, Write},
};
use smoltcp::{
    socket::{
        tcp::{Socket, ListenError, ConnectError},
    },
    wire::{IpListenEndpoint, IpEndpoint},
    storage::{Assembler, RingBuffer},
    iface::{SocketSet},
};
use managed::ManagedSlice;
// NOTE: the name of this type CHANGES between 0.8.2 (the version forked into
//      Twizzler) and 0.11 (the default that docs.rs shows)⚠️

// TODO -------------------------------------------
// - bind function
// - write test script that checks that bind just creates a tcpsocket
// - test script that checks the remote endpoint works
// ------------------------------------------------

// TODO
// global struct containing all of the actual sockets


// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener<'a> {
    socket_handle: usize,
    // one more thing...?
}

pub fn init() {
    // TODO
    let ??? = SocketSet::new(Vec::new());
}

pub fn bind<A: ToSocketAddrs>(addr: A)-> Result<SmolTcpListener<'a>{
    // takes an address and creates a listener
    // address is the "remote endpoint"
    // basically the new() function
   
    // questions to research:
    // - should this fn initialize the socket array if it doesn't exist? or should that go
    //      elsewhere?
    // - what type goes in the rx/tx buffers?

    // notes 09-06-24
    // smoltcp doesnt do stuff on its own
    // we need to use a loop to make it do stuff on its own
    // "poll" function is the driver
    //      -> runs whenever something changes (such as send_slice modifying state machine)
    //      -> if there is work to do it will do work
    //      -> uses states to figure out if something needs to be done
    //      -> native fn to smoltcp
    //      -> poll is called on an interface
    //      CONCLUSION: poll is like the execute() that i had in http130
    //
    // standard read/write will block until there is data and then return it
    // sockets can be referred to by handles, which are MUCH easier to pass around
    // the sockets themselves are locked and hard to move. but you can
    //      index into them with easy to move names! yippee
    // no lifetime information either if its just a usize inside
    //
    // make the Vec::new()s sized (daniel wrote it as a todo in the ex.)
    // bind might have a .select_device() eventually in twz to support
    //      hetero hardware/privacy
    // "for now, no need to worry about the ip address" - daniel

    // TODO: what does this vector hold??
    let rcv_buf = Vec::with_capacity(4);
    let trs_buf = Vec::with_capacity(4);
    let rx_buffer = RingBuffer::from(ManagedSlice::Owned(rcv_buf));
    let tx_buffer = RingBuffer::from(ManagedSlice::Owned(trs_buf));

    let socket = Socket::new(rx_buffer, tx_buffer);
    // place socket into socket array

    // extract the socket handle

    // put socket handle into SmolTcpListener
    let listener = SmolTcpListener;

    // return:
    Ok(listener)
}

impl<'a> SmolTcpListener<'a> {
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
    socket_handle: usize,
    }

impl<'a> Read for &SmolTcpStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    // check if the socket can even recieve
    //if may_recv(&self) {
    //    println!("Can recieve!");
        // call recv on up to the size of the buffer + load it
        // return recv's f
    //} else {
    //    println!("Cannot recieve :(");
    //}
    }
}

impl<'a> SmolTcpStream<'a> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_socket(){
        
    }
}
