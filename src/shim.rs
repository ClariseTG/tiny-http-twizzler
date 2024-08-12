use std::{
    net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    path::PathBuf,
};

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener();

impl SmolTcpListener {
    // implement from
    // listener creates a smoltcp::socket, then calls listen() on it
    
    // implement local_addr
    // return socketaddr from local_endpoint (stcp sock)
    
    // implement accept
    // create a smoltcpstream object
    // call connect() (i think?) on the socket to change it to stream state
    // transfer ownership of the socket from the listener to the stream object
    // return the stream object  
}

pub struct SmolTcpStream();

impl SmolTcpStream(){
    // imp. read
    // call can_recv
    // call recv on up to the size of the buffer + load it
    // return recv's f
    //
    // imp. write
    // call can_send
    // call send on buffer, then return f from send
    //
    // imp. flush
    // needs to make sure the output buffer is empty... 
    //      maybe a loop of checking can_send until it's false?
    // have to check how the buffer is emptied. it seems automatic?
    // 
}
