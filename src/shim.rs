// NOTES
// poll() is the driving feature that drives all state machines forward. poll = try to move forward in work. call in a loop on a separate thread.
// why does daniel use loopback device? other examples use other ones. possibly because we are currently only focused on working with a local host loopback device. later, we can try working with other ones.
// what is the point of the stack/blocking. it allows us to synchronize the use of sockets behind an interface that we create. using a stack also helps with lifetime issues. we only need to deal with integers as socket handles to access sockets in a set.
// how do i find the sizes/actual numbers for everything? a later issue. don't worry about it.
// use socket handles!! use a stack to use sockets!! 
// can bind be called with the same address? probably. try it.

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
    wire::{IpListenEndpoint, IpEndpoint, EthernetAddress, IpAddress},
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
    // init: bool,
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
    // fns to get sockets
    fn block(){}
}

impl Core {
    fn new() -> Self {
        let mut core = Self {
            socketset: SocketSet::new(Vec::new()),
            // init: false,
        };
        // create a new thread and poll
        // todo!();
        // core.init = true;
        core
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
    fn poll() {}
}

// a variant of std's tcplistener using smoltcp's api
pub struct SmolTcpListener {
    // local_addr: SocketAddr, // maybe needed. maybe not. take out afterwards
    socket_handle: SocketHandle,
}

impl SmolTcpListener {
    // check_socketset_conditions
    // checks whether socket set exists. else makes a new one
    fn check_socketset_conditions(pointer: Option<Arc<Engine>>) -> Arc<Engine> {
        match pointer {
            Some(e) => e,
            None => {
                let e = Arc::new(Engine::new());
                e
            }
            // what about init field in core?
        }
    }

    // each_addr
    fn each_addr<A: ToSocketAddrs>(sock_addrs: A, s: &mut Socket<'static>) -> Result<(), ListenError> {
        let addrs = {
            match sock_addrs.to_socket_addrs() {
                Ok(addrs) => addrs, 
                Err(e) => return Err(ListenError::InvalidState),
            }
        };
        for addr in addrs {
            match (*s).listen(addr.port()) {
                Ok(_) => return Ok(()),
                Err(e) => return Err(ListenError::Unaddressable),
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
    pub fn bind<A: ToSocketAddrs>(addrs: A) -> Result<SmolTcpListener, ListenError> {
        // is the return value of ListenError correct?
      /*
      passed to bind: 
      "127.0.0.1:0"
      SocketAddr::from(([127, 0, 0, 1], 443))
      let addrs = [ SocketAddr::from(([127, 0, 0, 1], 80)),  SocketAddr::from(([127, 0, 0, 1], 443)), ];
      */

      // FIX ARG TO check_socketset_conditions!!!!!!!!!!!!!!!!!!
      let engine = Self::check_socketset_conditions(None);

      let rx_buffer = SocketBuffer::new(Vec::new());
      let tx_buffer = SocketBuffer::new(Vec::new());
      let mut sock = Socket::new(rx_buffer, tx_buffer);
      if let Err(e) = Self::each_addr(addrs, &mut sock) {
        return Err(e);
      }
    //   let _ = sock.listen(addrs.port()); // change later
      let handle = engine.add_socket(sock);
      let tcp = SmolTcpListener { socket_handle: handle };
      Ok(tcp)
    }

}

pub struct SmolTcpStream {
    // tcpsocket (copy of the one in listener)
    socket_handle: usize
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

    // connect
    // TODO: research what is a RefinedTcpStrema?


}

#[cfg(test)]
mod tests {
    use crate::shim::SmolTcpListener;
    use std::net::SocketAddr;
    #[test]
    fn make_listener() {
        let _listener = SmolTcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 443))).unwrap();
    }
}